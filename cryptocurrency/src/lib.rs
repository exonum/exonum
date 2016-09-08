#![feature(question_mark)]

#[macro_use(message)]
extern crate exonum;

use std::borrow::{Borrow, BorrowMut};

use exonum::messages::{RawMessage, Message, Error as MessageError};
use exonum::crypto::{PublicKey, Hash, hash};
use exonum::storage::{Map, Database, Fork, Error, MerklePatriciaTable, MapTable};
use exonum::blockchain::Blockchain;

pub const TX_TRANSFER_ID: u16 = 128;
pub const TX_ISSUE_ID: u16 = 129;

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


pub struct CurrencyBlockchain<D: Database> {
    pub db: D,
}

impl<D: Database> Borrow<D> for CurrencyBlockchain<D> {
    fn borrow(&self) -> &D {
        &self.db
    }
}

impl<D: Database> BorrowMut<D> for CurrencyBlockchain<D> {
    fn borrow_mut(&mut self) -> &mut D {
        &mut self.db
    }
}


impl<D> Blockchain for CurrencyBlockchain<D>
    where D: Database
{
    type Database = D;
    type Transaction = CurrencyTx;

    fn verify_tx(tx: &Self::Transaction) -> bool {
        tx.verify(tx.pub_key())
    }

    fn state_hash(fork: &mut Fork<Self::Database>) -> Result<Hash, Error> {
        fork.wallets().root_hash().map(|o| o.unwrap_or(hash(&[])))
    }

    fn execute(fork: &mut Fork<Self::Database>, tx: &Self::Transaction) -> Result<(), Error> {
        match *tx {
            CurrencyTx::Transfer(ref msg) => {
                let from_amount = {
                    fork.wallets().get(msg.from())?.unwrap_or(0)
                };

                // if from_amount < msg.amount() {
                //     return Ok(())
                // }

                let to_amount = {
                    fork.wallets().get(msg.to())?.unwrap_or(0)
                };

                fork.wallets().put(msg.from(), from_amount - msg.amount())?;
                fork.wallets().put(msg.to(), to_amount + msg.amount())?;
            }
            CurrencyTx::Issue(ref msg) => {
                let amount = {
                    fork.wallets().get(msg.wallet())?.unwrap_or(0) + msg.amount()
                };
                fork.wallets().put(msg.wallet(), amount)?;
            }
        };
        Ok(())
    }
}

trait WalletStorage<Db: Database>
    where Self: Borrow<Db> + BorrowMut<Db>
{
    fn wallets(&mut self) -> MerklePatriciaTable<MapTable<Db, [u8], Vec<u8>>, PublicKey, i64>;
}

impl<'a, Db> WalletStorage<Fork<'a, Db>> for Fork<'a, Db>
    where Db: Database
{
    fn wallets(&mut self) -> MerklePatriciaTable<MapTable<Self, [u8], Vec<u8>>, PublicKey, i64> {
        MerklePatriciaTable::new(MapTable::new(vec![09], self))
    }
}
