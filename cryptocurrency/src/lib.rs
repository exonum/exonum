#![feature(type_ascription)]
#![feature(custom_derive)]
#![feature(plugin)]
#![plugin(serde_macros)]
#![feature(question_mark)]

extern crate rand;
extern crate time;
extern crate serde;
extern crate toml;
#[macro_use]
extern crate log;

#[macro_use(message)]
extern crate exonum;

use std::ops::Deref;

use exonum::messages::{RawMessage, Message, Error as MessageError};
use exonum::crypto::{PublicKey, Hash, hash};
use exonum::storage::{Map, Database, Fork, Error, MerklePatriciaTable, MapTable};
use exonum::blockchain::{Blockchain, View};

pub const TX_TRANSFER_ID: u16 = 128;
pub const TX_ISSUE_ID: u16 = 129;

pub mod config;
pub mod config_file;

message! {
    TxTransfer {
        const ID = TX_TRANSFER_ID;
        const SIZE = 80;

        from:        &PublicKey  [00 => 32]
        to:          &PublicKey  [32 => 64]
        amount:      i64         [64 => 72]
        seed:        u64         [72 => 80]
    }
}

message! {
    TxIssue {
        const ID = TX_ISSUE_ID;
        const SIZE = 48;

        wallet:      &PublicKey  [00 => 32]
        amount:      i64         [32 => 40]
        seed:        u64         [40 => 48]
    }
}

#[derive(PartialEq, Debug, Clone)]
pub enum CurrencyTx {
    Transfer(TxTransfer),
    Issue(TxIssue),
}

impl Message for CurrencyTx {
    fn raw(&self) -> &RawMessage {
        match *self {
            CurrencyTx::Transfer(ref msg) => msg.raw(),
            CurrencyTx::Issue(ref msg) => msg.raw(),
        }
    }

    fn from_raw(raw: RawMessage) -> Result<Self, MessageError> {
        Ok(match raw.message_type() {
            TX_TRANSFER_ID => CurrencyTx::Transfer(TxTransfer::from_raw(raw)?),
            TX_ISSUE_ID => CurrencyTx::Issue(TxIssue::from_raw(raw)?),
            _ => panic!("Undefined message type"),
        })
    }

    fn hash(&self) -> Hash {
        match *self {
            CurrencyTx::Transfer(ref msg) => msg.hash(),
            CurrencyTx::Issue(ref msg) => msg.hash(),
        }
    }

    fn verify(&self, pub_key: &PublicKey) -> bool {
        match *self {
            CurrencyTx::Transfer(ref msg) => msg.verify(pub_key),
            CurrencyTx::Issue(ref msg) => msg.verify(pub_key),
        }
    }
}

impl CurrencyTx {
    pub fn pub_key(&self) -> &PublicKey {
        match *self {
            CurrencyTx::Transfer(ref msg) => msg.from(),
            CurrencyTx::Issue(ref msg) => msg.wallet(),
        }
    }
}

#[derive(Clone)]
pub struct CurrencyBlockchain<D: Database> {
    pub db: D,
}

pub struct CurrencyView<F: Fork> {
    pub fork: F,
}

impl<F> View<F> for CurrencyView<F>
    where F: Fork
{
    type Transaction = CurrencyTx;

    fn from_fork(fork: F) -> Self {
        CurrencyView { fork: fork }
    }
}

impl<F> Deref for CurrencyView<F>
    where F: Fork
{
    type Target = F;

    fn deref(&self) -> &Self::Target {
        &self.fork
    }
}

impl<D: Database> Deref for CurrencyBlockchain<D> {
    type Target = D;

    fn deref(&self) -> &D {
        &self.db
    }
}

impl<F> CurrencyView<F>
    where F: Fork
{
    fn wallets(&self) -> MerklePatriciaTable<MapTable<F, [u8], Vec<u8>>, PublicKey, i64> {
        MerklePatriciaTable::new(MapTable::new(vec![09], &self))
    }
}

impl<D> Blockchain for CurrencyBlockchain<D>
    where D: Database
{
    type Database = D;
    type Transaction = CurrencyTx;
    type View = CurrencyView<D::Fork>;

    fn verify_tx(tx: &Self::Transaction) -> bool {
        tx.verify(tx.pub_key())
    }

    fn state_hash(view: &Self::View) -> Result<Hash, Error> {
        view.wallets().root_hash().map(|o| o.unwrap_or(hash(&[])))
    }

    fn execute(view: &Self::View, tx: &Self::Transaction) -> Result<(), Error> {
        match *tx {
            CurrencyTx::Transfer(ref msg) => {
                let from_amount = {
                    view.wallets().get(msg.from())?.unwrap_or(0)
                };

                // if from_amount < msg.amount() {
                //     return Ok(())
                // }

                let to_amount = {
                    view.wallets().get(msg.to())?.unwrap_or(0)
                };

                view.wallets().put(msg.from(), from_amount - msg.amount())?;
                view.wallets().put(msg.to(), to_amount + msg.amount())?;
            }
            CurrencyTx::Issue(ref msg) => {
                let amount = {
                    view.wallets().get(msg.wallet())?.unwrap_or(0) + msg.amount()
                };
                view.wallets().put(msg.wallet(), amount)?;
            }
        };
        Ok(())
    }
}
