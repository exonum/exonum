#![feature(question_mark)]

#[macro_use(message)]
extern crate exonum;

use std::ops::Deref;
use std::borrow::{Borrow};

use exonum::messages::Message;
use exonum::crypto::{PublicKey, Hash, hash};
use exonum::storage::{Database, Fork, Error};
use exonum::blockchain::{View, Blockchain};

pub const TIMESTAMPING_TRANSACTION_MESSAGE_ID: u16 = 128;

message! {
    TimestampTx {
        const ID = TIMESTAMPING_TRANSACTION_MESSAGE_ID;
        const SIZE = 40;

        pub_key:        &PublicKey  [00 => 32]
        data:           &[u8]       [32 => 40]
    }
}

pub struct TimestampingBlockchain<D: Database> {
    pub db: D,
}

pub struct TimestampingView<F: Fork> {
    pub fork: F
}

impl<F> View<F> for TimestampingView<F>
    where F: Fork
{
    type Transaction = TimestampTx;

    fn from_fork(fork: F) -> Self {
        TimestampingView {
            fork: fork
        }
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

impl<D: Database> Borrow<D> for TimestampingBlockchain<D> {
    fn borrow(&self) -> &D {
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

    fn state_hash(_: &mut Self::View) -> Result<Hash, Error> {
        Ok(hash(&[]))
    }

    fn execute(_: &mut Self::View, _: &Self::Transaction) -> Result<(), Error> {
        Ok(())
    }
}
