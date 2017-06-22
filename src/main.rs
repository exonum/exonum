extern crate serde;
extern crate serde_json;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate exonum;
extern crate router;
extern crate bodyparser;
extern crate iron;

use exonum::blockchain::{Blockchain, Service, GenesisConfig, Transaction, ApiContext};
use exonum::messages::{RawTransaction, RawMessage, FromRaw, Message};
use exonum::node::{Node, NodeConfig, NodeApiConfig, TransactionSend};
use exonum::storage::{Fork, MemoryDB, MapIndex};
use exonum::crypto::{PublicKey, Hash};
use exonum::encoding::{self, Field};
use exonum::api::{Api, ApiError};
use iron::prelude::*;
use iron::Handler;
use router::Router;

pub const SERVICE_ID: u16 = 128;
pub const TX_WALLET_ID: u16 = 130;
pub const TX_ISSUE_ID: u16 = 129;
pub const TX_TRANSFER_ID: u16 = 128;

encoding_struct! {
    struct Wallet {
        const SIZE = 48;

        field pub_key:            &PublicKey  [00 => 32]
        field name:               &str        [32 => 40]
        field balance:            u64         [40 => 48]
    }
}

impl Wallet {
    pub fn set_balance(&mut self, balance: u64) {
        Field::write(&balance, &mut self.raw, 40, 48);
    }

    pub fn transfer_to(&mut self, other: &mut Wallet, amount: u64) {
        if self.pub_key() != other.pub_key() && self.balance() >= amount {
            let self_amount = self.balance() - amount;
            let other_amount = other.balance() + amount;
            self.set_balance(self_amount);
            other.set_balance(other_amount);
        }
    }
}

message! {
    struct TxCreateWallet {
        const TYPE = SERVICE_ID;
        const ID = TX_WALLET_ID;
        const SIZE = 40;

        field pub_key:     &PublicKey  [00 => 32]
        field name:        &str        [32 => 40]
    }
}

impl TxCreateWallet {
    pub fn execute(&self, fork: &mut Fork) {
        let mut schema = CurrencySchema::new(fork);
        if let None = schema.wallet(self.pub_key()) {
            let wallet = Wallet::new(self.pub_key(), self.name(), 0);
            schema.wallets().put(self.pub_key(), wallet)
        }
    }
}

impl TxIssue {
    pub fn execute(&self, fork: &mut Fork) {
        let mut schema = CurrencySchema::new(fork);
        if let Some(mut wallet) = schema.wallet(self.wallet()) {
            let new_balance = wallet.balance() + self.amount();
            wallet.set_balance(new_balance);
            schema.wallets().put(self.wallet(), wallet)
        }
    }
}

impl TxTransfer {
    pub fn execute(&self, fork: &mut Fork) {
        let mut schema = CurrencySchema::new(fork);
        let sender = schema.wallet(self.from());
        let receiver = schema.wallet(self.to());
        if let (Some(mut sender), Some(mut receiver)) = (sender, receiver) {
            sender.transfer_to(&mut receiver, self.amount());
            let mut wallets = schema.wallets();
            wallets.put(self.from(), sender);
            wallets.put(self.to(), receiver);
        }
    }
}

message! {
    struct TxIssue {
        const TYPE = SERVICE_ID;
        const ID = TX_ISSUE_ID;
        const SIZE = 48;

        field wallet:      &PublicKey  [00 => 32]
        field amount:      u64         [32 => 40]
        field seed:        u64         [40 => 48]
    }
}

message! {
    struct TxTransfer {
        const TYPE = SERVICE_ID;
        const ID = TX_TRANSFER_ID;
        const SIZE = 80;

        field from:        &PublicKey  [00 => 32]
        field to:          &PublicKey  [32 => 64]
        field amount:      u64         [64 => 72]
        field seed:        u64         [72 => 80]
    }
}

#[derive(Serialize, Deserialize)]
struct TransactionResult {
    tx_hash: Hash,
}

#[derive(Clone)]
struct CryptocurrencyApi<T> {
    channel: T,
}

