use serde_json::Value;
use router::Router;

use crypto::{Hash, PublicKey, SecretKey};
use storage::{View, Error as StorageError};
use messages::{Message, RawTransaction, Error as MessageError};
use node::{Node, State, NodeChannel, TxSender};
use node::state::ValidatorState;
use events::Milliseconds;
use blockchain::{StoredConfiguration, ConsensusConfig, Blockchain};

pub trait Transaction: Message + 'static {
    fn verify(&self) -> bool;
    fn execute(&self, view: &View) -> Result<(), StorageError>;
    fn info(&self) -> Value {
        Value::Null
    }
}

pub trait Service: Send + Sync + 'static {
    fn service_id(&self) -> u16;

    fn state_hash(&self, _: &View) -> Result<Vec<Hash>, StorageError> {
        Ok(Vec::new())
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, MessageError>;

    fn handle_genesis_block(&self, _: &View) -> Result<Value, StorageError> {
        Ok(Value::Null)
    }

    fn handle_commit(&self, _: &mut NodeState) -> Result<(), StorageError> {
        Ok(())
    }

    fn wire_public_api(&self, _: &ApiContext, _: &mut Router) {}

    fn wire_private_api(&self, _: &ApiContext, _: &mut Router) {}
}

pub struct NodeState<'a, 'b> {
    state: &'a mut State,
    view: &'b View,
    txs: Vec<Box<Transaction>>,
}

impl<'a, 'b> NodeState<'a, 'b> {
    pub fn new(state: &'a mut State, view: &'b View) -> NodeState<'a, 'b> {
        NodeState {
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

    pub fn public_key(&self) -> &PublicKey {
        self.state.public_key()
    }

    pub fn secret_key(&self) -> &SecretKey {
        self.state.secret_key()
    }

    pub fn actual_config(&self) -> &StoredConfiguration {
        self.state.config()
    }

    pub fn consensus_config(&self) -> &ConsensusConfig {
        self.state.consensus_config()
    }

    pub fn service_config(&self, service: &Service) -> &Value {
        let id = service.service_id();
        self.state
            .services_config()
            .get(&format!("{}", id))
            .unwrap()
    }

    pub fn update_config(&mut self, new_config: StoredConfiguration) {
        self.state.update_config(new_config)
    }

    pub fn propose_timeout(&self) -> Milliseconds {
        self.state.propose_timeout()
    }

    pub fn set_propose_timeout(&mut self, timeout: Milliseconds) {
        self.state.set_propose_timeout(timeout)
    }

    pub fn add_transaction<T: Transaction>(&mut self, tx: T) {
        assert!(tx.verify());
        self.txs.push(Box::new(tx));
    }

    pub fn transactions(self) -> Vec<Box<Transaction>> {
        self.txs
    }
}

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

    pub fn wire_public_api(&self, router: &mut Router) {
        self.blockchain.wire_public_api(self, router)
    }

    pub fn wire_private_api(&self, router: &mut Router) {
        self.blockchain.wire_public_api(self, router)
    }
}