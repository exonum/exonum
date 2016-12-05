#[macro_use(message, storage_value)]
extern crate exonum;
extern crate blockchain_explorer;
extern crate serde;
extern crate time;

use std::ops::Deref;

use serde::{Serialize, Serializer};
use time::Timespec;

use exonum::crypto::{Hash, hash, HexValue};
use exonum::storage::{Database, Fork, Error, MapTable, MerklePatriciaTable, Map};
use exonum::blockchain::{View, Blockchain};

use blockchain_explorer::TransactionInfo;

pub const TIMESTAMPING_TRANSACTION_MESSAGE_ID: u16 = 128;
pub const TIMESTAMPING_FILE_SIZE_LIMIT: u64 = 20 * 1024 * 1024;

message! {
    TimestampTx {
        const ID = TIMESTAMPING_TRANSACTION_MESSAGE_ID;
        const SIZE = 72;

        file_name:      &str        [00 => 08]
        description:    &str        [08 => 16]
        mime:           &str        [16 => 24]
        time:           Timespec    [24 => 32]
        hash:           &Hash       [32 => 64]
        data:           &[u8]       [64 => 72]
    }
}

// impl TimestampTx {
//     pub fn from_file(file_name: &str, file: &File) -> Option<TimestampTx> {
//         let ts = time::now_utc().to_timespec();

//         let mut tx = TimestampTx::
//     }
// }

storage_value! {
    Content {
        const SIZE = 72;

        file_name:          &str        [00 => 08]
        description:        &str        [08 => 16]
        mime:               &str        [16 => 24]
        time:               Timespec    [24 => 32]
        data_hash:          &Hash       [32 => 64]
        data:               &[u8]       [64 => 72]
    }
}

impl Serialize for TimestampTx {
    fn serialize<S>(&self, ser: &mut S) -> Result<(), S::Error>
        where S: Serializer
    {
        let mut state = ser.serialize_struct("transaction", 4)?;
        ser.serialize_struct_elt(&mut state, "file_name", self.file_name())?;
        ser.serialize_struct_elt(&mut state, "description", self.description())?;
        ser.serialize_struct_elt(&mut state, "time", self.time().sec)?;
        ser.serialize_struct_elt(&mut state, "hash", self.hash().to_hex())?;
        ser.serialize_struct_end(state)
    }
}

impl Serialize for Content {
    fn serialize<S>(&self, ser: &mut S) -> Result<(), S::Error>
        where S: Serializer
    {
        let mut state = ser.serialize_struct("content", 4)?;
        ser.serialize_struct_elt(&mut state, "file_name", self.file_name())?;
        ser.serialize_struct_elt(&mut state, "description", self.description())?;
        ser.serialize_struct_elt(&mut state, "time", self.time().sec)?;
        ser.serialize_struct_elt(&mut state, "hash", self.data_hash().to_hex())?;
        ser.serialize_struct_end(state)
    }
}

impl TransactionInfo for TimestampTx {}

#[derive(Clone)]
pub struct TimestampingBlockchain<D: Database> {
    pub db: D,
}

pub struct TimestampingView<F: Fork> {
    pub fork: F,
}

impl<F> View<F> for TimestampingView<F>
    where F: Fork
{
    type Transaction = TimestampTx;

    fn from_fork(fork: F) -> Self {
        TimestampingView { fork: fork }
    }
}

impl<F> Deref for TimestampingView<F>
    where F: Fork
{
    type Target = F;

    fn deref(&self) -> &Self::Target {
        &self.fork
    }
}

impl<D: Database> Deref for TimestampingBlockchain<D> {
    type Target = D;

    fn deref(&self) -> &D {
        &self.db
    }
}

impl<F> TimestampingView<F>
    where F: Fork
{
    pub fn contents(&self) -> MerklePatriciaTable<MapTable<F, [u8], Vec<u8>>, Hash, Content> {
        MerklePatriciaTable::new(MapTable::new(vec![21], &self))
    }
}

impl<D> Blockchain for TimestampingBlockchain<D>
    where D: Database
{
    type Database = D;
    type Transaction = TimestampTx;
    type View = TimestampingView<D::Fork>;

    fn verify_tx(tx: &Self::Transaction) -> bool {
        tx.data().len() < TIMESTAMPING_FILE_SIZE_LIMIT as usize
    }

    fn state_hash(view: &Self::View) -> Result<Hash, Error> {
        let contents = view.contents();

        let mut hashes = Vec::new();
        hashes.extend_from_slice(contents.root_hash()?.as_ref());
        Ok(hash(&hashes))
    }

    fn execute(view: &Self::View, tx: &Self::Transaction) -> Result<(), Error> {
        let file = Content::new(tx.file_name(),
                                tx.description(),
                                tx.mime(),
                                tx.time(),
                                tx.hash(),
                                tx.data());
        view.contents().put(tx.hash(), file)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use time;

    use exonum::crypto::{gen_keypair, hash};

    use super::{TimestampTx, Content};

    #[test]
    fn test_timestamp_tx() {
        let file_name = "Проверка файла с кучей буков.jpg";
        let description = "Test Description";
        let mime = "application/json";
        let time = time::now().to_timespec();
        let data = b"js9sdhcsdh32or830ru8043ru-wf9-12u8u3280y8hfwoefnsdljs";
        let hash = hash(data);
        let (_, sec_key) = gen_keypair();

        let tx = TimestampTx::new(file_name, description, mime, time, &hash, data, &sec_key);
        assert_eq!(tx.file_name(), file_name);
        assert_eq!(tx.description(), description);
        assert_eq!(tx.mime(), mime);
        assert_eq!(tx.time(), time);
        assert_eq!(tx.hash(), &hash);
        assert_eq!(tx.data(), data.as_ref());
    }

    #[test]
    fn test_file_content() {
        let file_name = "Проверка файла с кучей буков.jpg";
        let description = "Test Description";
        let mime = "application/json";
        let time = time::now().to_timespec();
        let data = b"js9sdhcsdh32or830ru8043ru-wf9-12u8u3280y8hfwoefnsdljs";
        let hash = hash(data);

        let content = Content::new(file_name, description, mime, time, &hash, data);
        assert_eq!(content.file_name(), file_name);
        assert_eq!(content.description(), description);
        assert_eq!(content.mime(), mime);
        assert_eq!(content.time(), time);
        assert_eq!(content.data_hash(), &hash);
        assert_eq!(content.data(), data.as_ref());
    }
}