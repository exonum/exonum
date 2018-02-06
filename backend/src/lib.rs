//! Cryptocurrency implementation example using [exonum](http://exonum.com/).

// TODO: Uncomment when `encoding_struct!` and `message!` implementation will be updated.
// #![deny(missing_docs)]
#![deny(missing_debug_implementations)]

extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate log;
#[macro_use]
extern crate exonum;
extern crate params;
extern crate router;
extern crate iron;
extern crate bodyparser;
extern crate percent_encoding;
#[cfg(test)]
extern crate rand;
#[cfg(test)]
extern crate byteorder;
#[cfg(test)]
extern crate exonum_testkit;

use iron::Handler;
use router::Router;

use std::fmt;
use std::error::Error;

use exonum::messages::{RawMessage, RawTransaction, Message};
use exonum::crypto::{PublicKey, Hash};
use exonum::storage::{Snapshot, Fork, MapIndex, ProofListIndex, ProofMapIndex};
use exonum::blockchain::{Service, Transaction, ApiContext, gen_prefix};
use exonum::encoding::serialize::json::reexport as serde_json;
use exonum::encoding::{Offset, Field, Error as StreamStructError};
use exonum::helpers::fabric::{ServiceFactory, Context};
use serde_json::Value;
use exonum::encoding::serialize::json::ExonumJson;
use exonum::encoding::serialize::{WriteBufferWrapper, FromHex, ToHex};

use wallet::{Wallet, WalletAccess};
use tx_metarecord::TxMetaRecord;

mod tx_metarecord;

pub mod api;
pub mod wallet;

#[derive(Clone)]
pub struct KeyBox(pub [u8; 128]);

impl fmt::Debug for KeyBox {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "KeyBox {:?}...", &self.0[0..10])
    }
}

implement_pod_as_ref_field! { KeyBox }

impl<'a> ExonumJson for &'a KeyBox {
    fn deserialize_field<B: WriteBufferWrapper>(
        value: &Value,
        buffer: &mut B,
        from: Offset,
        to: Offset,
    ) -> Result<(), Box<Error>> {
        let v = value.as_str().ok_or("Can't cast json as string")?;
        let bytes = Vec::<u8>::from_hex(v)?;
        if bytes.len() != 128 {
            return Err("wrong length".into());
        }
        let mut array: [u8; 128] = [0; 128];
        for (element, value) in array.iter_mut().zip(bytes.iter()) {
            *element = *value;
        }
        buffer.write(from, to, &KeyBox(array));
        Ok(())
    }

    fn serialize_field(&self) -> Result<Value, Box<Error + Send + Sync>> {
        let mut s = String::new();
        self.0.as_ref().write_hex(&mut s)?;
        Ok(Value::String(s))
    }
}

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

        from:        &PublicKey,
        to:          &PublicKey,
        amount:      u64,
        seed:        u64,
    }
}

message! {
/// Issue `amount` of the currency to the `wallet`.
    struct TxIssue {
        const TYPE = CRYPTOCURRENCY_SERVICE_ID;
        const ID = TX_ISSUE_ID;

        wallet:      &PublicKey,
        amount:      u64,
        seed:        u64,
    }
}

message! {
/// Create wallet with the given `login`.
    struct TxCreateWallet {
        const TYPE = CRYPTOCURRENCY_SERVICE_ID;
        const ID = TX_WALLET_ID;

        pub_key:     &PublicKey,
        login:       &str,
        key_box:     &KeyBox,
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
    /// Create wallet with given login.
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

    fn from_raw(raw: RawMessage) -> Result<Self, StreamStructError> {
        match raw.message_type() {
            TX_TRANSFER_ID => Ok(CurrencyTx::Transfer(TxTransfer::from_raw(raw)?)),
            TX_ISSUE_ID => Ok(CurrencyTx::Issue(TxIssue::from_raw(raw)?)),
            TX_WALLET_ID => Ok(CurrencyTx::CreateWallet(TxCreateWallet::from_raw(raw)?)),
            _ => Err(StreamStructError::IncorrectMessageType {
                message_type: raw.message_type(),
            }),
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

impl<T> AsMut<T> for CurrencySchema<T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.view
    }
}

impl<T> fmt::Debug for CurrencySchema<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "CurrencySchema {{}}")
    }
}