impl<T: TransactionSend + Clone + 'static> Api for CryptocurrencyApi<T> {
    fn wire(&self, router: &mut Router) {
        let self_ = self.clone();
        let transaction = move |req: &mut Request| -> IronResult<Response> {
            match req.get::<bodyparser::Struct<CurrencyTx>>() {
                Ok(Some(transaction)) => {
                    let tx_hash = transaction.hash();
                    self_.channel.send(transaction).map_err(|e| ApiError::Events(e))?;
                    let json = TransactionResult { tx_hash };
                    self_.ok_response(&serde_json::to_value(&json).unwrap())
                }
                Ok(None) => Err(ApiError::IncorrectRequest("Empty request body".into()))?,
                Err(e) => Err(ApiError::IncorrectRequest(Box::new(e)))?,
            }
        };
        let route_post = "/v1/wallets/transaction";
        router.post(&route_post, transaction, "transaction");
    }
}

#[serde(untagged)]
#[derive(Debug, Clone, Serialize, Deserialize)]
enum CurrencyTx {
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
}

impl FromRaw for CurrencyTx {
    fn from_raw(raw: RawMessage) -> Result<Self, encoding::Error> {
        match raw.message_type() {
            TX_TRANSFER_ID => Ok(CurrencyTx::Transfer(TxTransfer::from_raw(raw)?)),
            TX_ISSUE_ID => Ok(CurrencyTx::Issue(TxIssue::from_raw(raw)?)),
            TX_WALLET_ID => Ok(CurrencyTx::CreateWallet(TxCreateWallet::from_raw(raw)?)),
            _ => Err(encoding::Error::IncorrectMessageType { message_type: raw.message_type() }),
        }
    }
}

impl Transaction for CurrencyTx {
    fn verify(&self) -> bool {
        match *self {
            CurrencyTx::CreateWallet(ref msg) => {
                self.verify_signature(msg.pub_key())
            },
            CurrencyTx::Issue(ref msg) => {
                self.verify_signature(msg.wallet())
            },
            CurrencyTx::Transfer(ref msg) => {
                self.verify_signature(msg.from()) && (*msg.from() != *msg.to())
            },
        }
    }

    fn execute(&self, view: &mut Fork) {
        match *self {
            CurrencyTx::Transfer(ref msg) => msg.execute(view),
            CurrencyTx::Issue(ref msg) => msg.execute(view),
            CurrencyTx::CreateWallet(ref msg) => msg.execute(view),
        }
    }
}

pub struct CurrencySchema<T> {
    view: T,
}

impl<T> CurrencySchema<T> {
    pub fn new(view: T) -> Self {
        CurrencySchema { view }
    }
}

impl<'a> CurrencySchema<&'a mut Fork> {
    /// Returns `MerklePatriciaTable` with wallets.
    pub fn wallets(&mut self) -> MapIndex<&mut Fork, PublicKey, Wallet> {
        MapIndex::new(vec![20], self.view)
    }

    /// Returns wallet for the given public key.
    pub fn wallet(&mut self, pub_key: &PublicKey) -> Option<Wallet> {
        self.wallets().get(pub_key)
    }

}

struct CurrencyService {

}

impl CurrencyService {
    pub fn new() -> Self {
        CurrencyService { }
    }
}

impl Service for CurrencyService {
    fn service_name(&self) -> &'static str { "cryptocurrency" }

    fn service_id(&self) -> u16 { SERVICE_ID }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, encoding::Error> {
        CurrencyTx::from_raw(raw).map(|tx| Box::new(tx) as Box<Transaction>)
    }

    fn public_api_handler(&self, ctx: &ApiContext) -> Option<Box<Handler>> {
        let mut router = Router::new();
        let api = CryptocurrencyApi {
            channel: ctx.node_channel().clone(),
        };
        api.wire(&mut router);
        Some(Box::new(router))
    }
}

fn main() {
    exonum::crypto::init();
    exonum::helpers::init_logger().unwrap();

    let db = MemoryDB::new();
    let services: Vec<Box<Service>> = vec![
        Box::new(CurrencyService::new()),
    ];
    let blockchain = Blockchain::new(Box::new(db), services);

    let (public_key, secret_key) = exonum::crypto::gen_keypair();

    let peer = "0.0.0.0:2000".parse().unwrap();
    let api = "0.0.0.0:8000".parse().unwrap();

    let genesis = GenesisConfig::new(vec![public_key].into_iter());

    let api_cfg = NodeApiConfig {
        enable_blockchain_explorer: true,
        public_api_address: Some(api),
        private_api_address: None,
    };

    let node_cfg = NodeConfig {
        listen_address: peer,
        peers: vec![peer],
        public_key,
        secret_key,
        genesis,
        network: Default::default(),
        whitelist: Default::default(),
        api: api_cfg,
    };

    let mut node = Node::new(blockchain, node_cfg);
    node.run().unwrap();
}
