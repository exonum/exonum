//! Cryptocurrency implementation example using [exonum](http://exonum.com/).

// TODO: Uncomment when `encoding_struct!` and `message!` implementation will be updated.
// #![deny(missing_docs)]
#![deny(missing_debug_implementations)]

extern crate rand;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate byteorder;
#[macro_use]
extern crate log;
#[cfg(test)]
extern crate tempdir;
#[macro_use(message, encoding_struct)]
extern crate exonum;
extern crate params;
extern crate router;
extern crate iron;
extern crate bodyparser;

use iron::Handler;
use router::Router;

use std::fmt;

use exonum::messages::{RawMessage, RawTransaction, FromRaw, Message};
use exonum::crypto::{PublicKey, Hash, PUBLIC_KEY_LENGTH};
use exonum::storage::{self, Snapshot, Fork, ListIndex, MapIndex, ProofMapIndex};
use exonum::blockchain::{Service, Transaction, ApiContext};
use exonum::encoding::serialize::json::reexport as serde_json;
use exonum::encoding::Error as StreamStructError;
use serde_json::{Value, to_value};

use wallet::Wallet;
use tx_metarecord::TxMetaRecord;

mod tx_metarecord;

pub mod api;
pub mod wallet;

/// Id for our cryptocurrency messages.
pub const CRYPTOCURRENCY_SERVICE_ID: u16 = 128;

/// `TxTransfer` Id.
pub const TX_TRANSFER_ID: u16 = 128;
/// `TxIssue` Id.
pub const TX_ISSUE_ID: u16 = 129;
/// `TxCreateWallet` Id.
pub const TX_WALLET_ID: u16 = 130;

message! {
/// Transfer `amount` of the currency from one wallet to another.
    struct TxTransfer {
        const TYPE = CRYPTOCURRENCY_SERVICE_ID;
        const ID = TX_TRANSFER_ID;
        const SIZE = 80;

        field from:        &PublicKey  [00 => 32]
        field to:          &PublicKey  [32 => 64]
        field amount:      u64         [64 => 72]
        field seed:        u64         [72 => 80]
    }
}

message! {
/// Issue `amount` of the currency to the `wallet`.
    struct TxIssue {
        const TYPE = CRYPTOCURRENCY_SERVICE_ID;
        const ID = TX_ISSUE_ID;
        const SIZE = 48;

        field wallet:      &PublicKey  [00 => 32]
        field amount:      u64         [32 => 40]
        field seed:        u64         [40 => 48]
    }
}

message! {
/// Create wallet with the given `name`.
    struct TxCreateWallet {
        const TYPE = CRYPTOCURRENCY_SERVICE_ID;
        const ID = TX_WALLET_ID;
        const SIZE = 40;

        field pub_key:     &PublicKey  [00 => 32]
        field name:        &str        [32 => 40]
    }
}

/// Transaction types.
#[serde(untagged)]
#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub enum CurrencyTx {
    /// Transfer currency from one wallet to another.
    Transfer(TxTransfer),
    /// Issue currency to some wallet.
    Issue(TxIssue),
    /// Create wallet with given name.
    CreateWallet(TxCreateWallet),
}

impl CurrencyTx {
    /// Returns public key from the transaction.
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
}

impl FromRaw for CurrencyTx {
    fn from_raw(raw: RawMessage) -> Result<Self, StreamStructError> {
        match raw.message_type() {
            TX_TRANSFER_ID => Ok(CurrencyTx::Transfer(TxTransfer::from_raw(raw)?)),
            TX_ISSUE_ID => Ok(CurrencyTx::Issue(TxIssue::from_raw(raw)?)),
            TX_WALLET_ID => Ok(CurrencyTx::CreateWallet(TxCreateWallet::from_raw(raw)?)),
            _ => Err(StreamStructError::IncorrectMessageType { message_type: raw.message_type() }),
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

/// Database schema for the cryptocurrency.
pub struct CurrencySchema<T> {
    view: T,
}

impl<T> fmt::Debug for CurrencySchema<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "CurrencySchema {{}}")
    }
}