impl<T> CurrencySchema<T>
where
    T: AsRef<Snapshot>,
{
    /// Constructs schema from the database view.
    pub fn new(view: T) -> Self {
        CurrencySchema { view }
    }

    /// Returns `MerklePatriciaTable` with wallets.
    pub fn wallets_proof(&self) -> ProofMapIndex<&T, PublicKey, Wallet> {
        ProofMapIndex::new("cryptocurrency.wallets", &self.view)
    }

    /// Returns state hash.
    pub fn state_hash(&self) -> Vec<Hash> {
        vec![self.wallets_proof().root_hash()]
    }
}

impl<'a> CurrencySchema<&'a mut Fork> {
    /// Returns `MerklePatriciaTable` with wallets.
    pub fn wallets(&mut self) -> ProofMapIndex<&mut Fork, PublicKey, Wallet> {
        ProofMapIndex::new("cryptocurrency.wallets", self.view)
    }

    /// Returns wallet for the given public key.
    pub fn wallet(&mut self, pub_key: &PublicKey) -> Option<Wallet> {
        self.wallets().get(pub_key)
    }

    /// Returns Map with keyboxes.
    pub fn key_boxes(&mut self) -> MapIndex<&mut Fork, String, WalletAccess> {
        MapIndex::new("cryptocurrency.keys_boxes", self.view)
    }

    /// Returns history for the wallet by the given public key.
    pub fn wallet_history(
        &mut self,
        public_key: &PublicKey,
    ) -> ProofListIndex<&mut Fork, TxMetaRecord> {
        ProofListIndex::with_prefix(
            "cryptocurrency.wallet_history",
            gen_prefix(public_key),
            self.view,
        )
    }

    /// Adds transaction record to the walled by the given public key.
    fn append_history(&mut self, mut wallet: Wallet, key: &PublicKey, meta: TxMetaRecord) {
        {
            let mut history = self.wallet_history(key);
            history.push(meta);
            wallet.grow_length_set_history_hash(&history.root_hash());
        }
        self.wallets().put(key, wallet)
    }

    #[cfg(test)]
    fn collect_history(&mut self, key: &PublicKey) -> Vec<TxMetaRecord> {
        let history = self.wallet_history(key);
        let history = history.into_iter();
        history.collect()
    }
}

impl TxTransfer {
    /// Executes transfer transaction.
    pub fn execute(&self, view: &mut Fork, tx_hash: Hash) {
        let mut schema = CurrencySchema::new(view);
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
        let mut schema = CurrencySchema::new(view);
        let pub_key = self.wallet();
        if let Some(mut wallet) = schema.wallet(pub_key) {
            let new_balance = if cfg!(feature = "byzantine-behavior") {
                wallet.balance() + self.amount() + 5
            } else {
                wallet.balance() + self.amount()
            };
            wallet.set_balance(new_balance);
            let meta = TxMetaRecord::new(&tx_hash, true);
            schema.append_history(wallet, pub_key, meta);
        }
    }
}

impl TxCreateWallet {
    /// Executes wallet creation transaction.
    pub fn execute(&self, view: &mut Fork, tx_hash: Hash) {
        let mut schema = CurrencySchema::new(view);
        let wallet = {
            let found_wallet = schema.wallet(self.pub_key());
            let execution_status = found_wallet.is_none();

            let root_hash = {
                let meta = TxMetaRecord::new(&tx_hash, execution_status);
                let mut history = schema.wallet_history(self.pub_key());
                history.push(meta);
                history.root_hash()
            };

            if let Some(mut wallet) = found_wallet {
                wallet.grow_length_set_history_hash(&root_hash);
                wallet
            } else {
                let login = self.login().to_owned();
                let access = WalletAccess::new(self.pub_key(), self.key_box());
                schema.key_boxes().put(&login, access);
                Wallet::new(
                    self.pub_key(),
                    self.login(),
                    0,
                    1, // history_len
                    &root_hash,
                )
            }
        };
        schema.wallets().put(self.pub_key(), wallet)
    }
}

