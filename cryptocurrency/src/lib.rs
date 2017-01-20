#![feature(type_ascription)]

extern crate rand;
extern crate time;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate byteorder;
#[macro_use]
extern crate log;
#[cfg(test)]
extern crate tempdir;

#[macro_use(message, storage_value)]
extern crate exonum;
extern crate blockchain_explorer;

pub mod api;
pub mod wallet;

use byteorder::{ByteOrder, LittleEndian};

use exonum::messages::{RawMessage, RawTransaction, FromRaw, Message, Error as MessageError};
use exonum::crypto::{PublicKey, Hash, hash};
use exonum::storage::{Map, Error, MerklePatriciaTable, MapTable, MerkleTable, List,
                      View as StorageView};
use exonum::blockchain::{Service, Transaction};
use exonum::node::State;

use wallet::{Wallet, WalletId};

pub const CRYPTOCURRENCY: u16 = 128;

pub const TX_TRANSFER_ID: u16 = 128;
pub const TX_ISSUE_ID: u16 = 129;
pub const TX_WALLET_ID: u16 = 130;

message! {
    TxTransfer {
        const TYPE = CRYPTOCURRENCY;
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
        const TYPE = CRYPTOCURRENCY;
        const ID = TX_ISSUE_ID;
        const SIZE = 48;

        wallet:      &PublicKey  [00 => 32]
        amount:      i64         [32 => 40]
        seed:        u64         [40 => 48]
    }
}

