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
use exonum::storage::{Map, Database, Fork, Error, MerklePatriciaTable, 
                      MapTable, MerkleTable, List};
use exonum::blockchain::{Blockchain, View};

use wallet::{Wallet, WalletId};

pub const TX_TRANSFER_ID: u16 = 128;
pub const TX_ISSUE_ID: u16 = 129;
pub const TX_WALLET_ID: u16 = 130;

pub mod config;
pub mod config_file;
pub mod wallet;

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

message! {
    TxCreateWallet {
        const ID = TX_WALLET_ID;
        const SIZE = 40;

        pub_key:     &PublicKey  [00 => 32]
        name:        &str        [32 => 40]
    }
}

#[derive(PartialEq, Debug, Clone)]
pub enum CurrencyTx {
    Transfer(TxTransfer),
    Issue(TxIssue),
    CreateWallet(TxCreateWallet),
}

impl Message for CurrencyTx {
    fn raw(&self) -> &RawMessage {
        match *self {
            CurrencyTx::Transfer(ref msg) => msg.raw(),
            CurrencyTx::Issue(ref msg) => msg.raw(),
            CurrencyTx::CreateWallet(ref msg) => msg.raw()
        }
    }

    fn from_raw(raw: RawMessage) -> Result<Self, MessageError> {
        Ok(match raw.message_type() {
            TX_TRANSFER_ID => CurrencyTx::Transfer(TxTransfer::from_raw(raw)?),
            TX_ISSUE_ID => CurrencyTx::Issue(TxIssue::from_raw(raw)?),
            TX_WALLET_ID => CurrencyTx::CreateWallet(TxCreateWallet::from_raw(raw)?),
            _ => panic!("Undefined message type"),
        })
    }

    fn hash(&self) -> Hash {
        match *self {
            CurrencyTx::Transfer(ref msg) => msg.hash(),
            CurrencyTx::Issue(ref msg) => msg.hash(),
            CurrencyTx::CreateWallet(ref msg) => msg.hash(),
        }
    }

    fn verify(&self, pub_key: &PublicKey) -> bool {
        match *self {
            CurrencyTx::Transfer(ref msg) => msg.verify(pub_key),
            CurrencyTx::Issue(ref msg) => msg.verify(pub_key),
            CurrencyTx::CreateWallet(ref msg) => msg.verify(pub_key),
        }
    }
}

impl CurrencyTx {
    pub fn pub_key(&self) -> &PublicKey {
        match *self {
            CurrencyTx::Transfer(ref msg) => msg.from(),
            CurrencyTx::Issue(ref msg) => msg.wallet(),
            CurrencyTx::CreateWallet(ref msg) => msg.pub_key(),
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
    pub fn wallets(&self) -> MerkleTable<MapTable<F, [u8], Vec<u8>>, u64, Wallet> {
        MerkleTable::new(MapTable::new(vec![09], &self))
    }

    pub fn wallets_by_pub_key(&self) -> MerklePatriciaTable<MapTable<F, [u8], Vec<u8>>, PublicKey, u64> {
        MerklePatriciaTable::new(MapTable::new(vec![10], &self))
    }

    pub fn get_wallet(&self, pub_key: &PublicKey) -> Result<Option<(WalletId, Wallet)>, Error> {
        if let Some(id) = self.wallets_by_pub_key().get(pub_key)? {
            let wallet_pair = self.wallets().get(id)?.map(|wallet| (id, wallet));
            return Ok(wallet_pair)
        }
        Ok(None)
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
                let from = view.get_wallet(msg.from())?;
                let to = view.get_wallet(msg.to())?;
                if let (Some(mut from), Some(mut to)) =(from, to) {
                    if from.1.amount() < msg.amount() {
                        return Ok(());
                    }

                    from.1.transfer_to(&mut to.1, msg.amount());
                    view.wallets().set(from.0, from.1)?;
                    view.wallets().set(to.0, to.1)?;

                    //TODO add history
                }
            }
            CurrencyTx::Issue(ref msg) => {
                if let Some((id, mut wallet)) = view.get_wallet(msg.wallet())? {
                    let new_amount = wallet.amount() + msg.amount();
                    wallet.set_amount(new_amount);
                    view.wallets().set(id, wallet)?;
                }
            }
            CurrencyTx::CreateWallet(ref msg) => {
                let wallet = Wallet::new(msg.pub_key(),
                                         msg.name(),
                                         0);

                let code = view.wallets().len()?;
                view.wallets().append(wallet)?;
                view.wallets_by_pub_key().put(msg.pub_key(), code)?;
            }
        };
        Ok(())
    }
}
