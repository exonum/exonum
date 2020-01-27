// Copyright 2020 The Exonum Team
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

//! WebSocket API of the explorer service.
//!
//! # Overview
//!
//! All communication via WebSockets uses JSON encoding.
//!
//! The API follows the publisher-subscriber pattern. Clients can subscribe to events. There are
//! two types of events encapsulated in [`Notification`]:
//!
//! - block creation
//! - commitment of a transaction
//!
//! Subscription types are encapsulated in [`SubscriptionType`]. A single client may have
//! multiple subscriptions.
//!
//! Besides pub-sub, clients may send signed transactions wrapped in [`TransactionHex`]. A client
//! should set subscriptions and send transactions using [`IncomingMessage`] type. The server
//! responds to each `IncomingMessage` with a [`Response`], which
//! wraps the response type (`()` for subscriptions, [`TransactionResponse`] for transactions).
//!
//! There are three WS endpoints, which differ by the initial subscription for the client:
//!
//! - `api/explorer/v1/ws` does not set any subscriptions
//! - `api/explorer/v1/blocks/subscribe` sets subscription to blocks
//! - `api/explorer/v1/transactions/subscribe` sets subscription to transactions. The parameters
//!   of the subscription are encoded in the query as [`TransactionFilter`]
//!
//! [`IncomingMessage`]: enum.IncomingMessage.html
//! [`Response`]: enum.Response.html
//! [`Notification`]: enum.Notification.html
//! [`SubscriptionType`]: enum.SubscriptionType.html
//! [`TransactionHex`]: ../struct.TransactionHex.html
//! [`TransactionResponse`]: ../struct.TransactionResponse.html
//! [`TransactionFilter`]: struct.TransactionFilter.html
//!
//! # Examples
//!
//! Connecting to generic endpoint and setting a subscription:
//!
//! ```
//! # use assert_matches::assert_matches;
//! # use exonum_explorer_service::ExplorerFactory;
//! # use exonum_explorer_service::api::websocket::{
//! #     IncomingMessage, Response, SubscriptionType, Notification,
//! # };
//! # use exonum_testkit::TestKitBuilder;
//! # use std::time::Duration;
//! use websocket::OwnedMessage;
//!
//! fn stringify(data: &impl serde::Serialize) -> OwnedMessage {
//!     OwnedMessage::Text(serde_json::to_string(data).unwrap())
//! }
//!
//! fn parse<T: serde::de::DeserializeOwned>(data: OwnedMessage) -> T {
//!     match data {
//!         OwnedMessage::Text(ref s) => serde_json::from_str(s).unwrap(),
//!         _ => panic!("Unexpected message"),
//!     }
//! }
//!
//! # fn main() -> Result<(), failure::Error> {
//! let mut testkit = TestKitBuilder::validator()
//!     .with_default_rust_service(ExplorerFactory)
//!     .build();
//! let api = testkit.api();
//! let url = api.public_url("api/explorer/v1/ws");
//! let mut client = websocket::ClientBuilder::new(&url)?.connect_insecure()?;
//! # client.stream_ref().set_read_timeout(Some(Duration::from_secs(1)))?;
//!
//! // Send a subscription message.
//! let subscription = SubscriptionType::Blocks;
//! let message = IncomingMessage::SetSubscriptions(vec![subscription]);
//! client.send_message(&stringify(&message))?;
//! // The server should respond with an empty response.
//! let response: Response<()> = parse(client.recv_message()?);
//! assert_matches!(response, Response::Success { .. });
//!
//! // Create a block and check that it is received by the client.
//! let block = testkit.create_block();
//! let response = parse::<Notification>(client.recv_message()?);
//! assert_matches!(
//!     response,
//!     Notification::Block(ref header) if *header == block.header
//! );
//! # Ok(())
//! # }
//! ```
//!
//! Sending a transaction and receiving it as a notification:
//!
//! ```
//! # use assert_matches::assert_matches;
//! # use exonum::{crypto::gen_keypair, runtime::ExecutionError};
//! # use exonum_rust_runtime::{ExecutionContext, DefaultInstance, Service, ServiceFactory};
//! # use exonum_derive::*;
//! # use exonum_explorer_service::ExplorerFactory;
//! # use exonum_explorer_service::api::{
//! #     websocket::{IncomingMessage, Response, SubscriptionType, Notification},
//! #     TransactionHex, TransactionResponse,
//! # };
//! # use exonum_testkit::TestKitBuilder;
//! # use std::time::Duration;
//! # use websocket::OwnedMessage;
//! // `stringify` and `parse` functions are defined as in the previous example.
//! # fn stringify(data: &impl serde::Serialize) -> OwnedMessage {
//! #     OwnedMessage::Text(serde_json::to_string(data).unwrap())
//! # }
//! # fn parse<T: serde::de::DeserializeOwned>(data: OwnedMessage) -> T {
//! #     match data {
//! #         OwnedMessage::Text(ref s) => serde_json::from_str(s).unwrap(),
//! #         _ => panic!("Unexpected message"),
//! #     }
//! # }
//!
//! #[exonum_interface]
//! trait ServiceInterface<Ctx> {
//!     type Output;
//!     #[interface_method(id = 0)]
//!     fn do_nothing(&self, ctx: Ctx, _seed: u32) -> Self::Output;
//! }
//!
//! #[derive(Debug, ServiceDispatcher, ServiceFactory)]
//! # #[service_factory(artifact_name = "my-service")]
//! #[service_dispatcher(implements("ServiceInterface"))]
//! struct MyService;
//! // Some implementations skipped for `MyService`...
//! # impl ServiceInterface<ExecutionContext<'_>> for MyService {
//! #    type Output = Result<(), ExecutionError>;
//! #    fn do_nothing(&self, ctx: ExecutionContext<'_>, _seed: u32) -> Self::Output { Ok(()) }
//! # }
//! # impl DefaultInstance for MyService {
//! #     const INSTANCE_ID: u32 = 100;
//! #     const INSTANCE_NAME: &'static str = "my-service";
//! # }
//! # impl Service for MyService {}
//!
//! # fn main() -> Result<(), failure::Error> {
//! let mut testkit = TestKitBuilder::validator()
//!    .with_default_rust_service(ExplorerFactory)
//!    .with_default_rust_service(MyService)
//!    .build();
//! let api = testkit.api();
//!
//! // Signal that we want to receive notifications about `MyService` transactions.
//! let url = format!(
//!     "api/explorer/v1/transactions/subscribe?instance_id={}",
//!     MyService::INSTANCE_ID
//! );
//! let mut client = websocket::ClientBuilder::new(&api.public_url(&url))?
//!     .connect_insecure()?;
//! # client.stream_ref().set_read_timeout(Some(Duration::from_secs(1)))?;
//!
//! // Create a transaction and send it via WS.
//! let tx = gen_keypair().do_nothing(MyService::INSTANCE_ID, 0);
//! let tx_hex = TransactionHex::new(&tx);
//! let message = IncomingMessage::Transaction(tx_hex);
//! client.send_message(&stringify(&message))?;
//!
//! // Receive a notification that the transaction was successfully accepted
//! // into the memory pool.
//! let res: Response<TransactionResponse> = parse(client.recv_message()?);
//! let response = res.into_result().unwrap();
//! let tx_hash = response.tx_hash;
//!
//! // Create a block.
//! let block = testkit.create_block();
//! assert_eq!(block.len(), 1); // The block contains the sent transaction.
//!
//! // Receive a notification about the committed transaction.
//! let notification = parse::<Notification>(client.recv_message()?);
//! assert_matches!(
//!     notification,
//!     Notification::Transaction(ref summary) if summary.tx_hash == tx_hash
//! );
//! # Ok(())
//! # }
//! ```

