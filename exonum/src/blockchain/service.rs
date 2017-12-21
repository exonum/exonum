// Copyright 2017 The Exonum Team
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

//! This module defines the Exonum services interfaces. Like smart contracts in some other
//! blockchain platforms, Exonum services encapsulate business logic of the blockchain application.

use std::fmt;
use std::sync::{Arc, RwLock};
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;

use serde_json::Value;
use iron::Handler;

use crypto::{Hash, PublicKey, SecretKey};
use storage::{Fork, Snapshot};
use messages::{Message, RawTransaction};
use encoding::Error as MessageError;
use node::{ApiSender, Node, State, TransactionSend};
use blockchain::{Blockchain, ConsensusConfig, Schema, StoredConfiguration, ValidatorKeys};
use helpers::{Height, Milliseconds, ValidatorId};

/// A trait that describes transaction processing rules (a group of sequential operations
/// with the Exonum storage) for the given `Message`.
pub trait Transaction: Message + 'static {
    /// Verifies the transaction, which includes the message signature verification and other
    /// specific internal constraints. verify is intended to check the internal consistency of
    /// a transaction; it has no access to the blockchain state.
    /// If a transaction fails verify, it is considered incorrect and cannot be included into
    /// any correct block proposal. Incorrect transactions are never included into the blockchain.
    ///
    /// *This method should not use external data, that is, it must be a pure function.*
    fn verify(&self) -> bool;
    /// Takes the current blockchain state via `fork` and can modify it if certain conditions
    /// are met.
    ///
    /// # Notes
    ///
    /// - When programming `execute`, you should perform state-related checks before any changes
    /// to the state and return early if these checks fail.
    /// - If the execute method of a transaction raises a `panic`, the changes made by the
    /// transactions are discarded, but the transaction itself is still considered committed.
    fn execute(&self, fork: &mut Fork);
    /// Returns the useful information about the transaction in the JSON format. The returned value
    /// is used to fill the [`TxInfo.content`] field in [the blockchain explorer][explorer].
    ///
    /// # Notes
    ///
    /// The default implementation returns `null`. For transactions defined with
    /// the [`message!`] macro, you may redefine `info()` as
    ///
    /// ```
    /// # #[macro_use] extern crate exonum;
    /// extern crate serde_json;
    /// # use exonum::blockchain::Transaction;
    /// # use exonum::storage::Fork;
    ///
    /// message! {
    ///     struct MyTransaction {
    ///         // Transaction definition...
    /// #       const TYPE = 1;
    /// #       const ID = 1;
    /// #       const SIZE = 8;
    /// #       field foo: u64 [0 => 8]
    ///     }
    /// }
    ///
    /// impl Transaction for MyTransaction {
    ///     // Other methods...
    /// #   fn verify(&self) -> bool { true }
    /// #   fn execute(&self, _: &mut Fork) { }
    ///
    ///     fn info(&self) -> serde_json::Value {
    ///         serde_json::to_value(self).expect("Cannot serialize transaction to JSON")
    ///     }
    /// }
    /// # fn main() { }
    /// ```
    ///
    /// [`TxInfo.content`]: ../explorer/struct.TxInfo.html#structfield.content
    /// [explorer]: ../explorer/index.html
    /// [`message!`]: ../macro.message.html
    fn info(&self) -> Value {
        Value::Null
    }
}

/// A trait that describes a business-logic of the concrete service.
#[allow(unused_variables, unused_mut)]
pub trait Service: Send + Sync + 'static {
    /// Unique service identification for database schema and service messages.
    fn service_id(&self) -> u16;

    /// Unique human readable service name.
    fn service_name(&self) -> &'static str;

    /// Returns a list of root hashes of tables that determine the current state
    /// of the service database. These hashes are collected from all services in a common
    ///  `MerklePatriciaTable` that named [`state_hash_aggregator`][1].
    ///
    /// See also [`service_table_unique_key`][2].
    ///
    /// [1]: struct.Schema.html#method.state_hash_aggregator
    /// [2]: struct.Blockchain.html#method.service_table_unique_key
    fn state_hash(&self, _: &Snapshot) -> Vec<Hash> {
        Vec::new()
    }

    /// Tries to create `Transaction` object from the given raw message.
    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, MessageError>;

    /// By this method you can initialize information schema of service
    /// and generates initial service configuration.
    /// This method is called on genesis block creation event.
    fn initialize(&self, fork: &mut Fork) -> Value {
        Value::Null
    }

    /// Handles commit event. This handler is invoked for each service after commit of the block.
    /// For example service can create some transaction if the specific condition occurred.
    ///
    /// *Try not to perform long operations here*.
    fn handle_commit(&self, context: &ServiceContext) {}

    /// Returns api handler for public users.
    fn public_api_handler(&self, context: &ApiContext) -> Option<Box<Handler>> {
        None
    }

    /// Returns api handler for maintainers.
    fn private_api_handler(&self, context: &ApiContext) -> Option<Box<Handler>> {
        None
    }
}