impl ExonumJson for CurrencyTx {
    fn deserialize_field<B: WriteBufferWrapper>(
        value: &Value,
        buffer: &mut B,
        from: Offset,
        to: Offset,
    ) -> Result<(), Box<Error>>
    where
        Self: Sized,
    {
        let tx = serde_json::from_value(value.clone())?;
        match tx {
            CurrencyTx::Transfer(ref t) => buffer.write(from, to, t.clone()),
            CurrencyTx::Issue(ref t) => buffer.write(from, to, t.clone()),
            CurrencyTx::CreateWallet(ref t) => buffer.write(from, to, t.clone()),
        }
        Ok(())
    }

    fn serialize_field(&self) -> Result<Value, Box<Error + Send + Sync>> {
        Ok(serde_json::to_value(self).unwrap())
    }
}

impl Transaction for CurrencyTx {
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

impl ServiceFactory for CurrencyService {
    fn make_service(&mut self, _: &Context) -> Box<Service> {
        Box::new(CurrencyService::new())
    }
}

#[cfg(test)]
mod tests {
    use byteorder::{ByteOrder, LittleEndian};
    use rand::{thread_rng, Rng};

    use exonum::crypto::{gen_keypair, Hash, hash, PublicKey};
    use exonum::blockchain::Transaction;
    use exonum::storage::Fork;
    use exonum::messages::Message;
    use exonum::encoding::serialize::json::reexport as serde_json;

    use exonum_testkit::TestKitBuilder;

