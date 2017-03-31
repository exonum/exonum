use serde_json::Value;

use ::crypto::{Hash, PublicKey, SecretKey};
use ::storage::{View, Error as StorageError};
use ::messages::{Message, RawTransaction, Error as MessageError};
use ::node::State;
use ::blockchain::{StoredConfiguration, ConsensusConfig};

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

    pub fn view(&self) -> &View {
        self.view
    }

    pub fn id(&self) -> u32 {
        self.state.id()
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

    pub fn propose_timeout(&self) -> i64 {
        self.state.propose_timeout()
    }

    pub fn set_propose_timeout(&mut self, timeout: i64) {
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