/// The current node state on which the blockchain is running, or in other words
/// execution context.
#[derive(Debug)]
pub struct ServiceContext {
    validator_id: Option<ValidatorId>,
    service_keypair: (PublicKey, SecretKey),
    api_sender: ApiSender,
    fork: Fork,
    stored_configuration: StoredConfiguration,
    height: Height,
}

impl ServiceContext {
    /// Creates the service context for the given node.
    ///
    /// This method is necessary if you want to implement an alternative exonum node.
    /// For example, you can implement special node without consensus for regression
    /// testing of services business logic.
    pub fn new(
        service_public_key: PublicKey,
        service_secret_key: SecretKey,
        api_sender: ApiSender,
        fork: Fork,
    ) -> ServiceContext {
        let (stored_configuration, height) = {
            let schema = Schema::new(fork.as_ref());
            let stored_configuration = schema.actual_configuration();
            let height = schema.height();
            (stored_configuration, height)
        };
        let validator_id = stored_configuration
            .validator_keys
            .iter()
            .position(|validator| service_public_key == validator.service_key)
            .map(|id| ValidatorId(id as u16));

        ServiceContext {
            validator_id,
            service_keypair: (service_public_key, service_secret_key),
            api_sender,
            fork,
            stored_configuration,
            height,
        }
    }

    /// If the current node is validator returns its identifier.
    /// For other nodes return `None`.
    pub fn validator_id(&self) -> Option<ValidatorId> {
        self.validator_id
    }

    /// Returns the current database snapshot.
    pub fn snapshot(&self) -> &Snapshot {
        self.fork.as_ref()
    }

    /// Returns the current blockchain height. This height is "height of the last committed block".
    pub fn height(&self) -> Height {
        self.height
    }

    /// Returns the current list of validators.
    pub fn validators(&self) -> &[ValidatorKeys] {
        self.stored_configuration.validator_keys.as_slice()
    }

    /// Returns current node's public key.
    pub fn public_key(&self) -> &PublicKey {
        &self.service_keypair.0
    }

    /// Returns current node's secret key.
    pub fn secret_key(&self) -> &SecretKey {
        &self.service_keypair.1
    }

    /// Returns the actual consensus configuration.
    pub fn actual_consensus_config(&self) -> &ConsensusConfig {
        &self.stored_configuration.consensus
    }

    /// Returns service specific global variables as json value.
    pub fn actual_service_config(&self, service: &Service) -> &Value {
        &self.stored_configuration.services[service.service_name()]
    }

    /// Returns reference to the transaction sender.
    pub fn transaction_sender(&self) -> &TransactionSend {
        &self.api_sender
    }

