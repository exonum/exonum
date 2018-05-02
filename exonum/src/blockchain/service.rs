// Copyright 2018 The Exonum Team
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

use serde_json::Value;
use iron::Handler;

use std::fmt;
use std::sync::{Arc, RwLock};
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;

use crypto::{Hash, PublicKey, SecretKey};
use storage::{Fork, Snapshot};
use messages::RawTransaction;
use encoding::Error as MessageError;
use node::{ApiSender, Node, State, TransactionSend};
use blockchain::{Blockchain, ConsensusConfig, Schema, StoredConfiguration, ValidatorKeys};
use helpers::{Height, Milliseconds, ValidatorId};
use super::transaction::Transaction;

/// A trait that describes the business logic of a certain service.
///
/// Services are the main extension point for the Exonum framework. Initially,
/// Exonum does not provide specific transaction processing rules or business
/// logic, they are implemented with the help of services.
///
/// The code above indicates the other traits on which the `Service` trait is
/// dependent and the functions required for this trait. The first four functions
/// are mandatory for implementation of the trait, while the last four functions
/// are optional as they have some default values.
///
/// See also [the documentation page on services][doc:services].
///
/// # Examples
///
/// The example below provides a bare-bones foundation for implementing a service.
///
/// ```
/// #[macro_use] extern crate exonum;
/// // Exports from `exonum` crate skipped
/// # use exonum::blockchain::{Service, Transaction, TransactionSet, ExecutionResult};
/// # use exonum::crypto::Hash;
/// # use exonum::messages::{ServiceMessage, Message, RawTransaction};
/// # use exonum::storage::{Fork, Snapshot};
/// use exonum::encoding::Error as EncError;
///
/// // Reused constants
/// const SERVICE_ID: u16 = 8000;
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
///         // Calculates the state hash of the service
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
/// transactions! {
///     MyTransactions {
///         const SERVICE_ID = SERVICE_ID;
///
///         struct TxA {
///             // Transaction fields
///         }
///
///         struct TxB {
///             // ...
///         }
///     }
/// }
///
/// impl Transaction for TxA {
///     // Business logic implementation
/// #   fn verify(&self) -> bool { true }
/// #   fn execute(&self, fork: &mut Fork) -> ExecutionResult { Ok(()) }
/// }
///
/// impl Transaction for TxB {
/// #   fn verify(&self) -> bool { true }
/// #   fn execute(&self, fork: &mut Fork) -> ExecutionResult { Ok(()) }
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
///     fn service_name(&self) -> &str {
///         "my_special_unique_service"
///     }
///
///     fn state_hash(&self, snapshot: &Snapshot) -> Vec<Hash> {
///         MyServiceSchema::new(snapshot).state_hash()
///     }
///
///     fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, EncError> {
///         let tx = MyTransactions::tx_from_raw(raw)?;
///         Ok(tx.into())
///     }
/// }
/// # fn main() { }
/// ```
///
/// [doc:services]: https://exonum.com/doc/architecture/services/
#[allow(unused_variables, unused_mut)]
pub trait Service: Send + Sync + 'static {
    /// Service identifier for database schema and service messages.
    /// This ID must be unique within the blockchain.
    fn service_id(&self) -> u16;

    /// A comprehensive string service name. This name must be unique within the
    /// blockchain.
    fn service_name(&self) -> &str;

    /// Returns a list of root hashes of tables that determine the current state
    /// of the service database. These hashes are collected from all the services in a common
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
    ///
    /// Exonum framework only guarantees that `SERVICE_ID` of the message is equal to the
    /// identifier of this service, therefore, the implementation should be ready to handle invalid
    /// transactions that may come from byzantine nodes.
    ///
    /// Service should return an error in the following cases (see `MessageError` for more details):
    ///
    /// - Incorrect transaction identifier.
    /// - Incorrect data layout.
    ///
    /// Service _shouldn't_ perform a signature check or logical validation of the transaction; these
    /// operations should be performed in the `Transaction::verify` and `Transaction::execute`
    /// methods.
    ///
    /// `transactions!` macro generates code that allows simple implementation, see
    /// [the `Service` example above](#examples).
    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, MessageError>;

    /// Initializes the information schema of the service
    /// and generates an initial service configuration.
    /// This method is called on genesis block creation.
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
    /// the `/api/services/{service_name}` path at [the public listen address][pub-addr]
    /// of all full nodes in the blockchain network.
    ///
    /// [pub-addr]: ../node/struct.NodeApiConfig.html#structfield.public_api_address
    fn public_api_handler(&self, context: &ApiContext) -> Option<Box<Handler>> {
        None
    }

    /// Returns an API handler for private requests. The handler is mounted on
    /// the `/api/services/{service_name}` path at [the private listen address][private-addr]
    /// of all full nodes in the blockchain network.
    ///
    /// [private-addr]: ../node/struct.NodeApiConfig.html#structfield.private_api_address
    fn private_api_handler(&self, context: &ApiContext) -> Option<Box<Handler>> {
        None
    }
}

