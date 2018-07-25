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

use actix::{Addr, Syn};
use serde_json::Value;

use std::{
    collections::{HashMap, HashSet}, fmt, net::SocketAddr, sync::{Arc, RwLock},
};

use super::transaction::Transaction;
use api::{websocket, ServiceApiBuilder};
use blockchain::{ConsensusConfig, Schema, StoredConfiguration, ValidatorKeys};
use crypto::{Hash, PublicKey, SecretKey};
use encoding::Error as MessageError;
use helpers::{Height, Milliseconds, ValidatorId};
use messages::RawTransaction;
use node::{ApiSender, NodeRole, State, TransactionSend};
use storage::{Fork, Snapshot};

/// A trait that describes the business logic of a certain service.
///
/// Services are the main extension point for the Exonum framework. Initially,
/// Exonum does not provide specific transaction processing rules or business
/// logic, they are implemented with the help of services.
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
    fn state_hash(&self, snapshot: &dyn Snapshot) -> Vec<Hash>;

    /// Tries to create a `Transaction` from the given raw message.
    ///
    /// Exonum framework only guarantees that `SERVICE_ID` of the message is equal to the
    /// identifier of this service, therefore, the implementation should be ready to handle
    /// invalid transactions that may come from byzantine nodes.
    ///
    /// Service should return an error in the following cases (see `MessageError` for more
    /// details):
    ///
    /// - Incorrect transaction identifier.
    /// - Incorrect data layout.
    ///
    /// Service _shouldn't_ perform a signature check or logical validation of the transaction;
    /// these operations should be performed in the `Transaction::verify` and
    /// `Transaction::execute` methods.
    ///
    /// `transactions!` macro generates code that allows simple implementation, see
    /// [the `Service` example above](#examples).
    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<dyn Transaction>, MessageError>;

    /// Invoked for all deployed services during the blockchain initialization
    /// on genesis block creation each time a node is started.
    /// During the handling of the method the service is able to perform the following activities:
    /// - store its own initial state to the storage [`&mut Fork`]
    /// - return an initial [global configuration][doc:global_cfg] of the service in the JSON
    /// format, if service has global configuration parameters. This configuration is used
    /// to create a genesis block.
    ///
    /// [doc:global_cfg]: https://exonum.com/doc/architecture/services/#global-configuration.
    /// [`&mut Fork`]: https://exonum.com/doc/architecture/storage/#forks
    fn initialize(&self, fork: &mut Fork) -> Value {
        Value::Null
    }

    /// A service execution. This method is invoked for each service after execution
    /// of all transactions in the block but before `after_commit` handler.
    ///
    /// The order of invoking `before_commit` method for every service depends on the
    /// service ID. `before_commit` for the service with the smallest ID is invoked
    /// first up to the largest one.
    /// Effectively, this means that services should not rely on a particular ordering of
    /// Service::execute invocations.
    fn before_commit(&self, fork: &mut Fork) {}

    /// Handles block commit. This handler is invoked for each service after commit of the block.
    /// For example, a service can create one or more transactions if a specific condition
    /// has occurred.
    ///
    /// *Try not to perform long operations in this handler*.
    fn after_commit(&self, context: &ServiceContext) {}

    /// Extends API by handlers of this service. The request handlers are mounted on
    /// the `/api/services/{service_name}` path at the listen address of every
    /// full node in the blockchain network.
    ///
    /// *Default implementation does nothing*
    fn wire_api(&self, _builder: &mut ServiceApiBuilder) {}
}

/// The current node state on which the blockchain is running, or in other words
/// execution context. This structure is passed to the `after_commit` method
/// of the `Service` trait and is used for the interaction between service
/// business logic and the current node state.
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
    ) -> Self {
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

        Self {
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
    /// retrieve schema information from the database.
    pub fn snapshot(&self) -> &dyn Snapshot {
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

    /// Returns service specific global variables as a JSON value.
    pub fn actual_service_config(&self, service: &dyn Service) -> &Value {
        &self.stored_configuration.services[service.service_name()]
    }

    /// Returns a reference to the transaction sender, which can then be used
    /// to broadcast a transaction to other nodes in the network.
    pub fn transaction_sender(&self) -> &dyn TransactionSend {
        &self.api_sender
    }

    /// Returns the actual blockchain global configuration.
    pub fn stored_configuration(&self) -> &StoredConfiguration {
        &self.stored_configuration
    }
}

#[derive(Default)]
pub struct ApiNodeState {
    incoming_connections: HashSet<SocketAddr>,
    outgoing_connections: HashSet<SocketAddr>,
    reconnects_timeout: HashMap<SocketAddr, Milliseconds>,
    // TODO: Update on event? (ECR-1632)
    peers_info: HashMap<SocketAddr, PublicKey>,
    is_enabled: bool,
    node_role: NodeRole,
    majority_count: usize,
    validators: Vec<ValidatorKeys>,
    broadcast_server_address: Option<Addr<Syn, websocket::Server>>,
}

impl fmt::Debug for ApiNodeState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ApiNodeState")
            .field("incoming_connections", &self.incoming_connections)
            .field("outgoing_connections", &self.outgoing_connections)
            .field("reconnects_timeout", &self.reconnects_timeout)
            .field("peers_info", &self.peers_info)
            .field("is_enabled", &self.is_enabled)
            .field("node_role", &self.node_role)
            .field("majority_count", &self.majority_count)
            .field("validators", &self.validators)
            .finish()
    }
}