pub use exonum_explorer::api::websocket::{
    CommittedTransactionSummary, IncomingMessage, Notification, Response, SubscriptionType,
    TransactionFilter,
};

use actix::*;
use actix_web::ws;
use exonum::{
    blockchain::{Blockchain, Schema},
    crypto::Hash,
    merkledb::ObjectHash,
    messages::{AnyTx, SignedMessage, Verified},
};
use exonum_explorer::api::{TransactionHex, TransactionResponse};
use futures::{Future, IntoFuture};
use hex::FromHex;

use std::{
    collections::{BTreeMap, HashMap},
    fmt, mem,
    sync::{Arc, Mutex, Weak},
    time::Duration,
};

mod glue;

#[derive(Debug, Default)]
pub(crate) struct SharedState {
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
pub(crate) struct SharedStateRef {
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
        let addr = inner.server_addr.get_or_insert_with(|| {
            let blockchain = blockchain.to_owned();
            Arbiter::start(|_| Server::new(blockchain))
        });
        Some(addr.clone())
    }
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
#[derive(Debug, Message)]
struct Terminate;

#[derive(Message)]
#[rtype(u64)]
struct Subscribe {
    address: Recipient<Message>,
    subscriptions: Vec<SubscriptionType>,
}

impl fmt::Debug for Subscribe {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Subscribe")
            .field("subscriptions", &self.subscriptions)
            .finish()
    }
}