impl<T> CurrencySchema<T>
    where T: AsRef<Snapshot>
{
    /// Constructs schema from the database view.
    pub fn new(view: T) -> Self {
        CurrencySchema { view }
    }

    /// Returns `MerklePatriciaTable` with wallets.
    pub fn wallets_proof(&self) -> ProofMapIndex<&T, PublicKey, Wallet> {
        ProofMapIndex::new(vec![20], &self.view)
    }

    /// Returns state hash.
    pub fn state_hash(&self) -> Vec<Hash> {
        vec![self.wallets_proof().root_hash()]
    }
}

impl<'a> CurrencySchema<&'a mut Fork> {
    /// Returns `MerklePatriciaTable` with wallets.
    pub fn wallets(&self) -> MapIndex<&mut Fork, PublicKey, Wallet> {
        MapIndex::new(vec![20], self.view)
    }

    /// Returns wallet for the given public key.
    pub fn wallet(&self, pub_key: &PublicKey) -> Option<Wallet> {
        self.wallets().get(pub_key)
    }

    /// Returns history for the wallet by the given public key.
    pub fn wallet_history(&self,
                          public_key: &PublicKey)
                          -> ListIndex<&mut Fork, TxMetaRecord> {
        let mut prefix = vec![19; 1 + PUBLIC_KEY_LENGTH];
        prefix[1..].copy_from_slice(public_key.as_ref());
        MapIndex::new(prefix, self.view)
    }

    /// Adds transaction record to the walled by the given public key.
    fn append_history(&self,
                      mut wallet: Wallet,
                      key: &PublicKey,
                      meta: TxMetaRecord) {
        let history = self.wallet_history(key);
        history.push(meta);
        wallet.grow_length_set_history_hash(&history.root_hash());
        self.wallets().put(key, wallet)
    }
}

impl TxTransfer {
    /// Executes transfer transaction.
    pub fn execute(&self, view: &mut Fork, tx_hash: Hash) {
        let schema = CurrencySchema::new(view);
        let sender_pub_key = self.from();
        let receiver_pub_key = self.to();

        let mut sender_wallet = match schema.wallet(sender_pub_key) {
            Some(val) => val,
            None => {
                return;
            }
        };

        let meta = match schema.wallet(receiver_pub_key) {
            Some(mut receiver) => {
                let status = sender_wallet.transfer_to(&mut receiver, self.amount());
                let meta = TxMetaRecord::new(&tx_hash, status);
                if status {
                    let meta = meta.clone();
                    schema.append_history(receiver, receiver_pub_key, meta);
                }
                meta
            }
            None => TxMetaRecord::new(&tx_hash, false),
        };
        schema.append_history(sender_wallet, sender_pub_key, meta)
    }
}

impl TxIssue {
    /// Executes issue transaction.
    pub fn execute(&self, view: &mut Fork, tx_hash: Hash) {
        let schema = CurrencySchema::new(view);
        let pub_key = self.wallet();
        if let Some(mut wallet) = schema.wallet(pub_key) {
            let new_balance = wallet.balance() + self.amount();
            wallet.set_balance(new_balance);
            let meta = TxMetaRecord::new(&tx_hash, true);
            schema.append_history(wallet, pub_key, meta);
        }
    }
}

impl TxCreateWallet {
    /// Executes wallet creation transaction.
    pub fn execute(&self, view: &mut Fork, tx_hash: Hash) {
        let schema = CurrencySchema::new(view);
        let found_wallet = schema.wallet(self.pub_key());
        let execution_status = found_wallet.is_none();

        let meta = TxMetaRecord::new(&tx_hash, execution_status);
        let history = schema.wallet_history(self.pub_key());
        history.push(meta);

        let wallet = if let Some(mut wallet) = found_wallet {
            wallet.grow_length_set_history_hash(&history.root_hash());
            wallet
        } else {
            Wallet::new(self.pub_key(),
                        self.name(),
                        0,
                        1, // history_len
                        &history.root_hash())
        };
        schema.wallets().put(self.pub_key(), wallet)
    }
}

