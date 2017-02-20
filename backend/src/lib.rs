extern crate rand;
extern crate time;
extern crate serde;
extern crate cookie;
#[macro_use]
extern crate serde_derive;
extern crate byteorder;
extern crate jsonway;
#[macro_use]
extern crate log;
#[cfg(test)]
extern crate tempdir;
extern crate serde_json;

#[macro_use(message, storage_value)]
extern crate exonum;
extern crate blockchain_explorer;
extern crate router;
extern crate iron;
extern crate hyper;
extern crate bodyparser;
extern crate configuration_service;

use serde::{Serialize, Serializer};
use serde::de::{self, Deserialize, Deserializer};
use exonum::messages::utils::U64;
use exonum::crypto::{PUBLIC_KEY_LENGTH, Signature};
use blockchain_explorer::TransactionInfo;
use serde_json::value::ToJson;
use serde_json::from_value;

pub mod api;
pub mod wallet;

use exonum::messages::{RawMessage, RawTransaction, FromRaw, Message, Error as MessageError};
use exonum::crypto::{PublicKey, Hash};
use exonum::storage::{Map, Error, MerklePatriciaTable, MapTable, MerkleTable, List, View,
                      Result as StorageResult};
use exonum::blockchain::{Service, Transaction};

use wallet::Wallet;

pub const CRYPTOCURRENCY: u16 = 128;

pub const TX_TRANSFER_ID: u16 = 128;
pub const TX_ISSUE_ID: u16 = 129;
pub const TX_WALLET_ID: u16 = 130;

use exonum::node::{TxSender, NodeChannel};
pub type CurrencyTxSender = TxSender<NodeChannel>;

message! {
    TxTransfer {
        const TYPE = CRYPTOCURRENCY;
        const ID = TX_TRANSFER_ID;
        const SIZE = 80;

        from:        &PublicKey  [00 => 32]
        to:          &PublicKey  [32 => 64]
        amount:      u64         [64 => 72]
        seed:        u64         [72 => 80]
    }
}