#[derive(Debug, Message)]
struct Unsubscribe {
    id: u64,
}

#[derive(Debug, Message)]
struct UpdateSubscriptions {
    id: u64,
    subscriptions: Vec<SubscriptionType>,
}

#[derive(Debug, Message)]
struct Broadcast {
    block_hash: Hash,
}

#[derive(Debug, Message)]
#[rtype("Result<TransactionResponse, failure::Error>")]
struct Transaction(TransactionHex);

pub(crate) struct Server {
    subscribers: BTreeMap<SubscriptionType, HashMap<u64, Recipient<Message>>>,
    blockchain: Blockchain,
    next_id: u64,
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
    /// Wait interval to merge the block.
    const MERGE_WAIT: Duration = Duration::from_millis(20);

    fn new(blockchain: Blockchain) -> Self {
        Self {
            subscribers: BTreeMap::new(),
            blockchain,
            next_id: 0,
        }
    }

    fn remove_subscriber(&mut self, id: u64) {
        for subscriber_group in self.subscribers.values_mut() {
            subscriber_group.remove(&id);
        }
    }

    fn set_subscriptions(
        &mut self,
        id: u64,
        addr: Recipient<Message>,
        subscriptions: Vec<SubscriptionType>,
    ) {
        for sub_type in subscriptions {
            self.subscribers
                .entry(sub_type)
                .or_insert_with(HashMap::new)
                .insert(id, addr.clone());
        }
    }

    fn disconnect_all(&mut self) {
        let subscribers = mem::replace(&mut self.subscribers, BTreeMap::new());
        for (_, subscriber_group) in subscribers {
            for (_, recipient) in subscriber_group {
                if let Err(err) = recipient.do_send(Message::Close) {
                    log::warn!(
                        "Can't send `Close` message to a websocket client: {:?}",
                        err
                    );
                }
            }
        }
    }

    fn check_transaction(&self, message: Transaction) -> Result<Verified<AnyTx>, failure::Error> {
        let signed = SignedMessage::from_hex(message.0.tx_body.as_bytes())?;
        let verified = signed.into_verified()?;
        Blockchain::check_tx(&self.blockchain.snapshot(), &verified)?;
        Ok(verified)
    }

    fn handle_transaction(
        &self,
        message: Transaction,
    ) -> impl Future<Item = TransactionResponse, Error = failure::Error> {
        let sender = self.blockchain.sender().to_owned();
        self.check_transaction(message)
            .into_future()
            .and_then(move |verified| {
                let tx_hash = verified.object_hash();
                sender
                    .broadcast_transaction(verified)
                    .map(move |()| TransactionResponse { tx_hash })
                    .from_err()
            })
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

    fn handle(&mut self, message: Subscribe, _ctx: &mut Self::Context) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.set_subscriptions(id, message.address, message.subscriptions);
        id
    }
}

impl Handler<Unsubscribe> for Server {
    type Result = ();

    fn handle(&mut self, message: Unsubscribe, _ctx: &mut Self::Context) {
        self.remove_subscriber(message.id);
    }
}

impl Handler<UpdateSubscriptions> for Server {
    type Result = ();

    fn handle(&mut self, message: UpdateSubscriptions, _ctx: &mut Self::Context) {
        // Find address of subscriber. If id not found, assume that subscriber doesn't exist
        // and return.
        let maybe_addr =
            self.subscribers
                .values()
                .flat_map(HashMap::iter)
                .find_map(|(id, addr)| {
                    if *id == message.id {
                        Some(addr.clone())
                    } else {
                        None
                    }
                });
        let addr = if let Some(addr) = maybe_addr {
            addr
        } else {
            return;
        };
        self.remove_subscriber(message.id);
        self.set_subscriptions(message.id, addr, message.subscriptions);
    }
}

impl Handler<Broadcast> for Server {
    type Result = ();