impl Transaction for CurrencyTx {
    fn info(&self) -> Value {
        to_value(self).unwrap()
    }

    fn verify(&self) -> bool {
        let res = self.verify_signature(self.pub_key());
        let res1 = match *self {
            CurrencyTx::Transfer(ref msg) => *msg.from() != *msg.to(),
            _ => true,
        };
        res && res1
    }

    fn execute(&self, view: &mut Fork) {
        let tx_hash = Message::hash(self);
        match *self {
            CurrencyTx::Transfer(ref msg) => msg.execute(view, tx_hash),
            CurrencyTx::Issue(ref msg) => msg.execute(view, tx_hash),
            CurrencyTx::CreateWallet(ref msg) => msg.execute(view, tx_hash),
        }
    }
}

/// Exonum `Service` implementation.
#[derive(Default, Debug)]
pub struct CurrencyService {}

impl CurrencyService {
    /// Creates `CurrencyService`.
    pub fn new() -> Self {
        CurrencyService {}
    }
}

impl Service for CurrencyService {
    fn service_name(&self) -> &'static str {
        "cryptocurrency"
    }

    fn service_id(&self) -> u16 {
        CRYPTOCURRENCY_SERVICE_ID
    }

    fn state_hash(&self, view: &Snapshot) -> Vec<Hash> {
        let schema = CurrencySchema::new(view);
        schema.state_hash()
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, StreamStructError> {
        CurrencyTx::from_raw(raw).map(|tx| Box::new(tx) as Box<Transaction>)
    }

    fn public_api_handler(&self, ctx: &ApiContext) -> Option<Box<Handler>> {
        let mut router = Router::new();
        use api;
        use exonum::api::Api;
        let api = api::CryptocurrencyApi {
            channel: ctx.node_channel().clone(),
            blockchain: ctx.blockchain().clone(),
        };
        api.wire(&mut router);
        Some(Box::new(router))
    }
}

#[cfg(test)]
mod tests {
    use byteorder::{ByteOrder, LittleEndian};
    use rand::{thread_rng, Rng};
    use tempdir::TempDir;

    use exonum::crypto::{gen_keypair, Hash, hash, PublicKey};
    use exonum::storage::{self, Storage};
    use exonum::blockchain::{Blockchain, Transaction};
    use exonum::messages::{FromRaw, Message};
    use exonum::encoding::serialize::json::reexport as serde_json;

    use super::{CurrencyTx, CurrencyService, CurrencySchema, TxCreateWallet, TxIssue, TxTransfer};
    use super::tx_metarecord::TxMetaRecord;
    use super::wallet::{Wallet, assert_wallet};

    #[test]
    fn test_tx_transfer_serde() {
        let mut rng = thread_rng();
        let generator = move |_| {
            let (p_from, s) = gen_keypair();
            let (p_to, _) = gen_keypair();
            let amount = rng.next_u64();
            let seed = rng.next_u64();
            TxTransfer::new(&p_from, &p_to, amount, seed, &s)
        };
        let create_txs = (0..50).map(generator).collect::<Vec<_>>();
        for tx in create_txs {
            let wrapped_tx = CurrencyTx::Transfer(tx);
            let json_str = serde_json::to_string(&wrapped_tx).unwrap();
            let parsed_json: CurrencyTx = serde_json::from_str(&json_str).unwrap();
            assert_eq!(wrapped_tx, parsed_json);
            trace!("tx issue test_data: {}",
                   serde_json::to_string(&TransactionTestData::new(wrapped_tx)).unwrap());
        }
    }

