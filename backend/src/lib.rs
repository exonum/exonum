#[macro_use(message)]
extern crate exonum;
extern crate blockchain_explorer;
extern crate serde;

use std::ops::Deref;

use serde::{Serialize, Serializer};

use exonum::messages::Message;
use exonum::crypto::{PublicKey, Hash, hash, HexValue};
use exonum::storage::{Database, Fork, Error};
use exonum::blockchain::{View, Blockchain};

use blockchain_explorer::TransactionInfo;

pub const TIMESTAMPING_TRANSACTION_MESSAGE_ID: u16 = 128;
pub const TIMESTAMPING_FILE_SIZE_LIMIT: u32 = 10 * 1024 * 1024;

message! {
    TimestampTx {
        const ID = TIMESTAMPING_TRANSACTION_MESSAGE_ID;
        const SIZE = 40;

        pub_key:        &PublicKey  [00 => 32]
        data:           &[u8]       [32 => 40]
    }
}

impl Serialize for TimestampTx {
    fn serialize<S>(&self, ser: &mut S) -> Result<(), S::Error>
        where S: Serializer
    {
        let mut state = ser.serialize_struct("transaction", 4)?;
        ser.serialize_struct_elt(&mut state, "pub_key", self.pub_key().to_hex())?;
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

impl<D> Blockchain for TimestampingBlockchain<D>
    where D: Database
{
    type Database = D;
    type Transaction = TimestampTx;
    type View = TimestampingView<D::Fork>;

    fn verify_tx(tx: &Self::Transaction) -> bool {
        tx.verify(tx.pub_key())
    }

    fn state_hash(_: &Self::View) -> Result<Hash, Error> {
        Ok(hash(&[]))
    }

    fn execute(_: &Self::View, _: &Self::Transaction) -> Result<(), Error> {
        Ok(())
    }
}