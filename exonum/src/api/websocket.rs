// Copyright 2019 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! WebSocket API.

use actix::*;
use actix_web::ws;
use exonum_merkledb::{IndexAccess, ListProof, ObjectHash, Snapshot};
use futures::Future;
use log::error;
use rand::{rngs::ThreadRng, Rng};

use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use crate::{
    api::{
        node::public::explorer::{TransactionHex, TransactionResponse},
        ServiceApiState,
    },
    blockchain::{Block, Schema, TransactionResult, TxLocation},
    crypto::Hash,
    events::error::into_failure,
    explorer::TxStatus,
    messages::{AnyTx, BinaryValue, Message as ExonumMessage, ProtocolMessage, SignedMessage},
};

/// Message, coming from websocket connection.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(tag = "type", content = "payload", rename_all = "kebab-case")]
enum IncomingMessage {
    /// Set subscription for websocket connection.
    SetSubscriptions(Vec<SubscriptionType>),
    /// Send transaction to blockchain.
    Transaction(TransactionHex),
}

/// Subscription type (new blocks or committed transactions).
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Clone, PartialOrd, Ord)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum SubscriptionType {
    /// Subscription to nothing.
    None,
    /// Subscription on new blocks.
    Blocks,
    /// Subscription on committed transactions.
    Transactions {
        /// Optional filter for subscription.
        filter: Option<TransactionFilter>,
    },
}

/// Describe filter for transactions by ID of service and (optionally)
/// transaction type in service.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Clone, PartialOrd, Ord)]
pub struct TransactionFilter {
    /// ID of service.
    pub service_id: u16,
    /// Optional ID of transaction in service (if not set, all transaction of service will be sent).
    pub message_id: Option<u16>,
}

impl TransactionFilter {
    /// Create new transaction filter.
    pub fn new(service_id: u16, message_id: Option<u16>) -> Self {
        Self {
            service_id,
            message_id,
        }
    }
}

/// Summary about a particular transaction in the blockchain (without transaction content).
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct CommittedTransactionSummary {
    tx_hash: Hash,
    /// ID of service.
    pub service_id: u16,
    /// ID of transaction in service.
    pub message_id: u16,
    #[serde(with = "TxStatus")]
    status: TransactionResult,
    location: TxLocation,
    proof: ListProof<Hash>,
}

impl CommittedTransactionSummary {
    fn new<T>(schema: &Schema<T>, tx_hash: &Hash) -> Option<Self>
    where
        T: AsRef<dyn Snapshot> + IndexAccess,
    {
        let tx = schema.transactions().get(tx_hash)?;
        let service_id = tx.call_info.instance_id as u16;
        let tx_id = tx.call_info.method_id as u16;
        let tx_result = schema.transaction_results().get(tx_hash)?;
        let location = schema.transactions_locations().get(tx_hash)?;
        let location_proof = schema
            .block_transactions(location.block_height())
            .get_proof(location.position_in_block());
        Some(Self {
            tx_hash: *tx_hash,
            service_id,
            message_id: tx_id,
            status: tx_result,
            location,
            proof: location_proof,
        })
    }
}

/// Websocket notification message. This enum describe data, which is sent to
/// subscriber of websocket.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Notification {
    /// Notification about new block.
    Block(Block),
    /// Notification about new transaction.
    Transaction(CommittedTransactionSummary),
}

/// WebSocket message for communication between clients(`Session`) and server(`Server`).
#[derive(Message, Debug)]
pub(crate) struct Message(pub String);

#[derive(Message)]
#[rtype(u64)]
pub(crate) struct Subscribe {
    pub address: Recipient<Message>,
    pub subscriptions: Vec<SubscriptionType>,
}

#[derive(Message)]
pub(crate) struct Unsubscribe {
    pub id: u64,
}

#[derive(Message)]
pub(crate) struct UpdateSubscriptions {
    pub id: u64,
    pub subscriptions: Vec<SubscriptionType>,
}

#[derive(Message)]
pub(crate) struct Broadcast {
    pub block_hash: Hash,
}

#[derive(Message)]
#[rtype("Result<TransactionResponse, failure::Error>")]
pub(crate) struct Transaction {
    tx: TransactionHex,
}