message! {
    TxCreateWallet {
        const TYPE = CRYPTOCURRENCY;
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

impl CurrencyTx {
    pub fn pub_key(&self) -> &PublicKey {
        match *self {
            CurrencyTx::Transfer(ref msg) => msg.from(),
            CurrencyTx::Issue(ref msg) => msg.wallet(),
            CurrencyTx::CreateWallet(ref msg) => msg.pub_key(),
        }
    }
}

impl Message for CurrencyTx {
    fn raw(&self) -> &RawMessage {
        match *self {
            CurrencyTx::Transfer(ref msg) => msg.raw(),
            CurrencyTx::Issue(ref msg) => msg.raw(),
            CurrencyTx::CreateWallet(ref msg) => msg.raw(),
        }
    }

    fn hash(&self) -> Hash {
        match *self {
            CurrencyTx::Transfer(ref msg) => msg.hash(),
            CurrencyTx::Issue(ref msg) => msg.hash(),
            CurrencyTx::CreateWallet(ref msg) => msg.hash(),
        }
    }

    fn verify_signature(&self, pub_key: &PublicKey) -> bool {
        match *self {
            CurrencyTx::Transfer(ref msg) => msg.verify_signature(pub_key),
            CurrencyTx::Issue(ref msg) => msg.verify_signature(pub_key),
            CurrencyTx::CreateWallet(ref msg) => msg.verify_signature(pub_key),
        }
    }
}

impl FromRaw for CurrencyTx {
    fn from_raw(raw: RawMessage) -> Result<Self, MessageError> {
        match raw.message_type() {
            TX_TRANSFER_ID => Ok(CurrencyTx::Transfer(TxTransfer::from_raw(raw)?)),
            TX_ISSUE_ID => Ok(CurrencyTx::Issue(TxIssue::from_raw(raw)?)),
            TX_WALLET_ID => Ok(CurrencyTx::CreateWallet(TxCreateWallet::from_raw(raw)?)),
            _ => Err(MessageError::IncorrectMessageType { message_type: raw.message_type() }),
        }
    }
}

impl From<TxTransfer> for CurrencyTx {
    fn from(tx: TxTransfer) -> CurrencyTx {
        CurrencyTx::Transfer(tx)
    }
}
impl From<TxCreateWallet> for CurrencyTx {
    fn from(tx: TxCreateWallet) -> CurrencyTx {
        CurrencyTx::CreateWallet(tx)
    }
}
impl From<TxIssue> for CurrencyTx {
    fn from(tx: TxIssue) -> CurrencyTx {
        CurrencyTx::Issue(tx)
    }
}
impl From<RawMessage> for CurrencyTx {
    fn from(raw: RawMessage) -> Self {
        CurrencyTx::from_raw(raw).unwrap()
    }
}

pub struct CurrencySchema<'a> {
    view: &'a StorageView,
}

impl<'a> CurrencySchema<'a> {
    pub fn new(view: &'a StorageView) -> CurrencySchema {
        CurrencySchema { view: view }
    }

    pub fn wallets(&self) -> MerkleTable<MapTable<StorageView, [u8], Vec<u8>>, u64, Wallet> {
        MerkleTable::new(MapTable::new(vec![20], self.view))
    }

    pub fn wallet_ids
        (&self)
         -> MerklePatriciaTable<MapTable<StorageView, [u8], Vec<u8>>, PublicKey, u64> {
        MerklePatriciaTable::new(MapTable::new(vec![21], self.view))
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
                          -> MerkleTable<MapTable<StorageView, [u8], Vec<u8>>, u64, Hash> {
        let mut prefix = vec![22; 9];
        LittleEndian::write_u64(&mut prefix[1..], id);
        MerkleTable::new(MapTable::new(prefix, self.view))
    }
}

pub struct CurrencyService {}

impl CurrencyService {
    pub fn new() -> CurrencyService {
        CurrencyService {}
    }
}

impl Transaction for CurrencyTx {
    fn verify(&self) -> bool {
        self.verify_signature(self.pub_key())
    }

    fn execute(&self, view: &StorageView) -> Result<(), Error> {
        let tx_hash = Message::hash(self);

        let schema = CurrencySchema::new(view);
        match *self {
            CurrencyTx::Transfer(ref msg) => {
                let from = schema.wallet(msg.from())?;
                let to = schema.wallet(msg.to())?;
                if let (Some(mut from), Some(mut to)) = (from, to) {
                    if from.1.balance() < msg.amount() {
                        return Ok(());
                    }
                    let from_history = schema.wallet_history(from.0);
                    let to_history = schema.wallet_history(to.0);
                    from_history.append(tx_hash)?;
                    to_history.append(tx_hash)?;

                    from.1.transfer_to(&mut to.1, msg.amount());
                    from.1.set_history_hash(&from_history.root_hash()?);
                    to.1.set_history_hash(&to_history.root_hash()?);

                    schema.wallets().set(from.0, from.1)?;
                    schema.wallets().set(to.0, to.1)?;
                }
            }
            CurrencyTx::Issue(ref msg) => {
                if let Some((id, mut wallet)) = schema.wallet(msg.wallet())? {
                    let history = schema.wallet_history(id);
                    history.append(tx_hash)?;

                    let new_amount = wallet.balance() + msg.amount();
                    wallet.set_balance(new_amount);
                    wallet.set_history_hash(&history.root_hash()?);
                    schema.wallets().set(id, wallet)?;
                }
            }
            CurrencyTx::CreateWallet(ref msg) => {
                if let Some(_) = schema.wallet_ids().get(msg.pub_key())? {
                    return Ok(());
                }

                let id = schema.wallets().len()?;
                schema.wallet_history(id).append(tx_hash)?;

                let wallet = Wallet::new(msg.pub_key(),
                                         msg.name(),
                                         0,
                                         &schema.wallet_history(id).root_hash()?);
                schema.wallets().append(wallet)?;
                schema.wallet_ids().put(msg.pub_key(), id)?;
            }
        };
        Ok(())
    }
}

impl Service for CurrencyService {
    fn service_id(&self) -> u16 {
        CRYPTOCURRENCY
    }

    fn handle_genesis_block(&self, _: &StorageView) -> Result<(), Error> {
        Ok(())
    }

    fn state_hash(&self, view: &StorageView) -> Result<Hash, Error> {
        let schema = CurrencySchema::new(view);
        let wallets = schema.wallets();
        let wallet_ids = schema.wallet_ids();

        let mut hashes = Vec::new();
        hashes.extend_from_slice(wallets.root_hash()?.as_ref());
        hashes.extend_from_slice(wallet_ids.root_hash()?.as_ref());

        Ok(hash(&hashes))
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, MessageError> {
        CurrencyTx::from_raw(raw).map(|tx| Box::new(tx) as Box<Transaction>)
    }

    fn handle_commit(&self,
                     _: &StorageView,
                     _: &mut State)
                     -> Result<Vec<Box<Transaction>>, Error> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use byteorder::{ByteOrder, LittleEndian};

    use exonum::crypto::gen_keypair;
    use exonum::storage::Storage;
    use exonum::blockchain::{Blockchain, Transaction};
    use exonum::messages::{FromRaw, Message};

    use super::{CurrencyTx, CurrencyService, CurrencySchema, TxCreateWallet, TxIssue, TxTransfer};

    #[cfg(feature="memorydb")]
    fn create_db() -> Storage {
        use exonum::storage::MemoryDB;

        MemoryDB::new()
    }

    #[cfg(not(feature="memorydb"))]
    fn create_db() -> Storage {
        use exonum::storage::{LevelDB, LevelDBOptions};
        use tempdir::TempDir;

        let mut options = LevelDBOptions::new();
        options.create_if_missing = true;
        let dir = TempDir::new("cryptocurrency").unwrap();
        LevelDB::new(dir.path(), options).unwrap()
    }

    #[test]
    fn test_tx_create_wallet() {
        let (p, s) = gen_keypair();
        let n = "babd, Юникод еще работает";

        let tx = TxCreateWallet::new(&p, n, &s);
        assert_eq!(tx.pub_key(), &p);
        assert_eq!(tx.name(), n);

        let tx2 = TxCreateWallet::from_raw(tx.raw().clone()).unwrap();
        assert_eq!(tx2.pub_key(), &p);
        assert_eq!(tx2.name(), n);
    }

    #[test]
    fn test_wallet_prefix() {
        let id = 4096;
        let mut prefix = vec![10; 9];
        LittleEndian::write_u64(&mut prefix[1..], id);
        assert_eq!(prefix, vec![10, 0, 16, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn test_wallet_history() {
        let db = create_db();
        let b = Blockchain::new(db, vec![Box::new(CurrencyService::new())]);

        let v = b.view();
        let s = CurrencySchema::new(&v);

        let (p1, s1) = gen_keypair();
        let (p2, s2) = gen_keypair();

        let cw1 = TxCreateWallet::new(&p1, "tx1", &s1);
        let cw2 = TxCreateWallet::new(&p2, "tx2", &s2);
        CurrencyTx::from(cw1.clone()).execute(&v).unwrap();
        CurrencyTx::from(cw2.clone()).execute(&v).unwrap();
        let w1 = s.wallet(&p1).unwrap().unwrap();
        let w2 = s.wallet(&p2).unwrap().unwrap();

        assert_eq!(w1.0, 0);
        assert_eq!(w2.0, 1);
        assert_eq!(w1.1.name(), "tx1");
        assert_eq!(w1.1.balance(), 0);
        assert_eq!(w2.1.name(), "tx2");
        assert_eq!(w2.1.balance(), 0);
        let rh1 = s.wallet_history(w1.0).root_hash().unwrap();
        let rh2 = s.wallet_history(w2.0).root_hash().unwrap();
        assert_eq!(&rh1, w1.1.history_hash());
        assert_eq!(&rh2, w2.1.history_hash());

        let iw1 = TxIssue::new(&p1, 1000, 1, &s1);
        let iw2 = TxIssue::new(&p2, 100, 2, &s2);
        CurrencyTx::from(iw1.clone()).execute(&v).unwrap();
        CurrencyTx::from(iw2.clone()).execute(&v).unwrap();
        let w1 = s.wallet(&p1).unwrap().unwrap();
        let w2 = s.wallet(&p2).unwrap().unwrap();

        assert_eq!(w1.1.balance(), 1000);
        assert_eq!(w2.1.balance(), 100);
        let rh1 = s.wallet_history(w1.0).root_hash().unwrap();
        let rh2 = s.wallet_history(w2.0).root_hash().unwrap();
        assert_eq!(&rh1, w1.1.history_hash());
        assert_eq!(&rh2, w2.1.history_hash());

        let tw = TxTransfer::new(&p1, &p2, 400, 3, &s1);
        CurrencyTx::from(tw.clone()).execute(&v).unwrap();
        let w1 = s.wallet(&p1).unwrap().unwrap();
        let w2 = s.wallet(&p2).unwrap().unwrap();

        assert_eq!(w1.1.balance(), 600);
        assert_eq!(w2.1.balance(), 500);
        let rh1 = s.wallet_history(w1.0).root_hash().unwrap();
        let rh2 = s.wallet_history(w2.0).root_hash().unwrap();
        assert_eq!(&rh1, w1.1.history_hash());
        assert_eq!(&rh2, w2.1.history_hash());

        let h1 = s.wallet_history(w1.0).values().unwrap();
        let h2 = s.wallet_history(w2.0).values().unwrap();
        assert_eq!(h1, vec![cw1.hash(), iw1.hash(), tw.hash()]);
        assert_eq!(h2, vec![cw2.hash(), iw2.hash(), tw.hash()]);
    }
}