impl ApiNodeState {
    fn new() -> Self {
        Self {
            is_enabled: true,
            ..Default::default()
        }
    }
}

/// Shared part of the context, used to take some values from the `Node`
/// `State`. As there is no way to directly access
/// the node state, this entity is regularly updated with information about the
/// node and transfers this information to API.
#[derive(Clone, Debug)]
pub struct SharedNodeState {
    state: Arc<RwLock<ApiNodeState>>,
    /// Timeout to update API state.
    pub state_update_timeout: Milliseconds,
}

impl SharedNodeState {
    /// Creates a new `SharedNodeState` instance.
    pub fn new(state_update_timeout: Milliseconds) -> Self {
        Self {
            state: Arc::new(RwLock::new(ApiNodeState::new())),
            state_update_timeout,
        }
    }
    /// Returns a list of connected addresses of other nodes.
    pub fn incoming_connections(&self) -> Vec<SocketAddr> {
        self.state
            .read()
            .expect("Expected read lock.")
            .incoming_connections
            .iter()
            .cloned()
            .collect()
    }
    /// Returns a list of our connection sockets.
    pub fn outgoing_connections(&self) -> Vec<SocketAddr> {
        self.state
            .read()
            .expect("Expected read lock.")
            .outgoing_connections
            .iter()
            .cloned()
            .collect()
    }
    /// Returns a list of other nodes to which the connection has failed
    /// and a reconnect attempt is required. The method also indicates the time
    /// after which a new connection attempt is performed.
    pub fn reconnects_timeout(&self) -> Vec<(SocketAddr, Milliseconds)> {
        self.state
            .read()
            .expect("Expected read lock.")
            .reconnects_timeout
            .iter()
            .map(|(c, e)| (*c, *e))
            .collect()
    }
    /// Returns a list of addresses and public keys of peers from which the
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
    /// Updates internal state, from `State` of a blockchain node.
    pub fn update_node_state(&self, state: &State) {
        let mut lock = self.state.write().expect("Expected write lock.");

        lock.peers_info.clear();
        lock.incoming_connections.clear();
        lock.outgoing_connections.clear();
        lock.majority_count = state.majority_count();
        lock.node_role = NodeRole::new(state.validator_id());
        lock.validators = state.validators().to_vec();

        for (p, c) in state.peers() {
            lock.peers_info.insert(c.addr(), *p);
            lock.outgoing_connections.insert(c.addr());
        }

        for addr in state.connections().keys() {
            lock.incoming_connections.insert(*addr);
        }
    }

    /// Returns a boolean value which indicates whether the consensus is achieved.
    pub fn consensus_status(&self) -> bool {
        let lock = self.state.read().expect("Expected read lock.");
        let mut active_validators = lock.peers_info
            .values()
            .filter(|peer_key| {
                lock.validators
                    .iter()
                    .any(|validator| validator.consensus_key == **peer_key)
            })
            .count();

        if lock.node_role.is_validator() {
            // Peers list doesn't include current node address, so we have to increment its length.
            // E.g. if we have 3 items in peers list, it means that we have 4 nodes overall.
            active_validators += 1;
        }

        // Just after Node is started (node status isn't updated) majority_count = 0,
        // so we have to check that majority count is greater than 0.
        active_validators >= lock.majority_count && lock.majority_count > 0
    }

    /// Returns a boolean value which indicates whether the node is enabled
    /// or not.
    pub fn is_enabled(&self) -> bool {
        let state = self.state.read().expect("Expected read lock.");
        state.is_enabled
    }

    /// Transfers information to the node that the consensus process on the node
    /// should halt.
    pub fn set_enabled(&self, is_enabled: bool) {
        let mut state = self.state.write().expect("Expected write lock.");
        state.is_enabled = is_enabled;
    }

    pub(crate) fn set_node_role(&self, role: NodeRole) {
        let mut state = self.state.write().expect("Expected write lock.");
        state.node_role = role;
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

    pub(crate) fn set_broadcast_server_address(&self, address: Addr<Syn, websocket::Server>) {
        let mut state = self.state.write().expect("Expected write lock");
        state.broadcast_server_address = Some(address);
    }

    /// Broadcast message to all subscribers.
    pub(crate) fn broadcast(&self, block_hash: &Hash) {
        if let Some(ref address) = self.state
            .read()
            .expect("Expected read lock")
            .broadcast_server_address
        {
            address.do_send(websocket::Broadcast {
                block_hash: *block_hash,
            })
        }
    }
}

impl<'a, S: Service> From<S> for Box<dyn Service + 'a> {
    fn from(s: S) -> Self {
        Box::new(s) as Self
    }
}