/// The current node state on which the blockchain is running, or in other words
/// execution context. This structure is passed to the `handle_commit` method
/// of the `Service` trait and is used for the interaction between service
/// business logic and the current node state. `ServiceContext` connects the
/// node entity and the user code of services. When writing a services, developers can
/// apply the 'ServiceContext' to retrieve information about the current node.
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
    /// Creates service context for the given node.
    ///
    /// This method is necessary if you want to implement an alternative exonum node.
    /// For example, you can implement a special node without consensus for regression
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

    /// If the current node is a validator, returns its identifier.
    /// For other nodes return `None`.
    pub fn validator_id(&self) -> Option<ValidatorId> {
        self.validator_id
    }

    /// Returns the current database snapshot. This snapshot is used to
    /// retrieve schema information from the data base.
    pub fn snapshot(&self) -> &Snapshot {
        self.fork.as_ref()
    }

    /// Returns the current blockchain height. This height is "height of the last committed block".
    pub fn height(&self) -> Height {
        self.height
    }

    /// Returns the current list of validator public keys.
    pub fn validators(&self) -> &[ValidatorKeys] {
        self.stored_configuration.validator_keys.as_slice()
    }

    /// Returns the public key of the current node.
    pub fn public_key(&self) -> &PublicKey {
        &self.service_keypair.0
    }

    /// Returns the secret key of the current node.
    pub fn secret_key(&self) -> &SecretKey {
        &self.service_keypair.1
    }

    /// Returns the actual consensus configuration.
    pub fn actual_consensus_config(&self) -> &ConsensusConfig {
        &self.stored_configuration.consensus
    }

    /// Returns service specific global variables as a json value.
    pub fn actual_service_config(&self, service: &Service) -> &Value {
        &self.stored_configuration.services[service.service_name()]
    }

    /// Returns a reference to the transaction sender, which can then be used
    /// to broadcast a transaction to other nodes in the network.
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
    is_enabled: bool,
}

impl ApiNodeState {
    fn new() -> ApiNodeState {
        Self::default()
    }
}

/// Shared part of the context, used to take some values from the `Node`
/// `State`. As there is no way to directly access
/// the node state, this entity is regularly updated with information about the
/// node and transfers this information to API.
#[derive(Clone, Debug)]
pub struct SharedNodeState {
    state: Arc<RwLock<ApiNodeState>>,
    /// Timeout to update api state.
    pub state_update_timeout: Milliseconds,
}