    use super::{CurrencyTx, CurrencyService, CurrencySchema, TxCreateWallet, TxIssue, TxTransfer,
                KeyBox};
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
            trace!(
                "tx issue test_data: {}",
                serde_json::to_string(&TransactionTestData::new(wrapped_tx)).unwrap()
            );
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
            trace!(
                "tx issue test_data: {}",
                serde_json::to_string(&TransactionTestData::new(wrapped_tx)).unwrap()
            );
        }
    }

    #[test]
    fn test_tx_create_wallet_serde() {
        let mut rng = thread_rng();
        let generator = move |_| {
            let (p, s) = gen_keypair();
            let string_len = rng.gen_range(20u8, 255u8);
            let login: String = rng.gen_ascii_chars().take(string_len as usize).collect();
            TxCreateWallet::new(&p, &login, &KeyBox([0; 128]), &s)
        };
        let (p, s) = gen_keypair();
        let non_ascii_create = TxCreateWallet::new(
            &p,
            "babd, Юникод еще работает",
            &KeyBox([0; 128]),
            &s,
        );
        let mut create_txs = (0..50).map(generator).collect::<Vec<_>>();
        create_txs.push(non_ascii_create);
        for tx in create_txs {
            let wrapped_tx = CurrencyTx::CreateWallet(tx);
            let json_str = serde_json::to_string(&wrapped_tx).unwrap();
            let parsed_json: CurrencyTx = serde_json::from_str(&json_str).unwrap();
            assert_eq!(wrapped_tx, parsed_json);
            trace!(
                "tx issue test_data: {}",
                serde_json::to_string(&TransactionTestData::new(wrapped_tx)).unwrap()
            );
        }
    }

    #[test]
    fn generate_simple_scenario_transactions() {
        let mut rng = thread_rng();
        let (p1, s1) = gen_keypair();
        let (p2, s2) = gen_keypair();
        let tx_create_1 = TxCreateWallet::new(
            &p1,
            "Василий Васильевич",
            &KeyBox([0; 128]),
            &s1,
        );
        let tx_create_2 = TxCreateWallet::new(&p2, "Name", &KeyBox([0; 128]), &s2);
        let tx_issue_1 = TxIssue::new(&p1, 6000, rng.next_u64(), &s1);
        let tx_transfer_1 = TxTransfer::new(&p1, &p2, 3000, rng.next_u64(), &s1);
        let tx_transfer_2 = TxTransfer::new(&p2, &p1, 1000, rng.next_u64(), &s2);
        let txs: Vec<CurrencyTx> = vec![
            tx_create_1.into(),
            tx_create_2.into(),
            tx_issue_1.into(),
            tx_transfer_1.into(),
            tx_transfer_2.into(),
        ];
        for (idx, tx) in txs.iter().enumerate() {
            trace!(
                "transaction #{}: {}",
                idx,
                serde_json::to_string(tx).unwrap()
            );
        }
    }

    #[test]
    fn test_tx_create_wallet() {
        let (p, s) = gen_keypair();
        let n = "babd, Юникод еще работает";

        let tx = TxCreateWallet::new(&p, n, &KeyBox([0; 128]), &s);
        assert_eq!(tx.pub_key(), &p);
        assert_eq!(tx.login(), n);

        let tx2 = TxCreateWallet::from_raw(tx.raw().clone()).unwrap();
        assert_eq!(tx2.pub_key(), &p);
        assert_eq!(tx2.login(), n);
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
        let mut testkit = TestKitBuilder::validator()
            .with_service(CurrencyService::new())
            .create();
        let mut fork = testkit.blockchain_mut().fork();
        let mut schema = CurrencySchema::new(&mut fork);

        let (p1, s1) = gen_keypair();
        let (p2, _) = gen_keypair();

        let cw1 = TxCreateWallet::new(&p1, "login_wallet1", &KeyBox([0; 128]), &s1);
        CurrencyTx::from(cw1.clone()).execute(schema.as_mut());

        let iw1 = TxIssue::new(&p1, 1000, 1, &s1);
        CurrencyTx::from(iw1.clone()).execute(schema.as_mut());

        let tw = TxTransfer::new(&p1, &p2, 300, 3, &s1);
        CurrencyTx::from(tw.clone()).execute(schema.as_mut());

        let (w1, rh1) = get_wallet_and_history(&mut schema, &p1);
        let (w2, _) = get_wallet_and_history(&mut schema, &p2);
        assert_wallet(&w1.unwrap(), &p1, "login_wallet1", 1000, 3, &rh1);
        assert_eq!(w2, None);
        let h1 = schema.collect_history(&p1);
        let h2 = schema.collect_history(&p2);
        let meta_create1 = TxMetaRecord::new(&cw1.hash(), true);
        let meta_issue1 = TxMetaRecord::new(&iw1.hash(), true);
        let meta_transfer = TxMetaRecord::new(&tw.hash(), false);
        assert_eq!(h1, vec![meta_create1, meta_issue1, meta_transfer.clone()]);
        assert_eq!(h2, vec![]);
    }

    #[test]
    fn test_wallet_history_txtransfer_false_status_insufficient_balance() {
        let mut testkit = TestKitBuilder::validator()
            .with_service(CurrencyService::new())
            .create();
        let mut fork = testkit.blockchain_mut().fork();
        let mut schema = CurrencySchema::new(&mut fork);

        let (p1, s1) = gen_keypair();
        let (p2, s2) = gen_keypair();

        let cw1 = TxCreateWallet::new(&p1, "login_wallet1", &KeyBox([0; 128]), &s1);
        let cw2 = TxCreateWallet::new(&p2, "login_wallet2", &KeyBox([0; 128]), &s2);
        CurrencyTx::from(cw1.clone()).execute(schema.as_mut());
        CurrencyTx::from(cw2.clone()).execute(schema.as_mut());

        let iw1 = TxIssue::new(&p1, 1000, 1, &s1);
        CurrencyTx::from(iw1.clone()).execute(schema.as_mut());

        let tw = TxTransfer::new(&p1, &p2, 1018, 3, &s1);
        CurrencyTx::from(tw.clone()).execute(schema.as_mut());

        let (w1, rh1) = get_wallet_and_history(&mut schema, &p1);
        let (w2, rh2) = get_wallet_and_history(&mut schema, &p2);
        assert_wallet(&w1.unwrap(), &p1, "login_wallet1", 1000, 3, &rh1);
        assert_wallet(&w2.unwrap(), &p2, "login_wallet2", 0, 1, &rh2);
        let h1 = schema.collect_history(&p1);
        let h2 = schema.collect_history(&p2);
        let meta_create1 = TxMetaRecord::new(&cw1.hash(), true);
        let meta_create2 = TxMetaRecord::new(&cw2.hash(), true);
        let meta_issue1 = TxMetaRecord::new(&iw1.hash(), true);
        let meta_transfer = TxMetaRecord::new(&tw.hash(), false);
        assert_eq!(h1, vec![meta_create1, meta_issue1, meta_transfer.clone()]);
        assert_eq!(h2, vec![meta_create2]);
    }

    #[test]
    fn test_wallet_history_txcreate_false_status() {
        let mut testkit = TestKitBuilder::validator()
            .with_service(CurrencyService::new())
            .create();
        let mut fork = testkit.blockchain_mut().fork();
        let mut schema = CurrencySchema::new(&mut fork);

        let (p1, s1) = gen_keypair();
        let cw1 = TxCreateWallet::new(&p1, "login_wallet1", &KeyBox([0; 128]), &s1);
        let meta_create1 = TxMetaRecord::new(&cw1.hash(), true);
        let cw2 = TxCreateWallet::new(&p1, "login_wallet2", &KeyBox([0; 128]), &s1);
        let meta_create2 = TxMetaRecord::new(&cw2.hash(), false);

        CurrencyTx::from(cw1.clone()).execute(schema.as_mut());
        CurrencyTx::from(cw2.clone()).execute(schema.as_mut());

        let (w, rh) = get_wallet_and_history(&mut schema, &p1);
        assert_wallet(&w.unwrap(), &p1, "login_wallet1", 0, 2, &rh);
        let h1 = schema.collect_history(&p1);
        assert_eq!(h1, vec![meta_create1, meta_create2]);
    }

    #[test]
    fn test_wallet_history_true_status() {
        let mut testkit = TestKitBuilder::validator()
            .with_service(CurrencyService::new())
            .create();
        let mut fork = testkit.blockchain_mut().fork();
        let mut schema = CurrencySchema::new(&mut fork);

        let (p1, s1) = gen_keypair();
        let (p2, s2) = gen_keypair();

        let cw1 = TxCreateWallet::new(&p1, "login_wallet1", &KeyBox([0; 128]), &s1);
        let cw2 = TxCreateWallet::new(&p2, "login_wallet2", &KeyBox([0; 128]), &s2);
        CurrencyTx::from(cw1.clone()).execute(schema.as_mut());
        CurrencyTx::from(cw2.clone()).execute(schema.as_mut());

        let (w1, rh1) = get_wallet_and_history(&mut schema, &p1);
        let (w2, rh2) = get_wallet_and_history(&mut schema, &p2);
        assert_wallet(&w1.unwrap(), &p1, "login_wallet1", 0, 1, &rh1);
        assert_wallet(&w2.unwrap(), &p2, "login_wallet2", 0, 1, &rh2);

        let iw1 = TxIssue::new(&p1, 1000, 1, &s1);
        let iw2 = TxIssue::new(&p2, 100, 2, &s2);
        CurrencyTx::from(iw1.clone()).execute(schema.as_mut());
        CurrencyTx::from(iw2.clone()).execute(schema.as_mut());

        let (w1, rh1) = get_wallet_and_history(&mut schema, &p1);
        let (w2, rh2) = get_wallet_and_history(&mut schema, &p2);
        assert_wallet(&w1.unwrap(), &p1, "login_wallet1", 1000, 2, &rh1);
        assert_wallet(&w2.unwrap(), &p2, "login_wallet2", 100, 2, &rh2);

        let tw = TxTransfer::new(&p1, &p2, 400, 3, &s1);
        CurrencyTx::from(tw.clone()).execute(schema.as_mut());

        let (w1, rh1) = get_wallet_and_history(&mut schema, &p1);
        let (w2, rh2) = get_wallet_and_history(&mut schema, &p2);
        assert_wallet(&w1.unwrap(), &p1, "login_wallet1", 600, 3, &rh1);
        assert_wallet(&w2.unwrap(), &p2, "login_wallet2", 500, 3, &rh2);

        let h1 = schema.collect_history(&p1);
        let h2 = schema.collect_history(&p2);
        let meta_create1 = TxMetaRecord::new(&cw1.hash(), true);
        let meta_create2 = TxMetaRecord::new(&cw2.hash(), true);
        let meta_issue1 = TxMetaRecord::new(&iw1.hash(), true);
        let meta_issue2 = TxMetaRecord::new(&iw2.hash(), true);
        let meta_transfer = TxMetaRecord::new(&tw.hash(), true);
        assert_eq!(h1, vec![meta_create1, meta_issue1, meta_transfer.clone()]);
        assert_eq!(h2, vec![meta_create2, meta_issue2, meta_transfer]);
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
                transaction,
                hash,
                raw,
            }
        }
    }

    fn get_wallet_and_history(
        schema: &mut CurrencySchema<&mut Fork>,
        pub_key: &PublicKey,
    ) -> (Option<Wallet>, Hash) {
        let w = schema.wallet(pub_key);
        let h = schema.wallet_history(pub_key).root_hash();
        (w, h)
    }
}
