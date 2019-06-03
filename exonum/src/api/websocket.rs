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

use rand::{rngs::ThreadRng, Rng};

use futures::Future;

use std::{cell::RefCell, collections::HashMap, sync::Arc};

use crate::api::{
    node::public::explorer::{TransactionHex, TransactionResponse},
    ServiceApiState,
};
use crate::blockchain::{Block, Schema, TransactionResult, TxLocation};
use crate::crypto::Hash;
use crate::events::error::into_failure;
use crate::explorer::TxStatus;
use crate::messages::{
    Message as ExonumMessage, ProtocolMessage, RawTransaction, Signed, SignedMessage,
};
use crate::storage::{ListProof, Snapshot};

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(tag = "type", content = "payload", rename_all = "kebab-case")]
pub enum IncomingMessage {
    SetSubscriptions(Vec<SubscriptionType>),
    Transaction(TransactionHex),
}

/// Subscription type (new blocks or committed transactions).
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Clone)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum SubscriptionType {
    /// Subscription to nothing
    None,
    /// Subscription on new blocks
    Blocks,
    /// Subscription on committed transactions.
    Transactions { filter: Option<TransactionFilter> },
}

/// Describe filter for transactions by ID of service and (optionally)
/// transaction type in service.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Clone)]
pub struct TransactionFilter {
    pub service_id: u16,
    pub transaction_id: Option<u16>,
}

impl TransactionFilter {
    pub fn new(service_id: u16, transaction_id: Option<u16>) -> Self {
        Self {
            service_id,
            transaction_id,
        }
    }
}

/// Information about a particular transaction in the blockchain (without transaction content).
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct TransactionInfo {
    tx_hash: Hash,
    pub service_id: u16,
    pub transaction_id: u16,
    #[serde(with = "TxStatus")]
    status: TransactionResult,
    location: TxLocation,
    proof: ListProof<Hash>,
}

impl TransactionInfo {
    fn new<T>(schema: &Schema<T>, tx_hash: &Hash) -> Self
    where
        T: AsRef<dyn Snapshot>,
    {
        let tx = schema.transactions().get(tx_hash).unwrap();
        let service_id = tx.payload().service_id();
        let tx_id = tx.payload().transaction_id();
        let tx_result = schema.transaction_results().get(tx_hash).unwrap();
        let location = schema.transactions_locations().get(tx_hash).unwrap();
        let location_proof = schema
            .block_transactions(location.block_height())
            .get_proof(location.position_in_block());
        Self {
            tx_hash: *tx_hash,
            service_id,
            transaction_id: tx_id,
            status: tx_result,
            location,
            proof: location_proof,
        }
    }
}

/// Websocket notification message. This enum describe data, which is sent to
/// subscriber of websocket.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Notification {
    Block(Block),
    Transaction(TransactionInfo),
}

/// WebSocket message for communication between clients(`Session`) and server(`Server`).
#[derive(Message, Debug)]
pub(crate) struct Message(pub String);

#[derive(Message)]
#[rtype(usize)]
pub(crate) struct Subscribe {
    pub address: Recipient<Message>,
    pub sub_types: Vec<SubscriptionType>,
}

#[derive(Message)]
pub(crate) struct Unsubscribe {
    pub id: usize,
}

#[derive(Message)]
pub(crate) struct UpdateSubscriptions {
    pub id: usize,
    pub sub_types: Vec<SubscriptionType>,
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
    pub subscribers: HashMap<SubscriptionType, HashMap<usize, Recipient<Message>>>,
    service_api_state: Arc<ServiceApiState>,
    rng: RefCell<ThreadRng>,
}

impl Server {
    pub fn new(service_api_state: Arc<ServiceApiState>) -> Self {
        Self {
            subscribers: HashMap::new(),
            service_api_state,
            rng: RefCell::new(rand::thread_rng()),
        }
    }

    fn remove_subscriber(&mut self, id: usize) {
        self.subscribers.iter_mut().for_each(|(_, v)| {
            v.remove(&id);
        });
    }

    fn set_subscriptions(
        &mut self,
        id: usize,
        addr: Recipient<Message>,
        sub_types: Vec<SubscriptionType>,
    ) {
        sub_types.into_iter().for_each(|sub_type| {
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
    type Result = usize;

    fn handle(
        &mut self,
        Subscribe { address, sub_types }: Subscribe,
        _ctx: &mut Self::Context,
    ) -> usize {
        let id = self.rng.borrow_mut().gen::<usize>();
        self.set_subscriptions(id, address, sub_types);

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
        UpdateSubscriptions { id, sub_types }: UpdateSubscriptions,
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
        self.set_subscriptions(id, addr, sub_types);
    }
}

impl Handler<Broadcast> for Server {
    type Result = ();

    fn handle(&mut self, Broadcast { block_hash }: Broadcast, _ctx: &mut Self::Context) {
        let snapshot = self.service_api_state.snapshot();
        let schema = Schema::new(snapshot);
        let block = schema.blocks().get(&block_hash).unwrap();
        let height = block.height();
        let block_header = Notification::Block(block);

        // Notify about block
        self.broadcast_message(SubscriptionType::Blocks, &block_header);

        // Get list of transactions in block and notify about each of them.
        let tx_hashes_table = schema.block_transactions(height);
        tx_hashes_table
            .iter()
            .map(|hash| TransactionInfo::new(&schema, &hash))
            .for_each(|tx_info| {
                let service_id = tx_info.service_id;
                let tx_id = tx_info.transaction_id;
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
        let signed = SignedMessage::from_raw_buffer(buf)?;
        let tx_hash = signed.hash();
        let signed = RawTransaction::try_from(ExonumMessage::deserialize(signed)?)
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
    pub id: usize,
    pub sub_types: Vec<SubscriptionType>,
    pub server_address: Addr<Server>,
}

impl Session {
    pub fn new(server_address: Addr<Server>, sub_types: Vec<SubscriptionType>) -> Self {
        Self {
            id: 0,
            server_address,
            sub_types,
        }
    }

    fn process_incoming_message(&mut self, msg: IncomingMessage) -> WsStatus {
        match msg {
            IncomingMessage::SetSubscriptions(subs) => self.set_subscriptions(subs),
            IncomingMessage::Transaction(tx) => self.send_transaction(tx),
        }
    }

    fn set_subscriptions(&mut self, sub_types: Vec<SubscriptionType>) -> WsStatus {
        self.sub_types = sub_types.clone();
        self.server_address
            .try_send(UpdateSubscriptions {
                id: self.id,
                sub_types,
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
                sub_types: self.sub_types.clone(),
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
