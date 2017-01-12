use std::fmt::Debug;

use serde_json::Value;

use ::crypto::Hash;
use ::storage::View;
use ::messages::RawTransaction;

pub type Error = ::storage::Error;
pub type Result<T> = ::std::result::Result<T, Error>;

pub trait Transaction: Send + 'static + Debug {
    fn hash(&self) -> Hash;
    fn verify(&self) -> bool;
    fn execute(&self, view: &View) -> Result<()>;

    fn raw(&self) -> RawTransaction;
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

    fn state_hash(&self, view: &View) -> Result<Hash>;
    fn tx_from_raw(&self, raw: RawTransaction) -> Box<Transaction>;

    fn handle_genesis_block(&self, view: &View) -> Result<()>;
    fn handle_commit(&self, view: &View) -> Result<Vec<Box<Transaction>>>;
}