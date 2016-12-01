#[macro_use(message, storage_value)]
extern crate exonum;
extern crate blockchain_explorer;
extern crate serde;

use std::ops::Deref;

use serde::{Serialize, Serializer};

use exonum::messages::Message;
use exonum::crypto::{PublicKey, Hash, hash, HexValue};
use exonum::storage::{Database, Fork, Error, MapTable, MerklePatriciaTable, Map};
use exonum::blockchain::{View, Blockchain};

use blockchain_explorer::TransactionInfo;

pub const TIMESTAMPING_TRANSACTION_MESSAGE_ID: u16 = 128;
pub const TIMESTAMPING_FILE_SIZE_LIMIT: u32 = 10 * 1024 * 1024;

message! {
    TimestampTx {
        const ID = TIMESTAMPING_TRANSACTION_MESSAGE_ID;
        const SIZE = 40;

        file_name:      &str        [00 => 08]
        data:           &[u8]       [08 => 16]
    }
}

storage_value! {
    File {
        const SIZE = 24;

        file_name:          &str        [00 => 08]
        data:               &[u8]       [16 => 24]
    }
}

impl Serialize for TimestampTx {
    fn serialize<S>(&self, ser: &mut S) -> Result<(), S::Error>
        where S: Serializer
    {
        let mut state = ser.serialize_struct("transaction", 4)?;
        ser.serialize_struct_elt(&mut state, "file_name", self.file_name())?;
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
    pub fn files(&self) -> MerklePatriciaTable<MapTable<F, [u8], Vec<u8>>, Hash, File> {
        MerklePatriciaTable::new(MapTable::new(vec![21], &self))
    }
}

impl<D> Blockchain for TimestampingBlockchain<D>
    where D: Database
{
    type Database = D;
    type Transaction = TimestampTx;
    type View = TimestampingView<D::Fork>;

    fn verify_tx(_: &Self::Transaction) -> bool {
        true
    }

    fn state_hash(view: &Self::View) -> Result<Hash, Error> {
        let files = view.files();

        let mut hashes = Vec::new();
        hashes.extend_from_slice(files.root_hash()?.as_ref());
        Ok(hash(&hashes))
    }

    fn execute(view: &Self::View, tx: &Self::Transaction) -> Result<(), Error> {
        let content = tx.data();
        let hash = hash(content);

        let file = File::new(tx.file_name(), content);
        view.files().put(&hash, file)?;
        Ok(())
    }
}