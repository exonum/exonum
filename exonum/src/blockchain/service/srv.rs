use serde_json::Value;
use serde::de::{Deserialize, DeserializeOwned, Deserializer};

use iron::Handler;

use storage::{Fork, Snapshot};
use messages::{Message, RawTransaction};
use encoding::Error as MessageError;
use crypto::Hash;

use super::{Transaction, ServiceContext, ApiContext, Service};


trait TransactionSet: DeserializeOwned {
    fn tx_from_raw(raw: RawTransaction) -> Result<Box<Transaction>, MessageError>;
}

enum NoTransactions {
}

impl TransactionSet for NoTransactions {
    fn tx_from_raw(raw: RawTransaction) -> Result<Box<Transaction>, MessageError> {
        unimplemented!()
    }
}

impl<'de> Deserialize<'de> for NoTransactions {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where
        D: Deserializer<'de> {
        unimplemented!()
    }
}

trait Srv: Send + Sync + 'static {
    type Transactions: TransactionSet;
    const ID: u16;
    const NAME: &'static str;

    fn state_hash(&self, snapshot: &Snapshot) -> Vec<Hash>;

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, MessageError>;

    fn initialize(&self, fork: &mut Fork) -> Value {
        Value::Null
    }

    fn handle_commit(&self, context: &ServiceContext) {}

    fn public_api_handler(&self, context: &ApiContext) -> Option<Box<Handler>> {
        None
    }

    fn private_api_handler(&self, context: &ApiContext) -> Option<Box<Handler>> {
        None
    }

    fn into_service(self) -> Box<Service> where Self: Sized {
        Box::new(SrvService(self))
    }
}

struct SrvService<S>(S);

impl<S: Srv> Service for SrvService<S> {
    fn service_id(&self) -> u16 {
        S::ID
    }

    fn service_name(&self) -> &'static str {
        S::NAME
    }

    fn state_hash(&self, snapshot: &Snapshot) -> Vec<Hash> {
        self.0.state_hash(snapshot)
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, MessageError> {
        S::Transactions::tx_from_raw(raw)
    }

    fn initialize(&self, fork: &mut Fork) -> Value {
        self.0.initialize(fork)
    }

    fn handle_commit(&self, context: &ServiceContext) {
        self.0.handle_commit(context)
    }

    fn public_api_handler(&self, context: &ApiContext) -> Option<Box<Handler>> {
        self.0.public_api_handler(context)
    }

    fn private_api_handler(&self, context: &ApiContext) -> Option<Box<Handler>> {
        self.0.private_api_handler(context)
    }
}
