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
use encoding::serialize::json::ExonumJson;
use node::{ApiSender, Node, State, TransactionSend};
use blockchain::{Blockchain, ConsensusConfig, Schema, StoredConfiguration, ValidatorKeys};
use helpers::{Height, Milliseconds, ValidatorId};

/// Transaction processing functionality for `Message`s allowing to apply authenticated, atomic,
/// constraint-preserving groups of changes to the blockchain storage.
///
/// See also [the documentation page on transactions][doc:transactions].
///
/// [doc:transactions]: https://exonum.com/doc/architecture/transactions/
pub trait Transaction: Message + ExonumJson + 'static {
    /// Verifies the internal consistency of the transaction. `verify` should usually include
    /// checking the message signature (via [`verify_signature`]) and, possibly,
    /// other internal constraints. `verify` has no access to the blockchain state;
    /// checks involving the blockchains state must be preformed in [`execute`](#tymethod.execute).
    ///
    /// If a transaction fails `verify`, it is considered incorrect and cannot be included into
    /// any correct block proposal. Incorrect transactions are never included into the blockchain.
    ///
    /// *This method should not use external data, that is, it must be a pure function.*
    ///
    /// [`verify_signature`]: ../messages/trait.Message.html#method.verify_signature
    fn verify(&self) -> bool;
    /// Receives a fork of the current blockchain state and can modify it depending on the contents
    /// of the transaction.
    ///
    /// # Notes
    ///
    /// - When programming `execute`, you should perform state-related checks before any changes
    ///   to the state and return early if these checks fail.
    /// - If the execute method of a transaction raises a panic, the changes made by the
    ///   transaction are discarded, but it is still considered committed.
    fn execute(&self, fork: &mut Fork);
}

/// A trait that describes business logic of a concrete service.
///
/// See also [the documentation page on services][doc:services].
///
/// # Examples
///
/// The following example provides a barebone foundation for implementing a service.
///
/// ```
/// #[macro_use] extern crate exonum;
/// // Exports from `exonum` crate skipped
/// # use exonum::blockchain::Service;
/// # use exonum::crypto::Hash;
/// # use exonum::blockchain::Transaction;
/// # use exonum::messages::{Message, RawTransaction};
/// # use exonum::storage::{Fork, Snapshot};
/// use exonum::encoding::Error as EncError;
///
/// // Reused constants
/// const SERVICE_ID: u16 = 8000;
/// const MY_TRANSACTION_ID: u16 = 1;
///
/// // Service schema
/// struct MyServiceSchema<T> {
///     view: T,
/// }
///
/// impl<T: AsRef<Snapshot>> MyServiceSchema<T> {
///     fn new(view: T) -> Self {
///         MyServiceSchema { view }
///     }
///
///     fn state_hash(&self) -> Vec<Hash> {
///         // Calculates the shate hash of the service
/// #       vec![]
///     }
///     // Other read-only methods
/// }
///
/// impl<'a> MyServiceSchema<&'a mut Fork> {
///     // Additional read-write methods
/// }
///
/// // Transaction definitions
/// message! {
///     struct MyTransaction {
///         const TYPE = SERVICE_ID;
///         const ID = MY_TRANSACTION_ID;
///         // Transaction fields
///     }
/// }
///
/// impl Transaction for MyTransaction {
///     // Business logic implementation
/// #   fn verify(&self) -> bool { true }
/// #   fn execute(&self, fork: &mut Fork) { }
/// }
///
/// // Service
/// struct MyService {}
///
/// impl Service for MyService {
///     fn service_id(&self) -> u16 {
///        SERVICE_ID
///     }
///
///     fn service_name(&self) -> &'static str {
///         "my_special_unique_service"
///     }
///
///     fn state_hash(&self, snapshot: &Snapshot) -> Vec<Hash> {
///         MyServiceSchema::new(snapshot).state_hash()
///     }
///
///     fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, EncError> {
///         let tx: Box<Transaction> = match raw.message_type() {
///             MY_TRANSACTION_ID => Box::new(MyTransaction::from_raw(raw)?),
///             _ => Err(EncError::IncorrectMessageType {
///                 message_type: raw.message_type(),
///             })?,
///         };
///         Ok(tx)
///     }
/// }
/// # fn main() { }
/// ```
///
/// [doc:services]: https://exonum.com/doc/architecture/services/
#[allow(unused_variables, unused_mut)]
pub trait Service: Send + Sync + 'static {
    /// Service identifier for database schema and service messages.
    /// Must be unique within the blockchain.
    fn service_id(&self) -> u16;

    /// Human-readable service name. Must be unique within the blockchain.
    fn service_name(&self) -> &'static str;

    /// Returns a list of root hashes of tables that determine the current state
    /// of the service database. These hashes are collected from all services in a common
    /// `ProofMapIndex` accessible in the core schema as [`state_hash_aggregator`][1].
    ///
    /// An empty vector can be returned if the service does not influence the blockchain state.
    ///
    /// See also [`service_table_unique_key`][2].
    ///
    /// [1]: struct.Schema.html#method.state_hash_aggregator
    /// [2]: struct.Blockchain.html#method.service_table_unique_key
    fn state_hash(&self, snapshot: &Snapshot) -> Vec<Hash>;

    /// Tries to create a `Transaction` from the given raw message.
    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, MessageError>;

    /// Initializes the information schema of service
    /// and generates an initial service configuration.
    /// Called on genesis block creation.
    fn initialize(&self, fork: &mut Fork) -> Value {
        Value::Null
    }

    /// Handles block commit. This handler is invoked for each service after commit of the block.
    /// For example, a service can create one or more transactions if a specific condition
    /// has occurred.
    ///
    /// *Try not to perform long operations in this handler*.
    fn handle_commit(&self, context: &ServiceContext) {}

    /// Returns an API handler for public requests. The handler is mounted on
    /// the `/api/services/{service_name}` path on [the public listen address][pub-addr]
    /// of all full nodes in the blockchain network.
    ///
    /// [pub-addr]: ../node/struct.NodeApiConfig.html#structfield.public_api_address
    fn public_api_handler(&self, context: &ApiContext) -> Option<Box<Handler>> {
        None
    }

    /// Returns an API handler for private requests. The handler is mounted on
    /// the `/api/services/{service_name}` path on [the private listen address][priv-addr]
    /// of all full nodes in the blockchain network.
    ///
    /// [priv-addr]: ../node/struct.NodeApiConfig.html#structfield.private_api_address
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

#[derive(Debug, Default)]
pub struct ApiNodeState {
    incoming_connections: HashSet<SocketAddr>,
    outgoing_connections: HashSet<SocketAddr>,
    reconnects_timeout: HashMap<SocketAddr, Milliseconds>,
    //TODO: update on event?
    peers_info: HashMap<SocketAddr, PublicKey>,
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
    pub fn peers_info(&self) -> Vec<(SocketAddr, PublicKey)> {
        self.state
            .read()
            .expect("Expected read lock.")
            .peers_info
            .iter()
            .map(|(c, e)| (*c, *e))
            .collect()
    }
    /// Update internal state, from `Node` State`
    pub fn update_node_state(&self, state: &State) {
        for (p, c) in state.peers().iter() {
            self.state
                .write()
                .expect("Expected write lock.")
                .peers_info
                .insert(c.addr(), *p);
        }
    }

    /// Returns value of the `state_update_timeout`.
    pub fn state_update_timeout(&self) -> Milliseconds {
        self.state_update_timeout
    }

    /// add incoming connection into state
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

    /// remove incoming connection from state
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

    /// Add reconnect timeout
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

    /// Removes reconnect timeout and returns the previous value.
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
