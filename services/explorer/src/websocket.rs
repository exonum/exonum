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
use exonum::{
    blockchain::{Block, Blockchain, Schema},
    crypto::Hash,
    merkledb::ObjectHash,
    messages::SignedMessage,
};
use exonum_explorer::api::{CommittedTransactionSummary, TransactionHex, TransactionResponse};
use futures::Future;
use hex::FromHex;
use rand::{rngs::ThreadRng, Rng};
use serde_derive::*;

use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    fmt,
    sync::{Arc, Mutex, Weak},
    time::Duration,
};

#[derive(Debug, Default)]
pub struct SharedState {
    inner: Arc<Mutex<SharedStateInner>>,
}

impl Drop for SharedState {
    fn drop(&mut self) {
        // If this is the last instance of the `SharedState`, send termination message
        // to the server.
        if Arc::strong_count(&self.inner) == 1 {
            if let Ok(inner) = self.inner.lock() {
                if let Some(ref addr) = inner.server_addr {
                    addr.do_send(Terminate);
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct SharedStateRef {
    inner: Weak<Mutex<SharedStateInner>>,
}

#[derive(Debug, Default)]
struct SharedStateInner {
    server_addr: Option<Addr<Server>>,
}

impl SharedState {
    pub fn get_ref(&self) -> SharedStateRef {
        SharedStateRef {
            inner: Arc::downgrade(&self.inner),
        }
    }

    pub fn broadcast_block(&self, block_hash: Hash) {
        let inner = self.inner.lock().expect("Cannot lock `SharedState`");
        if let Some(ref addr) = inner.server_addr {
            addr.do_send(Broadcast { block_hash });
        }
    }
}

impl SharedStateRef {
    /// Returns `None` if the service has shut down.
    pub fn ensure_server(&self, blockchain: &Blockchain) -> Option<Addr<Server>> {
        let arc = self.inner.upgrade()?;
        let mut inner = arc.lock().expect("Cannot lock `SharedState`");
        Some(
            inner
                .server_addr
                .get_or_insert_with(|| {
                    let blockchain = blockchain.to_owned();
                    Arbiter::start(|_| Server::new(blockchain))
                })
                .clone(),
        )
    }
}

/// Message coming from a websocket connection.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(tag = "type", content = "payload", rename_all = "kebab-case")]
enum IncomingMessage {
    /// Set subscription for websocket connection.
    SetSubscriptions(Vec<SubscriptionType>),
    /// Send transaction to blockchain.
    Transaction(TransactionHex),
}

/// Subscription type (new blocks or committed transactions).
#[derive(Debug, PartialEq, Eq, Hash, Clone, PartialOrd, Ord)]
#[derive(Serialize, Deserialize)]
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
#[derive(Debug, PartialEq, Eq, Hash, Clone, PartialOrd, Ord)]
#[derive(Serialize, Deserialize)]
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
enum Message {
    /// This message will send data to a client.
    Data(String),
    /// This message will terminate a client session.
    Close,
}

/// This message will terminate server.
#[derive(Message)]
struct Terminate;

#[derive(Message)]
#[rtype(u64)]
struct Subscribe {
    address: Recipient<Message>,
    subscriptions: Vec<SubscriptionType>,
}

#[derive(Message)]
struct Unsubscribe {
    pub id: u64,
}

#[derive(Message)]
struct UpdateSubscriptions {
    pub id: u64,
    pub subscriptions: Vec<SubscriptionType>,
}

#[derive(Message)]
struct Broadcast {
    block_hash: Hash,
}

#[derive(Message)]
#[rtype("Result<TransactionResponse, failure::Error>")]
struct Transaction {
    tx: TransactionHex,
}

pub struct Server {
    subscribers: BTreeMap<SubscriptionType, HashMap<u64, Recipient<Message>>>,
    blockchain: Blockchain,
    rng: RefCell<ThreadRng>,
}

impl fmt::Debug for Server {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Server")
            .field("subscribers", &self.subscribers.keys().collect::<Vec<_>>())
            .field("blockchain", &self.blockchain)
            .finish()
    }
}

impl Server {
    /// Wait to merge the block.
    const MERGE_WAIT: Duration = Duration::from_millis(20);

    fn new(blockchain: Blockchain) -> Self {
        Self {
            subscribers: BTreeMap::new(),
            blockchain,
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

    fn disconnect_all(&mut self) {
        for subscriber in self.subscribers.values_mut() {
            for recipient in subscriber.values_mut() {
                if let Err(err) = recipient.do_send(Message::Close) {
                    log::warn!(
                        "Can't send `Close` message to a websocket client: {:?}",
                        err
                    );
                }
            }
            subscriber.clear();
        }
        self.subscribers.clear();
    }
}

impl Actor for Server {
    type Context = Context<Self>;

    fn stopping(&mut self, _ctx: &mut Self::Context) -> Running {
        self.disconnect_all();
        Running::Stop
    }
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
            .flat_map(HashMap::iter)
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

    fn handle(&mut self, Broadcast { block_hash }: Broadcast, ctx: &mut Self::Context) {
        let snapshot = self.blockchain.snapshot();
        let schema = Schema::new(&snapshot);
        let block = match schema.blocks().get(&block_hash) {
            Some(block) => block,
            None => {
                // The block is not yet merged into the database, which can happen since
                // `after_commit` is called before the merge. Try again with a slight delay.
                ctx.notify_later(Broadcast { block_hash }, Self::MERGE_WAIT);
                return;
            }
        };
        let height = block.height;
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
                    log::error!(
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

    /// Broadcasts transaction if the check was passed, and returns an error otherwise.
    fn handle(
        &mut self,
        Transaction { tx }: Transaction,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        let msg = SignedMessage::from_hex(tx)?;
        let tx_hash = msg.object_hash();
        let verified = msg.into_verified()?;
        Blockchain::check_tx(&self.blockchain.snapshot(), &verified)?;

        // FIXME Don't ignore message error.
        let _ = self.blockchain.sender().broadcast_transaction(verified);
        Ok(TransactionResponse { tx_hash })
    }
}

impl Handler<Terminate> for Server {
    type Result = ();

    fn handle(&mut self, _msg: Terminate, ctx: &mut Self::Context) -> Self::Result {
        ctx.stop();
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
                let _ = addr.do_send(Message::Data(serialized.clone()));
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
    type Context = ws::WebsocketContext<Self, ()>;

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
        match msg {
            Message::Data(x) => ctx.text(x),
            Message::Close => {
                ctx.close(Some(ws::CloseReason {
                    code: ws::CloseCode::Normal,
                    description: Some("node shutdown".into()),
                }));
                ctx.stop();
                ctx.terminate();
            }
        }
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