impl SharedNodeState {
    /// Creates a new `SharedNodeState` instance.
    pub fn new(state_update_timeout: Milliseconds) -> SharedNodeState {
        SharedNodeState {
            state: Arc::new(RwLock::new(ApiNodeState::new())),
            state_update_timeout,
        }
    }
    /// Returns the list of connected addresses of other nodes.
    pub fn incoming_connections(&self) -> Vec<SocketAddr> {
        self.state
            .read()
            .expect("Expected read lock.")
            .incoming_connections
            .iter()
            .cloned()
            .collect()
    }
    /// Returns the list of our connection sockets.
    pub fn outgoing_connections(&self) -> Vec<SocketAddr> {
        self.state
            .read()
            .expect("Expected read lock.")
            .outgoing_connections
            .iter()
            .cloned()
            .collect()
    }
    /// Returns the list of other nodes to which the connection has failed
    /// and a reconnection is required. The method also indicates the time
    /// after which a new attempt at connection is performed.
    pub fn reconnects_timeout(&self) -> Vec<(SocketAddr, Milliseconds)> {
        self.state
            .read()
            .expect("Expected read lock.")
            .reconnects_timeout
            .iter()
            .map(|(c, e)| (*c, *e))
            .collect()
    }
    /// Returns the list of addresses and public keys of peers from which the
    /// node has received `Connect` messages.
    pub fn peers_info(&self) -> Vec<(SocketAddr, PublicKey)> {
        self.state
            .read()
            .expect("Expected read lock.")
            .peers_info
            .iter()
            .map(|(c, e)| (*c, *e))
            .collect()
    }
    /// Updates internal state, from `Node State`.
    pub fn update_node_state(&self, state: &State) {
        for (p, c) in state.peers().iter() {
            self.state
                .write()
                .expect("Expected write lock.")
                .peers_info
                .insert(c.addr(), *p);
        }
    }

    /// Returns a boolean value which indicates whether the node is enabled
    /// or not.
    pub fn is_enabled(&self) -> bool {
        let state = self.state.read().expect("Expected read lock.");
        state.is_enabled
    }

    /// Informs the internal state about the halting of the node.
    pub fn set_enabled(&self, is_enabled: bool) {
        let mut state = self.state.write().expect("Expected read lock.");
        state.is_enabled = is_enabled;
    }

    /// Returns the value of the `state_update_timeout`.
    pub fn state_update_timeout(&self) -> Milliseconds {
        self.state_update_timeout
    }

    /// Adds an incoming connection into the state.
    pub fn add_incoming_connection(&self, addr: SocketAddr) {
        self.state
            .write()
            .expect("Expected write lock")
            .incoming_connections
            .insert(addr);
    }
    /// Adds an outgoing connection into the state.
    pub fn add_outgoing_connection(&self, addr: SocketAddr) {
        self.state
            .write()
            .expect("Expected write lock")
            .outgoing_connections
            .insert(addr);
    }

    /// Removes an incoming connection from the state.
    pub fn remove_incoming_connection(&self, addr: &SocketAddr) -> bool {
        self.state
            .write()
            .expect("Expected write lock")
            .incoming_connections
            .remove(addr)
    }

    /// Removes an outgoing connection from the state.
    pub fn remove_outgoing_connection(&self, addr: &SocketAddr) -> bool {
        self.state
            .write()
            .expect("Expected write lock")
            .outgoing_connections
            .remove(addr)
    }

    /// Adds a reconnect timeout.
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

    /// Removes the reconnect timeout and returns the previous value.
    pub fn remove_reconnect_timeout(&self, addr: &SocketAddr) -> Option<Milliseconds> {
        self.state
            .write()
            .expect("Expected write lock")
            .reconnects_timeout
            .remove(addr)
    }
}

/// Provides the current node state to API handlers.
pub struct ApiContext {
    blockchain: Blockchain,
    node_channel: ApiSender,
    public_key: PublicKey,
    secret_key: SecretKey,
}

/// Provides the current node state to API handlers.
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

    /// Returns a reference to the blockchain of this node.
    pub fn blockchain(&self) -> &Blockchain {
        &self.blockchain
    }

    /// Returns a reference to the transaction sender.
    pub fn node_channel(&self) -> &ApiSender {
        &self.node_channel
    }

    /// Returns the public key of the current node.
    pub fn public_key(&self) -> &PublicKey {
        &self.public_key
    }

    /// Returns the secret key of the current node.
    pub fn secret_key(&self) -> &SecretKey {
        &self.secret_key
    }
}

impl ::std::fmt::Debug for ApiContext {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "ApiContext(blockchain: {:?}, public_key: {:?})",
            self.blockchain, self.public_key
        )
    }
}

impl<'a, S: Service> From<S> for Box<Service + 'a> {
    fn from(s: S) -> Self {
        Box::new(s) as Box<Service>
    }
}
