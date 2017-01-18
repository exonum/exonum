use std::fmt::Debug;

use serde_json::Value;

use ::crypto::Hash;
use ::storage::{View, Error as StorageError};
use ::messages::{RawTransaction, Error as MessageError};
use ::node::State;

pub trait Transaction: Send + 'static + Debug {
    fn hash(&self) -> Hash;
    fn verify(&self) -> bool;
    fn execute(&self, view: &View) -> Result<(), StorageError>;

    fn raw(&self) -> &RawTransaction;
    fn clone_box(&self) -> Box<Transaction>;

    fn info(&self) -> Value {
        Value::Null
    }
}

impl Clone for Box<Transaction> {
    fn clone(&self) -> Box<Transaction> {
        self.clone_box()
    }
}

pub trait Service: Send + Sync + 'static {
    fn service_id(&self) -> u16;

    fn state_hash(&self, view: &View) -> Result<Hash, StorageError>;
    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, MessageError>;

    fn handle_genesis_block(&self, view: &View) -> Result<(), StorageError>;
    fn handle_commit(&self,
                     view: &View,
                     state: &mut State)
                     -> Result<Vec<Box<Transaction>>, StorageError>;
}