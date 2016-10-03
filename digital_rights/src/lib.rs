#![feature(question_mark)]
#![feature(associated_consts)]

#[macro_use(message)]
extern crate exonum;
extern crate time;

mod txs;

use std::ops::Deref;

use exonum::messages::{RawMessage, Message, Error as MessageError};
use exonum::crypto::{PublicKey, Hash, hash};
use exonum::storage::{Map, Database, Fork, Error, MerklePatriciaTable, MapTable};
use exonum::blockchain::{Blockchain, View};

use txs::DigitalRightsTx;

#[derive(Clone)]
pub struct DigitalRightsBlockchain<D: Database> {
    pub db: D,
}

pub struct DigitalRightsView<F: Fork> {
    pub fork: F,
}

impl<F> View<F> for DigitalRightsView<F>
    where F: Fork
{
    type Transaction = DigitalRightsTx;

    fn from_fork(fork: F) -> Self {
        DigitalRightsView { fork: fork }
    }
}

impl<F> Deref for DigitalRightsView<F>
    where F: Fork
{
    type Target = F;

    fn deref(&self) -> &Self::Target {
        &self.fork
    }
}

impl<D: Database> Deref for DigitalRightsBlockchain<D> {
    type Target = D;

    fn deref(&self) -> &D {
        &self.db
    }
}

impl<F> DigitalRightsView<F> where F: Fork {}

impl<D> Blockchain for DigitalRightsBlockchain<D>
    where D: Database
{
    type Database = D;
    type Transaction = DigitalRightsTx;
    type View = DigitalRightsView<D::Fork>;

    fn verify_tx(tx: &Self::Transaction) -> bool {
        tx.verify(tx.pub_key())
    }

    fn state_hash(view: &Self::View) -> Result<Hash, Error> {
        unimplemented!();
    }

    fn execute(view: &Self::View, tx: &Self::Transaction) -> Result<(), Error> {
        unimplemented!();
        Ok(())
    }
}
