use exonum::crypto::Hash;
use exonum::storage::View;
use exonum::blockchain::{Transaction, Service, NodeState};
use exonum::storage::Error as StorageError;
use exonum::messages::{FromRaw, RawTransaction, Error as MessageError};

use {TimestampingSchema, TimestampTx};

pub const TIMESTAMPING_SERVICE_ID: u16 = 128;

pub struct TimestampingService {}

impl TimestampingService {
    pub fn new() -> TimestampingService {
        TimestampingService {}
    }
}

impl Service for TimestampingService {
    fn service_id(&self) -> u16 {
        TIMESTAMPING_SERVICE_ID
    }

    fn state_hash(&self, view: &View) -> Result<Vec<Hash>, StorageError> {
        let schema = TimestampingSchema::new(view);
        schema.state_hash()
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, MessageError> {
        TimestampTx::from_raw(raw).map(|tx| Box::new(tx) as Box<Transaction>)
    }

    fn handle_commit(&self, _: &mut NodeState) -> Result<(), StorageError> {
        Ok(())
    }
}