pub(crate) struct Server {
    pub subscribers: BTreeMap<SubscriptionType, HashMap<u64, Recipient<Message>>>,
    service_api_state: Arc<ServiceApiState>,
    rng: RefCell<ThreadRng>,
}

impl Server {
    pub fn new(service_api_state: Arc<ServiceApiState>) -> Self {
        Self {
            subscribers: BTreeMap::new(),
            service_api_state,
            rng: RefCell::new(rand::thread_rng()),
        }
    }

    fn remove_subscriber(&mut self, id: u64) {
        self.subscribers.iter_mut().for_each(|(_, v)| {
            v.remove(&id);
        });
    }

    fn set_subscriptions(
        &mut self,
        id: u64,
        addr: Recipient<Message>,
        subscriptions: Vec<SubscriptionType>,
    ) {
        subscriptions.into_iter().for_each(|sub_type| {
            self.subscribers
                .entry(sub_type)
                .or_insert_with(HashMap::new)
                .insert(id, addr.clone());
        });
    }
}

impl Actor for Server {
    type Context = Context<Self>;
}

impl Handler<Subscribe> for Server {
    type Result = u64;

    fn handle(
        &mut self,
        Subscribe {
            address,
            subscriptions,
        }: Subscribe,
        _ctx: &mut Self::Context,
    ) -> u64 {
        let id = self.rng.borrow_mut().gen::<u64>();
        self.set_subscriptions(id, address, subscriptions);

        id
    }
}

impl Handler<Unsubscribe> for Server {
    type Result = ();

    fn handle(&mut self, Unsubscribe { id }: Unsubscribe, _ctx: &mut Self::Context) {
        self.remove_subscriber(id);
    }
}

impl Handler<UpdateSubscriptions> for Server {
    type Result = ();

    fn handle(
        &mut self,
        UpdateSubscriptions { id, subscriptions }: UpdateSubscriptions,
        _ctx: &mut Self::Context,
    ) {
        // Find address of subscriber. If id not found, assume that subscriber doesn't exist and return.
        let addr = if let Some(addr) = self
            .subscribers
            .values()
            .map(HashMap::iter)
            .flatten()
            .find_map(|(k, v)| if k == &id { Some(v.clone()) } else { None })
        {
            addr
        } else {
            return;
        };
        self.remove_subscriber(id);
        self.set_subscriptions(id, addr, subscriptions);
    }
}

impl Handler<Broadcast> for Server {
    type Result = ();

    fn handle(&mut self, Broadcast { block_hash }: Broadcast, _ctx: &mut Self::Context) {
        let snapshot = self.service_api_state.snapshot();
        let schema = Schema::new(&snapshot);
        let block = schema.blocks().get(&block_hash).unwrap();
        let height = block.height();
        let block_header = Notification::Block(block);

        // Notify about block
        self.broadcast_message(SubscriptionType::Blocks, &block_header);

        // Get list of transactions in block and notify about each of them.
        let tx_hashes_table = schema.block_transactions(height);
        tx_hashes_table
            .iter()
            .filter_map(|hash| {
                let res = CommittedTransactionSummary::new(&schema, &hash);
                if res.is_none() {
                    error!(
                        "BUG. Cannot build summary about committed transaction {:?} \
                         because it doesn't exist in \"transactions\", \
                         \"transaction_results\" nor \"transactions_locations\" indexes.",
                        hash
                    );
                }
                res
            })
            .for_each(|tx_info| {
                let service_id = tx_info.service_id;
                let tx_id = tx_info.message_id;
                let data = Notification::Transaction(tx_info);
                self.broadcast_message(SubscriptionType::Transactions { filter: None }, &data);
                self.broadcast_message(
                    SubscriptionType::Transactions {
                        filter: Some(TransactionFilter::new(service_id, None)),
                    },
                    &data,
                );
                self.broadcast_message(
                    SubscriptionType::Transactions {
                        filter: Some(TransactionFilter::new(service_id, Some(tx_id))),
                    },
                    &data,
                );
            });
    }
}

impl Handler<Transaction> for Server {
    type Result = Result<TransactionResponse, failure::Error>;