    #[test]
    fn test_tx_issue_serde() {
        let mut rng = thread_rng();
        let generator = move |_| {
            let (p, s) = gen_keypair();
            let amount = rng.next_u64();
            let seed = rng.next_u64();
            TxIssue::new(&p, amount, seed, &s)
        };
        let create_txs = (0..50).map(generator).collect::<Vec<_>>();
        for tx in create_txs {
            let wrapped_tx = CurrencyTx::Issue(tx);
            let json_str = serde_json::to_string(&wrapped_tx).unwrap();
            let parsed_json: CurrencyTx = serde_json::from_str(&json_str).unwrap();
            assert_eq!(wrapped_tx, parsed_json);
            trace!("tx issue test_data: {}",
                   serde_json::to_string(&TransactionTestData::new(wrapped_tx)).unwrap());
        }
    }

    #[test]
    fn test_tx_create_wallet_serde() {
        let mut rng = thread_rng();
        let generator = move |_| {
            let (p, s) = gen_keypair();
            let string_len = rng.gen_range(20u8, 255u8);
            let name: String = rng.gen_ascii_chars().take(string_len as usize).collect();
            TxCreateWallet::new(&p, &name, &s)
        };
        let (p, s) = gen_keypair();
        let non_ascii_create =
            TxCreateWallet::new(&p, "babd, Юникод еще работает", &s);
        let mut create_txs = (0..50).map(generator).collect::<Vec<_>>();
        create_txs.push(non_ascii_create);
        for tx in create_txs {
            let wrapped_tx = CurrencyTx::CreateWallet(tx);
            let json_str = serde_json::to_string(&wrapped_tx).unwrap();
            let parsed_json: CurrencyTx = serde_json::from_str(&json_str).unwrap();
            assert_eq!(wrapped_tx, parsed_json);
            trace!("tx issue test_data: {}",
                   serde_json::to_string(&TransactionTestData::new(wrapped_tx)).unwrap());
        }
    }

    #[test]
    fn generate_simple_scenario_transactions() {
        let mut rng = thread_rng();
        let (p1, s1) = gen_keypair();
        let (p2, s2) = gen_keypair();
        let tx_create_1 = TxCreateWallet::new(&p1, "Василий Васильевич", &s1);
        let tx_create_2 = TxCreateWallet::new(&p2, "Name", &s2);
        let tx_issue_1 = TxIssue::new(&p1, 6000, rng.next_u64(), &s1);
        let tx_transfer_1 = TxTransfer::new(&p1, &p2, 3000, rng.next_u64(), &s1);
        let tx_transfer_2 = TxTransfer::new(&p2, &p1, 1000, rng.next_u64(), &s2);
        let txs: Vec<CurrencyTx> = vec![tx_create_1.into(),
                                        tx_create_2.into(),
                                        tx_issue_1.into(),
                                        tx_transfer_1.into(),
                                        tx_transfer_2.into()];
        for (idx, tx) in txs.iter().enumerate() {
            trace!("transaction #{}: {}",
                   idx,
                   serde_json::to_string(tx).unwrap());
        }
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
        let expected_hash;
        {
            let slice = (&tx).raw().as_ref().as_ref();
            expected_hash = hash(slice);
        }
        assert_eq!(expected_hash, CurrencyTx::from(tx).hash())
    }