message! {
    TxIssue {
        const TYPE = CRYPTOCURRENCY;
        const ID = TX_ISSUE_ID;
        const SIZE = 48;

        wallet:      &PublicKey  [00 => 32]
        amount:      u64         [32 => 40]
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

#[derive(Serialize, Deserialize)]
struct TxSerdeHelper {
    id: u16,
    body: serde_json::Value,
    signature: Signature,
}

#[derive(Serialize, Deserialize)]
struct TxIssueSerdeHelper {
    wallet: PublicKey,
    amount: U64,
    seed: U64,
}

#[derive(Serialize, Deserialize)]
struct TxCreateSerdeHelper {
    pub_key: PublicKey,
    name: String,
}

#[derive(Serialize, Deserialize)]
struct TxTransferSerdeHelper {
    from: PublicKey,
    to: PublicKey,
    amount: U64,
    seed: U64,
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

impl Serialize for CurrencyTx {
    fn serialize<S>(&self, ser: &mut S) -> Result<(), S::Error>
        where S: Serializer
    {
        let id: u16;
        let signature = *self.raw().signature();
        let body;
        match *self {
            CurrencyTx::Issue(ref issue) => {
                id = TX_ISSUE_ID;
                let issue_body = TxIssueSerdeHelper {
                    wallet: *issue.wallet(),
                    amount: U64(issue.amount()),
                    seed: U64(issue.seed()),
                };
                body = issue_body.to_json();
            }
            CurrencyTx::Transfer(ref transfer) => {
                id = TX_TRANSFER_ID;
                let transfer_body = TxTransferSerdeHelper {
                    from: *transfer.from(),
                    to: *transfer.to(),
                    amount: U64(transfer.amount()),
                    seed: U64(transfer.seed()),
                };
                body = transfer_body.to_json();
            }
            CurrencyTx::CreateWallet(ref wallet) => {
                id = TX_WALLET_ID;
                let create_body = TxCreateSerdeHelper {
                    pub_key: *wallet.pub_key(),
                    name: wallet.name().to_string(),
                };
                body = create_body.to_json();
            }
        }
        let h = TxSerdeHelper {
            id: id,
            body: body,
            signature: signature,
        };
        h.serialize(ser)
    }
}

impl Deserialize for CurrencyTx {
    fn deserialize<D>(deserializer: &mut D) -> Result<Self, D::Error>
        where D: Deserializer
    {
        let h = <TxSerdeHelper>::deserialize(deserializer)?;
        let res = match h.id {
            TX_ISSUE_ID => {
                let body_type = "Tx_ISSUE";
                let body = from_value::<TxIssueSerdeHelper>(h.body).map_err(|e| {
                        de::Error::custom(format!("Coudn't parse '{}' transaction body from \
                                                   json. serde_json::error: {}",
                                                  body_type,
                                                  e))
                    })?;
                let tx = TxIssue::new_with_signature(&body.wallet,
                                                     body.amount.0,
                                                     body.seed.0,
                                                     &h.signature);
                CurrencyTx::Issue(tx)
            }
            TX_WALLET_ID => {
                let body_type = "Tx_CREATE";
                let body = from_value::<TxCreateSerdeHelper>(h.body).map_err(|e| {
                        de::Error::custom(format!("Coudn't parse '{}' transaction body from \
                                                   json. serde_json::error: {}",
                                                  body_type,
                                                  e))
                    })?;
                let tx =
                    TxCreateWallet::new_with_signature(&body.pub_key, &body.name, &h.signature);
                CurrencyTx::CreateWallet(tx)
            }
            TX_TRANSFER_ID => {
                let body_type = "Tx_TRANSFER";
                let body = from_value::<TxTransferSerdeHelper>(h.body).map_err(|e| {
                        de::Error::custom(format!("Coudn't parse '{}' transaction body from \
                                                   json. serde_json::error: {}",
                                                  body_type,
                                                  e))
                    })?;
                let tx = TxTransfer::new_with_signature(&body.from,
                                                        &body.to,
                                                        body.amount.0,
                                                        body.seed.0,
                                                        &h.signature);
                CurrencyTx::Transfer(tx)
            }
            other => {
                return Err(de::Error::custom(format!("Unknown transaction id for \
                                                      Cryptocurrency Service: {}",
                                                     other)));
            }
        };
        Ok(res)
    }
}

impl TransactionInfo for CurrencyTx {}

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
    view: &'a View,
}

impl<'a> CurrencySchema<'a> {
    pub fn new(view: &'a View) -> CurrencySchema {
        CurrencySchema { view: view }
    }

    pub fn wallets(&self) -> MerklePatriciaTable<MapTable<View, [u8], Vec<u8>>, PublicKey, Wallet> {
        MerklePatriciaTable::new(MapTable::new(vec![20], self.view))
    }

    pub fn wallet(&self, pub_key: &PublicKey) -> StorageResult<Option<Wallet>> {
        self.wallets().get(pub_key)
    }

    pub fn wallet_history(&self,
                          public_key: &PublicKey)
                          -> MerkleTable<MapTable<View, [u8], Vec<u8>>, u64, Hash> {
        let mut prefix = vec![19; 1 + PUBLIC_KEY_LENGTH];
        prefix[1..].copy_from_slice(public_key.as_ref());
        MerkleTable::new(MapTable::new(prefix, self.view))
    }

    pub fn state_hash(&self) -> StorageResult<Vec<Hash>> {
        Ok(vec![self.wallets().root_hash()?])
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

    fn execute(&self, view: &View) -> Result<(), Error> {
        let tx_hash = Message::hash(self);

        let schema = CurrencySchema::new(view);
        match *self {
            CurrencyTx::Transfer(ref msg) => {
                let sender_pub_key = msg.from();
                let receiver_pub_key = msg.to();

                let sender_w = schema.wallet(sender_pub_key)?;
                let receiver_w = schema.wallet(receiver_pub_key)?;
                if let (Some(mut sender), Some(mut receiver)) = (sender_w, receiver_w) {
                    if sender.balance() < msg.amount() {
                        return Ok(());
                    }
                    let sender_history = schema.wallet_history(sender_pub_key);
                    let receiver_history = schema.wallet_history(receiver_pub_key);
                    sender_history.append(tx_hash)?;
                    receiver_history.append(tx_hash)?;

                    sender.transfer_to(&mut receiver, msg.amount());
                    sender.set_history_hash(&sender_history.root_hash()?);
                    sender.increase_history_len();
                    receiver.set_history_hash(&receiver_history.root_hash()?);
                    receiver.increase_history_len();

                    schema.wallets().put(sender_pub_key, sender)?;
                    schema.wallets().put(receiver_pub_key, receiver)?;
                }
            }
            CurrencyTx::Issue(ref msg) => {
                let pub_key = msg.wallet();
                if let Some(mut wallet) = schema.wallet(pub_key)? {
                    let history = schema.wallet_history(pub_key);
                    history.append(tx_hash)?;

                    let new_amount = wallet.balance() + msg.amount();
                    wallet.set_balance(new_amount);
                    wallet.set_history_hash(&history.root_hash()?);
                    wallet.increase_history_len();
                    schema.wallets().put(pub_key, wallet)?;
                }
            }
            CurrencyTx::CreateWallet(ref msg) => {
                let pub_key = msg.pub_key();
                if let Some(_) = schema.wallet(pub_key)? {
                    return Ok(());
                }

                let history = schema.wallet_history(pub_key);
                history.append(tx_hash)?;

                let wallet = Wallet::new(msg.pub_key(),
                                         msg.name(),
                                         0,
                                         1, // history_len
                                         &history.root_hash()?);
                schema.wallets().put(pub_key, wallet)?;
            }
        };
        Ok(())
    }
}

impl Service for CurrencyService {
    fn service_id(&self) -> u16 {
        CRYPTOCURRENCY
    }

    fn state_hash(&self, view: &View) -> StorageResult<Vec<Hash>> {
        let schema = CurrencySchema::new(view);
        schema.state_hash()
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, MessageError> {
        CurrencyTx::from_raw(raw).map(|tx| Box::new(tx) as Box<Transaction>)
    }
}

#[cfg(test)]
mod tests {
    use byteorder::{ByteOrder, LittleEndian};
    use rand::{thread_rng, Rng};
    use serde_json;

    use exonum::crypto::{gen_keypair, Hash};
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
        let create_txs = (0..50)
            .map(generator)
            .collect::<Vec<_>>();
        for tx in create_txs {
            let wrapped_tx = CurrencyTx::Transfer(tx);
            let json_str = serde_json::to_string(&wrapped_tx).unwrap();
            let parsed_json: CurrencyTx = serde_json::from_str(&json_str).unwrap();
            assert_eq!(wrapped_tx, parsed_json);
            println!("tx issue test_data: {}",
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
        let create_txs = (0..50)
            .map(generator)
            .collect::<Vec<_>>();
        for tx in create_txs {
            let wrapped_tx = CurrencyTx::Issue(tx);
            let json_str = serde_json::to_string(&wrapped_tx).unwrap();
            let parsed_json: CurrencyTx = serde_json::from_str(&json_str).unwrap();
            assert_eq!(wrapped_tx, parsed_json);
            println!("tx issue test_data: {}",
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
        let mut create_txs = (0..50)
            .map(generator)
            .collect::<Vec<_>>();
        create_txs.push(non_ascii_create);
        for tx in create_txs {
            let wrapped_tx = CurrencyTx::CreateWallet(tx);
            let json_str = serde_json::to_string(&wrapped_tx).unwrap();
            let parsed_json: CurrencyTx = serde_json::from_str(&json_str).unwrap();
            assert_eq!(wrapped_tx, parsed_json);
            println!("tx issue test_data: {}",
                     serde_json::to_string(&TransactionTestData::new(wrapped_tx)).unwrap());
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

        assert_eq!(w1.name(), "tx1");
        assert_eq!(w1.history_len(), 1);
        assert_eq!(w1.balance(), 0);
        assert_eq!(w2.name(), "tx2");
        assert_eq!(w2.history_len(), 1);
        assert_eq!(w2.balance(), 0);
        let rh1 = s.wallet_history(&p1).root_hash().unwrap();
        let rh2 = s.wallet_history(&p2).root_hash().unwrap();
        assert_eq!(&rh1, w1.history_hash());
        assert_eq!(&rh2, w2.history_hash());

        let iw1 = TxIssue::new(&p1, 1000, 1, &s1);
        let iw2 = TxIssue::new(&p2, 100, 2, &s2);
        CurrencyTx::from(iw1.clone()).execute(&v).unwrap();
        CurrencyTx::from(iw2.clone()).execute(&v).unwrap();
        let w1 = s.wallet(&p1).unwrap().unwrap();
        let w2 = s.wallet(&p2).unwrap().unwrap();

        assert_eq!(w1.balance(), 1000);
        assert_eq!(w2.balance(), 100);
        assert_eq!(w1.history_len(), 2);
        assert_eq!(w2.history_len(), 2);
        let rh1 = s.wallet_history(&p1).root_hash().unwrap();
        let rh2 = s.wallet_history(&p2).root_hash().unwrap();
        assert_eq!(&rh1, w1.history_hash());
        assert_eq!(&rh2, w2.history_hash());

        let tw = TxTransfer::new(&p1, &p2, 400, 3, &s1);
        CurrencyTx::from(tw.clone()).execute(&v).unwrap();
        let w1 = s.wallet(&p1).unwrap().unwrap();
        let w2 = s.wallet(&p2).unwrap().unwrap();

        assert_eq!(w1.balance(), 600);
        assert_eq!(w2.balance(), 500);
        assert_eq!(w1.history_len(), 3);
        assert_eq!(w2.history_len(), 3);
        let rh1 = s.wallet_history(&p1).root_hash().unwrap();
        let rh2 = s.wallet_history(&p2).root_hash().unwrap();
        assert_eq!(&rh1, w1.history_hash());
        assert_eq!(&rh2, w2.history_hash());

        let h1 = s.wallet_history(&p1).values().unwrap();
        let h2 = s.wallet_history(&p2).values().unwrap();
        assert_eq!(h1, vec![cw1.hash(), iw1.hash(), tw.hash()]);
        assert_eq!(h2, vec![cw2.hash(), iw2.hash(), tw.hash()]);
    }
}