    fn handle(
        &mut self,
        Transaction { tx }: Transaction,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        let buf: Vec<u8> = hex::decode(tx.tx_body).map_err(into_failure)?;
        let signed = SignedMessage::from_bytes(buf.into())?;
        ensure!(signed.verify(), "Failed to verify signature.");
        let tx_hash = signed.object_hash();
        let signed = AnyTx::try_from(ExonumMessage::deserialize(signed)?)
            .map_err(|_| format_err!("Couldn't deserialize transaction message."))?;
        let _ = self
            .service_api_state
            .sender()
            .broadcast_transaction(signed);
        Ok(TransactionResponse { tx_hash })
    }
}

impl Server {
    fn broadcast_message<T>(&mut self, sub_type: SubscriptionType, data: &T)
    where
        T: serde::Serialize,
    {
        let serialized = serde_json::to_string(data).unwrap();
        self.subscribers
            .entry(sub_type)
            .or_insert_with(HashMap::new)
            .iter()
            .for_each(|(_, addr)| {
                let _ = addr.do_send(Message(serialized.clone()));
            });
    }
}

pub(crate) struct Session {
    pub id: u64,
    pub subscriptions: Vec<SubscriptionType>,
    pub server_address: Addr<Server>,
}

impl Session {
    pub fn new(server_address: Addr<Server>, subscriptions: Vec<SubscriptionType>) -> Self {
        Self {
            id: 0,
            server_address,
            subscriptions,
        }
    }

    fn process_incoming_message(&mut self, msg: IncomingMessage) -> WsStatus {
        match msg {
            IncomingMessage::SetSubscriptions(subs) => self.set_subscriptions(subs),
            IncomingMessage::Transaction(tx) => self.send_transaction(tx),
        }
    }

    fn set_subscriptions(&mut self, subscriptions: Vec<SubscriptionType>) -> WsStatus {
        self.subscriptions = subscriptions.clone();
        self.server_address
            .try_send(UpdateSubscriptions {
                id: self.id,
                subscriptions,
            })
            .map(|_| WsStatus::Success { response: None })
            .unwrap_or_else(|e| WsStatus::Error {
                description: e.to_string(),
            })
    }

    fn send_transaction(&mut self, tx: TransactionHex) -> WsStatus {
        self.server_address
            .send(Transaction { tx })
            .wait()
            .map(|x| match x {
                Ok(r) => WsStatus::Success {
                    response: Some(serde_json::to_value(&r).unwrap()),
                },
                Err(e) => WsStatus::Error {
                    description: e.to_string(),
                },
            })
            .unwrap_or_else(|e| WsStatus::Error {
                description: e.to_string(),
            })
    }
}

impl Actor for Session {
    type Context = ws::WebsocketContext<Self, ServiceApiState>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let address: Recipient<_> = ctx.address().recipient();
        self.server_address
            .send(Subscribe {
                address,
                subscriptions: self.subscriptions.clone(),
            })
            .into_actor(self)
            .then(|response, actor, context| {
                match response {
                    Ok(result) => {
                        actor.id = result;
                    }
                    _ => context.stop(),
                }
                fut::ok(())
            })
            .wait(ctx);
    }

    fn stopping(&mut self, _ctx: &mut <Self as Actor>::Context) -> Running {
        self.server_address.do_send(Unsubscribe { id: self.id });
        Running::Stop
    }
}

impl Handler<Message> for Session {
    type Result = ();

    fn handle(&mut self, msg: Message, ctx: &mut Self::Context) {
        ctx.text(msg.0);
    }
}

#[serde(tag = "result", rename_all = "kebab-case")]
#[derive(Debug, Serialize, Deserialize)]
enum WsStatus {
    Success {
        #[serde(skip_serializing_if = "Option::is_none")]
        response: Option<serde_json::Value>,
    },
    Error {
        description: String,
    },
}

impl StreamHandler<ws::Message, ws::ProtocolError> for Session {
    fn handle(&mut self, msg: ws::Message, ctx: &mut Self::Context) {
        match msg {
            ws::Message::Ping(msg) => ctx.pong(&msg),
            ws::Message::Close(_) => ctx.stop(),
            ws::Message::Text(ref text) => {
                let res = serde_json::from_str(text)
                    .map(|m| self.process_incoming_message(m))
                    .unwrap_or_else(|e| WsStatus::Error {
                        description: e.to_string(),
                    });
                ctx.text(serde_json::to_string(&res).unwrap());
            }
            _ => {}
        }
    }
}