    #[test]
    fn test_wallet_prefix() {
        let id = 4096;
        let mut prefix = vec![10; 9];
        LittleEndian::write_u64(&mut prefix[1..], id);
        assert_eq!(prefix, vec![10, 0, 16, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn test_wallet_history_txtransfer_false_status_absent_receiver_wallet() {
        let db = create_db();
        let b = Blockchain::new(db, vec![Box::new(CurrencyService::new())]);
        let v = b.view();
        let s = CurrencySchema::new(&v);

        let (p1, s1) = gen_keypair();
        let (p2, _) = gen_keypair();

        let cw1 = TxCreateWallet::new(&p1, "name_wallet1", &s1);
        CurrencyTx::from(cw1.clone()).execute(&v).unwrap();

        let iw1 = TxIssue::new(&p1, 1000, 1, &s1);
        CurrencyTx::from(iw1.clone()).execute(&v).unwrap();

        let tw = TxTransfer::new(&p1, &p2, 300, 3, &s1);
        CurrencyTx::from(tw.clone()).execute(&v).unwrap();

        let (w1, rh1) = get_wallet_and_history(&s, &p1);
        let (w2, _) = get_wallet_and_history(&s, &p2);
        assert_wallet(&w1.unwrap(), &p1, "name_wallet1", 1000, 3, &rh1);
        assert_eq!(w2, None);
        let h1 = s.wallet_history(&p1).values().unwrap();
        let h2 = s.wallet_history(&p2).values().unwrap();
        let meta_create1 = TxMetaRecord::new(&cw1.hash(), true);
        let meta_issue1 = TxMetaRecord::new(&iw1.hash(), true);
        let meta_transfer = TxMetaRecord::new(&tw.hash(), false);
        assert_eq!(h1, vec![meta_create1, meta_issue1, meta_transfer.clone()]);
        assert_eq!(h2, vec![]);
    }

    #[test]
    fn test_wallet_history_txtransfer_false_status_insufficient_balance() {
        let db = create_db();
        let b = Blockchain::new(db, vec![Box::new(CurrencyService::new())]);
        let v = b.view();
        let s = CurrencySchema::new(&v);

        let (p1, s1) = gen_keypair();
        let (p2, s2) = gen_keypair();

        let cw1 = TxCreateWallet::new(&p1, "name_wallet1", &s1);
        let cw2 = TxCreateWallet::new(&p2, "name_wallet2", &s2);
        CurrencyTx::from(cw1.clone()).execute(&v).unwrap();
        CurrencyTx::from(cw2.clone()).execute(&v).unwrap();

        let iw1 = TxIssue::new(&p1, 1000, 1, &s1);
        CurrencyTx::from(iw1.clone()).execute(&v).unwrap();

        let tw = TxTransfer::new(&p1, &p2, 1018, 3, &s1);
        CurrencyTx::from(tw.clone()).execute(&v).unwrap();

        let (w1, rh1) = get_wallet_and_history(&s, &p1);
        let (w2, rh2) = get_wallet_and_history(&s, &p2);
        assert_wallet(&w1.unwrap(), &p1, "name_wallet1", 1000, 3, &rh1);
        assert_wallet(&w2.unwrap(), &p2, "name_wallet2", 0, 1, &rh2);
        let h1 = s.wallet_history(&p1).values().unwrap();
        let h2 = s.wallet_history(&p2).values().unwrap();
        let meta_create1 = TxMetaRecord::new(&cw1.hash(), true);
        let meta_create2 = TxMetaRecord::new(&cw2.hash(), true);
        let meta_issue1 = TxMetaRecord::new(&iw1.hash(), true);
        let meta_transfer = TxMetaRecord::new(&tw.hash(), false);
        assert_eq!(h1, vec![meta_create1, meta_issue1, meta_transfer.clone()]);
        assert_eq!(h2, vec![meta_create2]);
    }

    #[test]
    fn test_wallet_history_txcreate_false_status() {
        let db = create_db();
        let b = Blockchain::new(db, vec![Box::new(CurrencyService::new())]);
        let v = b.view();
        let s = CurrencySchema::new(&v);

        let (p1, s1) = gen_keypair();
        let cw1 = TxCreateWallet::new(&p1, "name_wallet1", &s1);
        let meta_create1 = TxMetaRecord::new(&cw1.hash(), true);
        let cw2 = TxCreateWallet::new(&p1, "name_wallet2", &s1);
        let meta_create2 = TxMetaRecord::new(&cw2.hash(), false);

        CurrencyTx::from(cw1.clone()).execute(&v).unwrap();
        CurrencyTx::from(cw2.clone()).execute(&v).unwrap();

        let (w, rh) = get_wallet_and_history(&s, &p1);
        assert_wallet(&w.unwrap(), &p1, "name_wallet1", 0, 2, &rh);
        let h1 = s.wallet_history(&p1).values().unwrap();
        assert_eq!(h1, vec![meta_create1, meta_create2]);
    }

    #[test]
    fn test_wallet_history_true_status() {
        let db = create_db();
        let b = Blockchain::new(db, vec![Box::new(CurrencyService::new())]);

        let v = b.view();
        let s = CurrencySchema::new(&v);

        let (p1, s1) = gen_keypair();
        let (p2, s2) = gen_keypair();

        let cw1 = TxCreateWallet::new(&p1, "name_wallet1", &s1);
        let cw2 = TxCreateWallet::new(&p2, "name_wallet2", &s2);
        CurrencyTx::from(cw1.clone()).execute(&v).unwrap();
        CurrencyTx::from(cw2.clone()).execute(&v).unwrap();

        let (w1, rh1) = get_wallet_and_history(&s, &p1);
        let (w2, rh2) = get_wallet_and_history(&s, &p2);
        assert_wallet(&w1.unwrap(), &p1, "name_wallet1", 0, 1, &rh1);
        assert_wallet(&w2.unwrap(), &p2, "name_wallet2", 0, 1, &rh2);

        let iw1 = TxIssue::new(&p1, 1000, 1, &s1);
        let iw2 = TxIssue::new(&p2, 100, 2, &s2);
        CurrencyTx::from(iw1.clone()).execute(&v).unwrap();
        CurrencyTx::from(iw2.clone()).execute(&v).unwrap();

        let (w1, rh1) = get_wallet_and_history(&s, &p1);
        let (w2, rh2) = get_wallet_and_history(&s, &p2);
        assert_wallet(&w1.unwrap(), &p1, "name_wallet1", 1000, 2, &rh1);
        assert_wallet(&w2.unwrap(), &p2, "name_wallet2", 100, 2, &rh2);

        let tw = TxTransfer::new(&p1, &p2, 400, 3, &s1);
        CurrencyTx::from(tw.clone()).execute(&v).unwrap();

        let (w1, rh1) = get_wallet_and_history(&s, &p1);
        let (w2, rh2) = get_wallet_and_history(&s, &p2);
        assert_wallet(&w1.unwrap(), &p1, "name_wallet1", 600, 3, &rh1);
        assert_wallet(&w2.unwrap(), &p2, "name_wallet2", 500, 3, &rh2);

        let h1 = s.wallet_history(&p1).values().unwrap();
        let h2 = s.wallet_history(&p2).values().unwrap();
        let meta_create1 = TxMetaRecord::new(&cw1.hash(), true);
        let meta_create2 = TxMetaRecord::new(&cw2.hash(), true);
        let meta_issue1 = TxMetaRecord::new(&iw1.hash(), true);
        let meta_issue2 = TxMetaRecord::new(&iw2.hash(), true);
        let meta_transfer = TxMetaRecord::new(&tw.hash(), true);
        assert_eq!(h1, vec![meta_create1, meta_issue1, meta_transfer.clone()]);
        assert_eq!(h2, vec![meta_create2, meta_issue2, meta_transfer]);
    }

    #[cfg(feature="memorydb")]
    fn create_db() -> Storage {
        storage::MemoryDB::new()
    }

    #[cfg(not(feature="memorydb"))]
    fn create_db() -> Storage {
        let mut options = storage::LevelDBOptions::new();
        options.create_if_missing = true;
        let dir = TempDir::new("cryptocurrency").unwrap();
        storage::LevelDB::new(dir.path(), options).unwrap()
    }

    #[derive(Serialize)]
    struct TransactionTestData {
        transaction: CurrencyTx,
        hash: Hash,
        raw: Vec<u8>,
    }

    impl TransactionTestData {
        fn new(transaction: CurrencyTx) -> TransactionTestData {
            let hash = transaction.hash();
            let raw = transaction.raw().as_ref().as_ref().to_vec();
            TransactionTestData {
                transaction: transaction,
                hash: hash,
                raw: raw,
            }
        }
    }

    fn get_wallet_and_history(schema: &CurrencySchema,
                              pub_key: &PublicKey)
                              -> (Option<Wallet>, Hash) {
        let w = schema.wallet(pub_key).unwrap();
        let h = schema.wallet_history(pub_key).root_hash().unwrap();
        (w, h)
    }
}
