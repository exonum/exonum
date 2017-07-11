//! This module defines the Exonum services interfaces. Like smart contracts in some other
//! blockchain platforms, Exonum services encapsulate business logic of the blockchain application.

use serde_json::Value;
use iron::Handler;
use mount::Mount;

use std::fmt;

use crypto::{Hash, PublicKey, SecretKey};
use storage::{Snapshot, Fork};
use messages::{Message, RawTransaction};
use encoding::Error as MessageError;
use node::{Node, State, NodeChannel, TxSender};
use node::state::ValidatorState;
use blockchain::{ConsensusConfig, Blockchain, ValidatorKeys};

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
    /// Returns the useful information about the transaction in the JSON format.
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

    /// Handles genesis block creation event.
    /// By this method you can initialize information schema of service
    /// and generates initial service configuration.
    fn handle_genesis_block(&self, fork: &mut Fork) -> Value {
        Value::Null
    }

    /// Handles commit event. This handler is invoked for each service after commit of the block.
    /// For example service can create some transaction if the specific condition occurred.
    ///
    /// *Try not to perform long operations here*.
    fn handle_commit(&self, context: &mut ServiceContext) { }

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
pub struct ServiceContext<'a, 'b> {
    state: &'a mut State,
    snapshot: &'b Snapshot,
    txs: Vec<Box<Transaction>>,
}


impl<'a, 'b> ServiceContext<'a, 'b> {
    #[doc(hidden)]
    pub fn new(state: &'a mut State, snapshot: &'b Snapshot) -> ServiceContext<'a, 'b> {
        ServiceContext {
            state: state,
            snapshot: snapshot,
            txs: Vec::new(),
        }
    }

    /// If the current node is validator returns its state.
    /// For other nodes return `None`.
    pub fn validator_state(&self) -> &Option<ValidatorState> {
        self.state.validator_state()
    }

    /// Returns the current database snapshot.
    pub fn snapshot(&self) -> &'b Snapshot {
        self.snapshot
    }

    /// Returns the current blockchain height. This height is 'height of last committed block` + 1.
    pub fn height(&self) -> u64 {
        self.state.height()
    }

    /// Returns the current node round.
    pub fn round(&self) -> u32 {
        self.state.round()
    }

    /// Returns the current list of validators.
    pub fn validators(&self) -> &[ValidatorKeys] {
        self.state.validators()
    }

    /// Returns current node's public key.
    pub fn public_key(&self) -> &PublicKey {
        self.state.service_public_key()
    }

    /// Returns current node's secret key.
    pub fn secret_key(&self) -> &SecretKey {
        self.state.service_secret_key()
    }

    /// Returns the actual blockchain global configuration.
    pub fn actual_consensus_config(&self) -> &ConsensusConfig {
        self.state.consensus_config()
    }

    /// Returns service specific global variables as json value.
    pub fn actual_service_config(&self, service: &Service) -> &Value {
        let name = service.service_name();
        self.state.services_config().get(name).unwrap()
    }

    /// Adds transaction to the queue.
    /// After the services handle commit event these transactions will be broadcast by node.
    pub fn add_transaction(&mut self, tx: Box<Transaction>) {
        assert!(tx.verify());
        self.txs.push(tx);
    }

    #[doc(hidden)]
    pub fn transactions(self) -> Vec<Box<Transaction>> {
        self.txs
    }
}

impl<'a, 'b> fmt::Debug for ServiceContext<'a, 'b> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ServiceContext(state: {:?}, txs: {:?})", self.state, self.txs)
    }
}

/// Provides the current node state to api handlers.
pub struct ApiContext {
    blockchain: Blockchain,
    node_channel: TxSender<NodeChannel>,
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

    /// Returns reference to the node's blockchain.
    pub fn blockchain(&self) -> &Blockchain {
        &self.blockchain
    }

    /// Returns reference to the transaction sender.
    pub fn node_channel(&self) -> &TxSender<NodeChannel> {
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

    /// Returns `Mount` object that aggregates public api handlers.
    pub fn mount_public_api(&self) -> Mount {
        self.blockchain.mount_public_api(self)
    }

    /// Returns `Mount` object that aggregates private api handlers.
    pub fn mount_private_api(&self) -> Mount {
        self.blockchain.mount_private_api(self)
    }
}

impl ::std::fmt::Debug for ApiContext {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ApiContext(blockchain: {:?}, public_key: {:?})", self.blockchain, self.public_key)
    }
}
