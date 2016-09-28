#![feature(type_ascription)]
#![feature(custom_derive)]
#![feature(plugin)]
#![plugin(serde_macros)]
#![feature(question_mark)]

extern crate rand;
extern crate time;
extern crate serde;
extern crate byteorder;
#[macro_use]
extern crate log;

#[macro_use(message)]
extern crate exonum;
extern crate utils;

pub mod api;
pub mod wallet;

use std::ops::Deref;

use byteorder::{ByteOrder, LittleEndian};

use exonum::messages::{RawMessage, Message, Error as MessageError};
use exonum::crypto::{PublicKey, Hash, hash};
use exonum::storage::{Map, Database, Fork, Error, MerklePatriciaTable, MapTable, MerkleTable, List};
use exonum::blockchain::{Blockchain, View};

use wallet::{Wallet, WalletId};

pub const TX_TRANSFER_ID: u16 = 128;
pub const TX_ISSUE_ID: u16 = 129;
pub const TX_WALLET_ID: u16 = 130;

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
            CurrencyTx::CreateWallet(ref msg) => msg.raw(),
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
        MerkleTable::new(MapTable::new(vec![20], &self))
    }

    pub fn wallet_ids(&self) -> MerklePatriciaTable<MapTable<F, [u8], Vec<u8>>, PublicKey, u64> {
        MerklePatriciaTable::new(MapTable::new(vec![21], &self))
    }

    pub fn wallet(&self, pub_key: &PublicKey) -> Result<Option<(WalletId, Wallet)>, Error> {
        if let Some(id) = self.wallet_ids().get(pub_key)? {
            let wallet_pair = self.wallets().get(id)?.map(|wallet| (id, wallet));
            return Ok(wallet_pair);
        }
        Ok(None)
    }

    pub fn wallet_history(&self,
                          id: WalletId)
                          -> MerkleTable<MapTable<F, [u8], Vec<u8>>, u64, Hash> {
        let mut prefix = vec![22; 9];
        LittleEndian::write_u64(&mut prefix[1..], id);
        MerkleTable::new(MapTable::new(prefix, &self))
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
        let push_if_some = |vec: &mut Vec<Vec<u8>>, option: Option<Hash>| {
            if let Some(hash) = option {
                vec.push(hash.as_ref().to_vec());
            }
        };

        let wallets = view.wallets();
        let wallet_ids = view.wallet_ids();

        let mut hashes = Vec::new();
        push_if_some(&mut hashes, wallets.root_hash()?);
        push_if_some(&mut hashes, wallet_ids.root_hash()?);
        for item in wallets.values()? {
            if let Some((id, _)) = view.wallet(item.pub_key())? {
                let history = view.wallet_history(id);
                push_if_some(&mut hashes, history.root_hash()?);
            }
        }
        Ok(hash(&hashes.concat()))
    }

    fn execute(view: &Self::View, tx: &Self::Transaction) -> Result<(), Error> {
        let tx_hash = tx.hash();
        match *tx {
            CurrencyTx::Transfer(ref msg) => {
                let from = view.wallet(msg.from())?;
                let to = view.wallet(msg.to())?;
                if let (Some(mut from), Some(mut to)) = (from, to) {
                    if from.1.amount() < msg.amount() {
                        return Ok(());
                    }

                    from.1.transfer_to(&mut to.1, msg.amount());
                    view.wallets().set(from.0, from.1)?;
                    view.wallets().set(to.0, to.1)?;

                    view.wallet_history(from.0).append(tx_hash)?;
                    view.wallet_history(to.0).append(tx_hash)?;
                }
            }
            CurrencyTx::Issue(ref msg) => {
                if let Some((id, mut wallet)) = view.wallet(msg.wallet())? {
                    let new_amount = wallet.amount() + msg.amount();
                    wallet.set_amount(new_amount);
                    view.wallets().set(id, wallet)?;
                    view.wallet_history(id).append(tx_hash)?;
                }
            }
            CurrencyTx::CreateWallet(ref msg) => {
                if let Some(_) = view.wallet_ids().get(msg.pub_key())? {
                    return Ok(());
                }

                let wallet = Wallet::new(msg.pub_key(), msg.name(), 0);
                let id = view.wallets().len()?;

                view.wallets().append(wallet)?;
                view.wallet_ids().put(msg.pub_key(), id)?;
                view.wallet_history(id).append(tx_hash)?;
            }
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use byteorder::{ByteOrder, LittleEndian};

    use exonum::crypto::gen_keypair;
    use exonum::storage::MemoryDB;
    use exonum::blockchain::Blockchain;
    use exonum::messages::Message;

    use super::{CurrencyTx, CurrencyBlockchain, TxCreateWallet, TxIssue, TxTransfer};

    #[test]
    fn test_wallet_prefix() {
        let id = 4096;
        let mut prefix = vec![10; 9];
        LittleEndian::write_u64(&mut prefix[1..], id);
        assert_eq!(prefix, vec![10, 0, 16, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn test_wallet_history() {
        let b = CurrencyBlockchain { db: MemoryDB::new() };
        let v = b.view();

        let (p1, s1) = gen_keypair();
        let (p2, s2) = gen_keypair();

        let cw1 = TxCreateWallet::new(&p1, "tx1", &s1);
        let cw2 = TxCreateWallet::new(&p2, "tx2", &s2);
        CurrencyBlockchain::<MemoryDB>::execute(&v, &CurrencyTx::CreateWallet(cw1.clone()))
            .unwrap();
        CurrencyBlockchain::<MemoryDB>::execute(&v, &CurrencyTx::CreateWallet(cw2.clone()))
            .unwrap();
        let w1 = v.wallet(&p1).unwrap().unwrap();
        let w2 = v.wallet(&p2).unwrap().unwrap();

        assert_eq!(w1.0, 0);
        assert_eq!(w2.0, 1);
        assert_eq!(w1.1.name(), "tx1");
        assert_eq!(w1.1.amount(), 0);
        assert_eq!(w2.1.name(), "tx2");
        assert_eq!(w2.1.amount(), 0);

        let iw1 = TxIssue::new(&p1, 1000, 1, &s1);
        let iw2 = TxIssue::new(&p2, 100, 2, &s2);
        CurrencyBlockchain::<MemoryDB>::execute(&v, &CurrencyTx::Issue(iw1.clone())).unwrap();
        CurrencyBlockchain::<MemoryDB>::execute(&v, &CurrencyTx::Issue(iw2.clone())).unwrap();
        let w1 = v.wallet(&p1).unwrap().unwrap();
        let w2 = v.wallet(&p2).unwrap().unwrap();

        assert_eq!(w1.1.amount(), 1000);
        assert_eq!(w2.1.amount(), 100);

        let tw = TxTransfer::new(&p1, &p2, 400, 3, &s1);
        CurrencyBlockchain::<MemoryDB>::execute(&v, &CurrencyTx::Transfer(tw.clone())).unwrap();
        let w1 = v.wallet(&p1).unwrap().unwrap();
        let w2 = v.wallet(&p2).unwrap().unwrap();

        assert_eq!(w1.1.amount(), 600);
        assert_eq!(w2.1.amount(), 500);

        let h1 = v.wallet_history(w1.0).values().unwrap();
        let h2 = v.wallet_history(w2.0).values().unwrap();
        assert_eq!(h1, vec![cw1.hash(), iw1.hash(), tw.hash()]);
        assert_eq!(h2, vec![cw2.hash(), iw2.hash(), tw.hash()]);
    }
}
