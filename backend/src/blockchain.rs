use serde::{Serialize, Serializer};

use exonum::crypto::{Hash, hash, HexValue};
use exonum::storage::View;
use exonum::blockchain::Transaction;
use exonum::storage::{MapTable, MerklePatriciaTable, Map, Error as StorageError};

use TIMESTAMPING_SERVICE_ID;
use blockchain_explorer::TransactionInfo;

pub const TIMESTAMPING_TX_ID: u16 = 0;

message! {
    TimestampTx {
        const TYPE = TIMESTAMPING_SERVICE_ID;
        const ID = TIMESTAMPING_TX_ID;
        const SIZE = 48;

        description:    &str        [00 => 08]
        time:           i64         [08 => 16]
        hash:           &Hash       [16 => 48]
    }
}

storage_value! {
    Content {
        const SIZE = 48;

        description:        &str        [00 => 08]
        time:               i64         [08 => 16]
        data_hash:          &Hash       [16 => 48]
    }
}

pub struct TimestampingSchema<'a> {
    view: &'a View,
}

impl Serialize for TimestampTx {
    fn serialize<S>(&self, ser: &mut S) -> Result<(), S::Error>
        where S: Serializer
    {
        let mut state = ser.serialize_struct("transaction", 4)?;
        ser.serialize_struct_elt(&mut state, "description", self.description())?;
        ser.serialize_struct_elt(&mut state, "time", self.time())?;
        ser.serialize_struct_elt(&mut state, "hash", self.hash().to_hex())?;
        ser.serialize_struct_end(state)
    }
}

impl Serialize for Content {
    fn serialize<S>(&self, ser: &mut S) -> Result<(), S::Error>
        where S: Serializer
    {
        let mut state = ser.serialize_struct("content", 4)?;
        ser.serialize_struct_elt(&mut state, "description", self.description())?;
        ser.serialize_struct_elt(&mut state, "time", self.time())?;
        ser.serialize_struct_elt(&mut state, "hash", self.data_hash().to_hex())?;
        ser.serialize_struct_end(state)
    }
}

impl TransactionInfo for TimestampTx {}

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