extern crate serde;
extern crate serde_json;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate exonum;
extern crate router;
extern crate bodyparser;
extern crate iron;

use exonum::blockchain::{Blockchain, Service, GenesisConfig, Transaction, ApiContext};
use exonum::messages::{RawTransaction, FromRaw, Message};
use exonum::node::{Node, NodeConfig, NodeApiConfig, TransactionSend};
use exonum::storage::{Fork, MemoryDB, MapIndex};
use exonum::crypto::{PublicKey, Hash};
use exonum::encoding::{self, Field};
use exonum::api::{Api, ApiError};
use iron::prelude::*;
use iron::Handler;
use router::Router;

// // // // // // // // // // PERSISTENT DATA // // // // // // // // // //

encoding_struct! {
    struct Wallet {
        const SIZE = 48;

        field pub_key:            &PublicKey  [00 => 32]
        field name:               &str        [32 => 40]
        field balance:            u64         [40 => 48]
    }
}

impl Wallet {
    pub fn increase(&mut self, amount: u64) {
        let balance = self.balance() + amount;
        Field::write(&balance, &mut self.raw, 40, 48);
    }

    pub fn decrease(&mut self, amount: u64) {
        let balance = self.balance() - amount;
        Field::write(&balance, &mut self.raw, 40, 48);
    }
}

// // // // // // // // // // TRANSACTIONS // // // // // // // // // //

pub const TX_WALLET_ID: u16 = 1;

message! {
    struct TxCreateWallet {
        const TYPE = SERVICE_ID;
        const ID = TX_WALLET_ID;
        const SIZE = 40;

        field pub_key:     &PublicKey  [00 => 32]
        field name:        &str        [32 => 40]
    }
}

pub const TX_ISSUE_ID: u16 = 2;

message! {
    struct TxIssue {
        const TYPE = SERVICE_ID;
        const ID = TX_ISSUE_ID;
        const SIZE = 48;

        field pub_key:     &PublicKey  [00 => 32]
        field amount:      u64         [32 => 40]
        field seed:        u64         [40 => 48]
    }
}

pub const TX_TRANSFER_ID: u16 = 3;

message! {
    struct TxTransfer {
        const TYPE = SERVICE_ID;
        const ID = TX_TRANSFER_ID;
        const SIZE = 72;

        field from:        &PublicKey  [00 => 32]
        field to:          &PublicKey  [32 => 64]
        field amount:      u64         [64 => 72]
    }
}

// // // // // // // // // // CONTRACTS // // // // // // // // // //

impl Transaction for TxCreateWallet {
    fn verify(&self) -> bool {
        self.verify_signature(self.pub_key())
    }

    fn execute(&self, view: &mut Fork) {
        let mut schema = CurrencySchema { view };
        if let None = schema.wallet(self.pub_key()) {
            let wallet = Wallet::new(self.pub_key(), self.name(), 0);
            schema.wallets().put(self.pub_key(), wallet)
        }
    }
}

impl Transaction for TxIssue {
    fn verify(&self) -> bool {
        self.verify_signature(self.pub_key())
    }

    fn execute(&self, view: &mut Fork) {
        let mut schema = CurrencySchema { view };
        if let Some(mut wallet) = schema.wallet(self.pub_key()) {
            wallet.increase(self.amount());
            schema.wallets().put(self.pub_key(), wallet)
        }
    }
}


impl Transaction for TxTransfer {
    fn verify(&self) -> bool {
        self.verify_signature(self.from()) && (*self.from() != *self.to())
    }

    fn execute(&self, view: &mut Fork) {
        let mut schema = CurrencySchema { view };
        let sender = schema.wallet(self.from());
        let receiver = schema.wallet(self.to());
        if let (Some(mut sender), Some(mut receiver)) = (sender, receiver) {
            let amount = self.amount();
            if sender.balance() >= amount {
                sender.decrease(amount);
                receiver.increase(amount);
                println!("{:?} => {:?}", sender, receiver);
                let mut wallets = schema.wallets();
                wallets.put(self.from(), sender);
                wallets.put(self.to(), receiver);
            }
        }
    }
}