    fn handle(&mut self, message: Broadcast, ctx: &mut Self::Context) {
        let snapshot = self.blockchain.snapshot();
        let schema = Schema::new(&snapshot);
        let block = match schema.blocks().get(&message.block_hash) {
            Some(block) => block,
            None => {
                // The block is not yet merged into the database, which can happen since
                // `after_commit` is called before the merge. Try again with a slight delay.
                ctx.notify_later(message, Self::MERGE_WAIT);
                return;
            }
        };
        let height = block.height;
        let block_header = Notification::Block(block);

        // Notify about block
        self.broadcast_message(SubscriptionType::Blocks, &block_header);

        // Get list of transactions in block and notify about each of them.
        let tx_hashes_table = schema.block_transactions(height);
        let tx_infos = tx_hashes_table.iter().map(|hash| {
            CommittedTransactionSummary::new(&schema, &hash).unwrap_or_else(|| {
                panic!(
                    "BUG. Cannot build summary about committed transaction {:?} \
                     because it doesn't exist in \"transactions\", \
                     \"transaction_results\" nor \"transactions_locations\" indexes.",
                    hash
                );
            })
        });

        for tx_info in tx_infos {
            let instance_id = tx_info.instance_id;
            let method_id = tx_info.method_id;
            let data = Notification::Transaction(tx_info);
            self.broadcast_message(SubscriptionType::Transactions { filter: None }, &data);
            self.broadcast_message(
                SubscriptionType::Transactions {
                    filter: Some(TransactionFilter::new(instance_id, None)),
                },
                &data,
            );
            self.broadcast_message(
                SubscriptionType::Transactions {
                    filter: Some(TransactionFilter::new(instance_id, Some(method_id))),
                },
                &data,
            );
        }
    }
}

impl Handler<Transaction> for Server {
    type Result = Box<dyn Future<Item = TransactionResponse, Error = failure::Error>>;

    /// Broadcasts transaction if the check was passed, and returns an error otherwise.
    fn handle(&mut self, message: Transaction, _ctx: &mut Self::Context) -> Self::Result {
        Box::new(self.handle_transaction(message))
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
        let subscriber_group = self
            .subscribers
            .entry(sub_type)
            .or_insert_with(HashMap::new);

        let serialized = serde_json::to_string(data).unwrap();
        for addr in subscriber_group.values() {
            addr.do_send(Message::Data(serialized.clone())).ok();
        }
    }
}

pub(crate) struct Session {
    id: u64,
    subscriptions: Vec<SubscriptionType>,
    server_address: Addr<Server>,
}

impl Session {
    pub fn new(server_address: Addr<Server>, subscriptions: Vec<SubscriptionType>) -> Self {
        Self {
            id: 0,
            server_address,
            subscriptions,
        }
    }

    fn process_incoming_message(&mut self, msg: IncomingMessage) -> String {
        match msg {
            IncomingMessage::SetSubscriptions(subs) => self.set_subscriptions(subs),
            IncomingMessage::Transaction(tx) => self.send_transaction(tx),
        }
    }

    fn set_subscriptions(&mut self, subscriptions: Vec<SubscriptionType>) -> String {
        self.subscriptions = subscriptions.clone();
        let response = self
            .server_address
            .try_send(UpdateSubscriptions {
                id: self.id,
                subscriptions,
            })
            .map(|_| Response::success(()))
            .unwrap_or_else(Response::error);
        serde_json::to_string(&response).unwrap()
    }

    fn send_transaction(&mut self, tx: TransactionHex) -> String {
        let response = self
            .server_address
            .send(Transaction(tx))
            .wait()
            .map(|res| {
                let res = res.map_err(|e| e.to_string());
                Response::from(res)
            })
            .unwrap_or_else(Response::error);
        serde_json::to_string(&response).unwrap()
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
                    code: ws::CloseCode::Away,
                    description: Some("Explorer service shut down".into()),
                }));
                ctx.stop();
                ctx.terminate();
            }
        }
    }
}

impl StreamHandler<ws::Message, ws::ProtocolError> for Session {
    fn handle(&mut self, msg: ws::Message, ctx: &mut Self::Context) {
        match msg {
            ws::Message::Ping(msg) => ctx.pong(&msg),
            ws::Message::Close(_) => ctx.stop(),
            ws::Message::Text(ref text) => {
                let response = serde_json::from_str(text)
                    .map(|msg| self.process_incoming_message(msg))
                    .unwrap_or_else(|err| {
                        let err = Response::<()>::error(err);
                        serde_json::to_string(&err).unwrap()
                    });
                ctx.text(response);
            }
            _ => {}
        }
    }
}
