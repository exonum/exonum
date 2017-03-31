use exonum::crypto::{PublicKey, Hash};
use exonum::blockchain::{Service, Transaction, Schema};
use exonum::messages::{RawTransaction, Message, FromRaw, Error as MessageError};
use exonum::storage::{View, Error as StorageError};
use exonum::blockchain::StoredConfiguration;

pub const CONFIG_SERVICE: u16 = 1;
pub const CONFIG_PROPOSE_MESSAGE_ID: u16 = 0;

message! {
    TxConfig {
        const TYPE = CONFIG_SERVICE;
        const ID = CONFIG_PROPOSE_MESSAGE_ID;
        const SIZE = 48;

        from:               &PublicKey  [00 => 32]
        config:             &[u8]       [32 => 40]
        actual_from_height: u64         [40 => 48]
    }
}


pub struct ConfigUpdateService {}

impl ConfigUpdateService {
    pub fn new() -> ConfigUpdateService {
        ConfigUpdateService {}
    }
}

impl Transaction for TxConfig {
    fn verify(&self) -> bool {
        self.verify_signature(self.from())
    }

    fn execute(&self, view: &View) -> Result<(), StorageError> {
        let schema = Schema::new(view);
        schema.commit_actual_configuration(StoredConfiguration::deserialize_err(self.config()).unwrap())
    }
}

impl Service for ConfigUpdateService {
    fn service_id(&self) -> u16 {
        CONFIG_SERVICE
    }

    fn state_hash(&self, _: &View) -> Result<Vec<Hash>, StorageError> {
        Ok(vec![])
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, MessageError> {
        if raw.message_type() != CONFIG_PROPOSE_MESSAGE_ID {
            return Err(MessageError::IncorrectMessageType { message_type: raw.message_type() });
        }
        TxConfig::from_raw(raw).map(|tx| Box::new(tx) as Box<Transaction>)
    }
}
