use serde_json::Value;
use iron::Handler;
use mount::Mount;

use crypto::{Hash, PublicKey, SecretKey};
use storage::{View, Error as StorageError};
use messages::{Message, RawTransaction};
use encoding::Error as MessageError;
use node::{Node, State, NodeChannel, TxSender};
use node::state::ValidatorState;
use blockchain::{ConsensusConfig, Blockchain};

pub trait Transaction: Message + 'static {
    fn verify(&self) -> bool;
    fn execute(&self, view: &View) -> Result<(), StorageError>;
    fn info(&self) -> Value {
        Value::Null
    }
}

#[allow(unused_variables, unused_mut)]
pub trait Service: Send + Sync + 'static {
    /// Unique service identification for database schema and service messages.
    fn service_id(&self) -> u16;
    /// Unique human readable service name.
    fn service_name(&self) -> &'static str;

    fn state_hash(&self, _: &View) -> Result<Vec<Hash>, StorageError> {
        Ok(Vec::new())
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, MessageError>;

    fn handle_genesis_block(&self, view: &View) -> Result<Value, StorageError> {
        Ok(Value::Null)
    }

    fn handle_commit(&self, context: &mut ServiceContext) -> Result<(), StorageError> {
        Ok(())
    }
    /// Returns api handler for public users.
    fn public_api_handler(&self, context: &ApiContext) -> Option<Box<Handler>> {
        None
    }
    /// Returns api handler for maintainers.
    fn private_api_handler(&self, context: &ApiContext) -> Option<Box<Handler>> {
        None
    }
}

#[derive(Debug)]
pub struct ServiceContext<'a, 'b> {
    state: &'a mut State,
    view: &'b View,
    txs: Vec<Box<Transaction>>,
}

impl<'a, 'b> ServiceContext<'a, 'b> {
    pub fn new(state: &'a mut State, view: &'b View) -> ServiceContext<'a, 'b> {
        ServiceContext {
            state: state,
            view: view,
            txs: Vec::new(),
        }
    }

    pub fn validator_state(&self) -> &Option<ValidatorState> {
        self.state.validator_state()
    }

    pub fn view(&self) -> &View {
        self.view
    }

    pub fn height(&self) -> u64 {
        self.state.height()
    }

    pub fn round(&self) -> u32 {
        self.state.round()
    }

    pub fn validators(&self) -> &[PublicKey] {
        self.state.validators()
    }

    pub fn consensus_public_key(&self) -> &PublicKey {
        self.state.consensus_public_key()
    }

    pub fn consensus_secret_key(&self) -> &SecretKey {
        self.state.consensus_secret_key()
    }

    pub fn service_public_key(&self) -> &PublicKey {
        self.state.service_public_key()
    }

    pub fn service_secret_key(&self) -> &SecretKey {
        self.state.service_secret_key()
    }

    pub fn actual_consensus_config(&self) -> &ConsensusConfig {
        self.state.consensus_config()
    }

    pub fn actual_service_config(&self, service: &Service) -> &Value {
        let name = service.service_name();
        self.state.services_config().get(name).unwrap()
    }

    pub fn add_transaction<T: Transaction>(&mut self, tx: T) {
        assert!(tx.verify());
        self.txs.push(Box::new(tx));
    }

    pub fn transactions(self) -> Vec<Box<Transaction>> {
        self.txs
    }
}

#[derive(Debug)]
pub struct ApiContext {
    blockchain: Blockchain,
    node_channel: TxSender<NodeChannel>,
    public_key: PublicKey,
    secret_key: SecretKey,
}

impl ApiContext {
    pub fn new(node: &Node) -> ApiContext {
        let handler = node.handler();
        ApiContext {
            blockchain: handler.blockchain.clone(),
            node_channel: node.channel(),
            public_key: *node.state().public_key(),
            secret_key: node.state().secret_key().clone(),
        }
    }

    pub fn blockchain(&self) -> &Blockchain {
        &self.blockchain
    }

    pub fn node_channel(&self) -> &TxSender<NodeChannel> {
        &self.node_channel
    }

    pub fn public_key(&self) -> &PublicKey {
        &self.public_key
    }

    pub fn secret_key(&self) -> &SecretKey {
        &self.secret_key
    }

    pub fn mount_public_api(&self) -> Mount {
        self.blockchain.mount_public_api(self)
    }

    pub fn mount_private_api(&self) -> Mount {
        self.blockchain.mount_private_api(self)
    }
}