// // // // // // // // // // REST API // // // // // // // // // //

#[derive(Clone)]
struct CryptocurrencyApi<T> {
    channel: T,
}

impl<T: TransactionSend + Clone + 'static> Api for CryptocurrencyApi<T> {
    fn wire(&self, router: &mut Router) {

        #[serde(untagged)]
        #[derive(Clone, Serialize, Deserialize)]
        enum TransactionRequest {
            CreateWallet(TxCreateWallet),
            Issue(TxIssue),
            Transfer(TxTransfer),
        }

        impl Into<Box<Transaction>> for TransactionRequest {
            fn into(self) -> Box<Transaction> {
                match self {
                    TransactionRequest::CreateWallet(trans) => Box::new(trans),
                    TransactionRequest::Issue(trans) => Box::new(trans),
                    TransactionRequest::Transfer(trans) => Box::new(trans),
                }
            }
        }

        #[derive(Serialize, Deserialize)]
        struct TransactionResponse {
            tx_hash: Hash,
        }

        let self_ = self.clone();
        let transaction = move |req: &mut Request| -> IronResult<Response> {
            match req.get::<bodyparser::Struct<TransactionRequest>>() {
                Ok(Some(transaction)) => {
                    let transaction: Box<Transaction> = transaction.into();
                    let tx_hash = transaction.hash();
                    self_.channel.send(transaction).map_err(|e| ApiError::Events(e))?;
                    let json = TransactionResponse { tx_hash };
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

// // // // // // // // // // STORAGE DATA LAYOUT // // // // // // // // // //

pub struct CurrencySchema<'a> {
    view: &'a mut Fork,
}

impl<'a> CurrencySchema<'a> {
    pub fn wallets(&mut self) -> MapIndex<&mut Fork, PublicKey, Wallet> {
        MapIndex::new(vec![20], self.view)
    }

    pub fn wallet(&mut self, pub_key: &PublicKey) -> Option<Wallet> {
        self.wallets().get(pub_key)
    }
}

// // // // // // // // // // SERVICE DECLARATION // // // // // // // // // //

pub const SERVICE_ID: u16 = 1;

struct CurrencyService;

impl Service for CurrencyService {
    fn service_name(&self) -> &'static str { "cryptocurrency" }

    fn service_id(&self) -> u16 { SERVICE_ID }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, encoding::Error> {
        let trans: Box<Transaction> = match raw.message_type() {
            TX_TRANSFER_ID => Box::new(TxTransfer::from_raw(raw)?),
            TX_ISSUE_ID => Box::new(TxIssue::from_raw(raw)?),
            TX_WALLET_ID => Box::new(TxCreateWallet::from_raw(raw)?),
            _ => {
                return Err(encoding::Error::IncorrectMessageType { message_type: raw.message_type() });
            },
        };
        Ok(trans)
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

// // // // // // // // // // ENTRY POINT // // // // // // // // // //

fn main() {
    exonum::crypto::init();
    exonum::helpers::init_logger().unwrap();

    let db = MemoryDB::new();
    let services: Vec<Box<Service>> = vec![
        Box::new(CurrencyService),
    ];
    let blockchain = Blockchain::new(Box::new(db), services);

    let (public_key, secret_key) = exonum::crypto::gen_keypair();

    let peer_address = "0.0.0.0:2000".parse().unwrap();
    let api_address = "0.0.0.0:8000".parse().unwrap();

    let genesis = GenesisConfig::new(vec![public_key].into_iter());

    let api_cfg = NodeApiConfig {
        enable_blockchain_explorer: true,
        public_api_address: Some(api_address),
        private_api_address: None,
    };

    let node_cfg = NodeConfig {
        listen_address: peer_address,
        peers: vec![],
        public_key,
        secret_key,
        genesis,
        network: Default::default(),
        whitelist: Default::default(),
        api: api_cfg,
        mempool: Default::default(),
    };

    let mut node = Node::new(blockchain, node_cfg);
    node.run().unwrap();
}
