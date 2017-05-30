use serde_json;
use serde_json::Value;

use exonum::crypto::Hash;
use exonum::storage::View;
use exonum::blockchain::Transaction;
use exonum::storage::{MapTable, MerklePatriciaTable, Map, Error as StorageError};

use TIMESTAMPING_SERVICE_ID;

pub const TIMESTAMPING_TX_ID: u16 = 0;

message! {
    struct TimestampTx {
        const TYPE = TIMESTAMPING_SERVICE_ID;
        const ID = TIMESTAMPING_TX_ID;
        const SIZE = 48;

        field description:    &str        [00 => 08]
        field time:           i64         [08 => 16]
        field hash:           &Hash       [16 => 48]
    }
}

storage_value! {
    struct Content {
        const SIZE = 48;

        field description:        &str        [00 => 08]
        field time:               i64         [08 => 16]
        field data_hash:          &Hash       [16 => 48]
    }
}

pub struct TimestampingSchema<'a> {
    view: &'a View,
}

impl<'a> TimestampingSchema<'a> {
    pub fn new(view: &'a View) -> TimestampingSchema {
        TimestampingSchema { view: view }
    }

    pub fn contents(&self) -> MerklePatriciaTable<MapTable<View, [u8], Vec<u8>>, Hash, Content> {
        MerklePatriciaTable::new(MapTable::new(vec![TIMESTAMPING_SERVICE_ID as u8, 0], self.view))
    }

    pub fn state_hash(&self) -> Result<Vec<Hash>, StorageError> {
        Ok(vec![self.contents().root_hash()?])
    }
}

impl Transaction for TimestampTx {
    fn verify(&self) -> bool {
        true
    }

    fn execute(&self, view: &View) -> Result<(), StorageError> {
        let schema = TimestampingSchema::new(view);
        let content = Content::new(self.description(), self.time(), self.hash());
        schema.contents().put(self.hash(), content)
    }

    fn info(&self) -> Value {
        serde_json::to_value(self).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use chrono::UTC;

    use exonum::crypto::{gen_keypair, hash};

    use super::{TimestampTx, Content};

    #[test]
    fn test_timestamp_tx() {
        let description = "Test Description";
        let time = UTC::now().timestamp();
        let hash = hash(b"js9sdhcsdh32or830ru8043ru-wf9-12u8u3280y8hfwoefnsdljs");
        let (_, sec_key) = gen_keypair();

        let tx = TimestampTx::new(description, time, &hash, &sec_key);
        assert_eq!(tx.description(), description);
        assert_eq!(tx.time(), time);
        assert_eq!(tx.hash(), &hash);
    }

    #[test]
    fn test_file_content() {
        let description = "Test Description";
        let time = UTC::now().timestamp();
        let hash = hash(b"js9sdhcsdh32or830ru8043ru-wf9-12u8u3280y8hfwoefnsdljs");

        let content = Content::new(description, time, &hash);
        assert_eq!(content.description(), description);
        assert_eq!(content.time(), time);
        assert_eq!(content.data_hash(), &hash);
    }
}