    /// Returns the actual blockchain global configuration.
    pub fn stored_configuration(&self) -> &StoredConfiguration {
        &self.stored_configuration
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PeerInfo {
    pub public_key: PublicKey,
    pub height: Option<Height>,
}

#[derive(Debug, Default)]
pub struct ApiNodeState {
    incoming_connections: HashSet<SocketAddr>,
    outgoing_connections: HashSet<SocketAddr>,
    reconnects_timeout: HashMap<SocketAddr, Milliseconds>,
    //TODO: update on event?
    peers_info: HashMap<SocketAddr, PeerInfo>,
}

impl ApiNodeState {
    fn new() -> ApiNodeState {
        Self::default()
    }
}

/// Shared part of the context, used to take some values from the `Node`s `State`
/// should be used to take some metrics.
#[derive(Clone, Debug)]
pub struct SharedNodeState {
    state: Arc<RwLock<ApiNodeState>>,
    /// Timeout to update api state.
    pub state_update_timeout: Milliseconds,
}

impl SharedNodeState {
    /// Creates new `SharedNodeState`
    pub fn new(state_update_timeout: Milliseconds) -> SharedNodeState {
        SharedNodeState {
            state: Arc::new(RwLock::new(ApiNodeState::new())),
            state_update_timeout,
        }
    }
    /// Return list of connected sockets
    pub fn incoming_connections(&self) -> Vec<SocketAddr> {
        self.state
            .read()
            .expect("Expected read lock.")
            .incoming_connections
            .iter()
            .cloned()
            .collect()
    }
    /// Return list of our connection sockets
    pub fn outgoing_connections(&self) -> Vec<SocketAddr> {
        self.state
            .read()
            .expect("Expected read lock.")
            .outgoing_connections
            .iter()
            .cloned()
            .collect()
    }
    /// Return reconnects list
    pub fn reconnects_timeout(&self) -> Vec<(SocketAddr, Milliseconds)> {
        self.state
            .read()
            .expect("Expected read lock.")
            .reconnects_timeout
            .iter()
            .map(|(c, e)| (*c, *e))
            .collect()
    }

    /// Return peers info list
    pub fn peers_info(&self) -> Vec<(SocketAddr, PeerInfo)> {
        self.state
            .read()
            .expect("Expected read lock.")
            .peers_info
            .iter()
            .map(|(&c, &info)| (c, info))
            .collect()
    }

    /// Update internal state, from `Node` State`
    pub fn update_node_state(&self, state: &State) {
        for (&public_key, c) in state.peers().iter() {
            let height = state.node_height(&public_key);
            let height = if height == Height::zero() {
                None
            } else {
                Some(height)
            };
            let mut lock = self.state.write().expect("Expected write lock.");
            lock.peers_info.insert(c.addr(), PeerInfo {
                height,
                public_key,
            });
        }
    }

    /// Returns value of the `state_update_timeout`.
    pub fn state_update_timeout(&self) -> Milliseconds {
        self.state_update_timeout
    }

    /// add incomming connection into state
    pub fn add_incoming_connection(&self, addr: SocketAddr) {
        self.state
            .write()
            .expect("Expected write lock")
            .incoming_connections
            .insert(addr);
    }
    /// add outgoing connection into state
    pub fn add_outgoing_connection(&self, addr: SocketAddr) {
        self.state
            .write()
            .expect("Expected write lock")
            .outgoing_connections
            .insert(addr);
    }

    /// remove incomming connection from state
    pub fn remove_incoming_connection(&self, addr: &SocketAddr) -> bool {
        self.state
            .write()
            .expect("Expected write lock")
            .incoming_connections
            .remove(addr)
    }

    /// remove outgoing connection from state
    pub fn remove_outgoing_connection(&self, addr: &SocketAddr) -> bool {
        self.state
            .write()
            .expect("Expected write lock")
            .outgoing_connections
            .remove(addr)
    }

    /// Add reconect timeout
    pub fn add_reconnect_timeout(
        &self,
        addr: SocketAddr,
        timeout: Milliseconds,
    ) -> Option<Milliseconds> {
        self.state
            .write()
            .expect("Expected write lock")
            .reconnects_timeout
            .insert(addr, timeout)
    }

    /// Removes reconect timeout and returns the previous value.
    pub fn remove_reconnect_timeout(&self, addr: &SocketAddr) -> Option<Milliseconds> {
        self.state
            .write()
            .expect("Expected write lock")
            .reconnects_timeout
            .remove(addr)
    }
}

/// Provides the current node state to api handlers.
pub struct ApiContext {
    blockchain: Blockchain,
    node_channel: ApiSender,
    public_key: PublicKey,
    secret_key: SecretKey,
}

/// Provides the current node state to api handlers.
impl ApiContext {
    /// Constructs context for the given `Node`.
    pub fn new(node: &Node) -> ApiContext {
        let handler = node.handler();
        ApiContext {
            blockchain: handler.blockchain.clone(),
            node_channel: node.channel(),
            public_key: *node.state().service_public_key(),
            secret_key: node.state().service_secret_key().clone(),
        }
    }

    /// Constructs context from raw parts.
    pub fn from_parts(
        blockchain: &Blockchain,
        node_channel: ApiSender,
        public_key: &PublicKey,
        secret_key: &SecretKey,
    ) -> ApiContext {
        ApiContext {
            blockchain: blockchain.clone(),
            node_channel,
            public_key: *public_key,
            secret_key: secret_key.clone(),
        }
    }

    /// Returns reference to the node's blockchain.
    pub fn blockchain(&self) -> &Blockchain {
        &self.blockchain
    }

    /// Returns reference to the transaction sender.
    pub fn node_channel(&self) -> &ApiSender {
        &self.node_channel
    }

    /// Returns the public key of current node.
    pub fn public_key(&self) -> &PublicKey {
        &self.public_key
    }

    /// Returns the secret key of current node.
    pub fn secret_key(&self) -> &SecretKey {
        &self.secret_key
    }
}

impl ::std::fmt::Debug for ApiContext {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "ApiContext(blockchain: {:?}, public_key: {:?})",
            self.blockchain,
            self.public_key
        )
    }
}

impl<'a, S: Service> From<S> for Box<Service + 'a> {
    fn from(s: S) -> Self {
        Box::new(s) as Box<Service>
    }
}

impl<'a, T: Transaction> From<T> for Box<Transaction + 'a> {
    fn from(tx: T) -> Self {
        Box::new(tx) as Box<Transaction>
    }
}
