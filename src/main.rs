extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate exonum;
extern crate router;
extern crate bodyparser;
extern crate iron;

use exonum::blockchain::{self, Blockchain, ServiceContext, GenesisConfig, ValidatorKeys,
                         Transaction, ApiContext, StoredConfiguration};
use exonum::node::{Node, NodeConfig, NodeApiConfig, TransactionSend, ApiSender};
use exonum::messages::{RawTransaction, FromRaw, Message};
use exonum::encoding::serialize::json::reexport::Value;
use exonum::storage::{Fork, MemoryDB, MapIndex, Entry};
use exonum::crypto::{PublicKey, Hash, HexValue};
use exonum::encoding;
use exonum::api::{Api, ApiError};
use iron::prelude::*;
use iron::Handler;
use router::Router;
use std::time::SystemTime;
use std::cmp::PartialOrd;

const SERVICE_ID: u16 = 1;
const TX_TIME_ID: u16 = 1;

// SCHEMA

encoding_struct! {
    struct Time {
        const SIZE = 12;

        field time:     SystemTime  [00 => 12]
    }
}

pub struct Schema<'a> {
    view: &'a mut Fork,
}

impl<'a> Schema<'a> {
    pub fn validators_time(&mut self) -> MapIndex<&mut Fork, PublicKey, Time> {
        MapIndex::new("time.validators_time", self.view)
    }

    pub fn validator_time(&mut self, pub_key: &PublicKey) -> Option<Time> {
        self.validators_time().get(pub_key)
    }

    pub fn time(&mut self) -> Entry<&mut Fork, Time> {
        Entry::new("time.time", self.view)
    }

    pub fn current_time(&mut self) -> Option<Time> {
        self.time().get()
    }
}


// TRANSACTION

message! {
    struct TxTime {
        const TYPE = SERVICE_ID;
        const ID = TX_TIME_ID;
        const SIZE = 44;

        field time:     SystemTime  [00 => 12]
        field pub_key:  &PublicKey  [12 => 44]
    }
}

impl Transaction for TxTime {
    fn verify(&self) -> bool {
        self.verify_signature(self.pub_key())
    }

    fn execute(&self, view: &mut Fork) {
        let validator_keys = blockchain::Schema::new(view)
            .actual_configuration()
            .validator_keys;
        if let Some(_) = validator_keys.iter().find(|&validator| validator.service_key == *self.pub_key()) {
        }
    }
}

// REST API

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum TxRequest {
    SendTime(TxTime),
}

impl Into<Box<Transaction>> for TxRequest {
    fn into(self) -> Box<Transaction> {
        match self {
            TxRequest::SendTime(trans) => Box::new(trans),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct TxResponse {
    tx_hash: Hash,
}

pub mod public {
    use serde_json;
    use exonum::api::{self, ApiError};
    use exonum::blockchain::{Blockchain, Transaction};
    use exonum::node::{ApiSender, TransactionSend};
    use exonum::crypto::PublicKey;
    use iron::prelude::*;
    use router::Router;
    use bodyparser;
    use super::{Schema, TxRequest, TxResponse};

    #[derive(Clone)]
    pub struct Api {
        blockchain: Blockchain,
        channel: ApiSender,
    }

    impl Api {
        pub fn new(blockchain: Blockchain, channel: ApiSender) -> Self {
            Api {
                blockchain,
                channel,
            }
        }
    }

    impl api::Api for Api {
        fn wire(&self, router: &mut Router) {

            let self_ = self.clone();
            let transaction = move |req: &mut Request| -> IronResult<Response> {
                match req.get::<bodyparser::Struct<TxRequest>>() {
                    Ok(Some(transaction)) => {
                        let transaction: Box<Transaction> = transaction.into();
                        let tx_hash = transaction.hash();
                        self_.channel.send(transaction).map_err(ApiError::Io)?;
                        let json = TxResponse { tx_hash };
                        self_.ok_response(&serde_json::to_value(&json).unwrap())
                    }
                    Ok(None) => Err(ApiError::IncorrectRequest("Empty request body".into()))?,
                    Err(e) => Err(ApiError::IncorrectRequest(Box::new(e)))?,
                }
            };
            router.post(&"v1/transaction", transaction, "transaction");

            let self_ = self.clone();
            let get_current_time = move |_: &mut Request| -> IronResult<Response> {
                let mut view = self_.blockchain.fork();
                let mut schema = Schema { view: &mut view };
                self_.ok_response(&serde_json::to_value(schema.current_time()).unwrap())
            };
            router.get("/v1/current_time", get_current_time, "current_time");
        }
    }
}

pub mod private {
    use serde_json;
    use exonum::api::{self, ApiError};
    use exonum::blockchain::{Blockchain, Transaction};
    use exonum::node::{ApiSender, TransactionSend};
    use exonum::crypto::PublicKey;
    use iron::prelude::*;
    use router::Router;
    use bodyparser;
    use super::{Schema, TxRequest, TxResponse};

    #[derive(Clone)]
    pub struct Api {
        blockchain: Blockchain,
        channel: ApiSender,
    }

    impl Api {
        pub fn new(blockchain: Blockchain, channel: ApiSender) -> Self {
            Api {
                blockchain,
                channel,
            }
        }
    }

    impl api::Api for Api {
        fn wire(&self, router: &mut Router) {

            let self_ = self.clone();
            let transaction = move |req: &mut Request| -> IronResult<Response> {
                match req.get::<bodyparser::Struct<TxRequest>>() {
                    Ok(Some(transaction)) => {
                        let transaction: Box<Transaction> = transaction.into();
                        let tx_hash = transaction.hash();
                        self_.channel.send(transaction).map_err(ApiError::Io)?;
                        let json = TxResponse { tx_hash };
                        self_.ok_response(&serde_json::to_value(&json).unwrap())
                    }
                    Ok(None) => Err(ApiError::IncorrectRequest("Empty request body".into()))?,
                    Err(e) => Err(ApiError::IncorrectRequest(Box::new(e)))?,
                }
            };
            router.post(&"v1/transaction", transaction, "transaction");

            let self_ = self.clone();
            let get_validators_time = move |_: &mut Request| -> IronResult<Response> {
                let mut view = self_.blockchain.fork();
                let mut schema = Schema { view: &mut view };
                let idx = schema.validators_time();
                let validators_time: Vec<super::Time> = idx.values().collect();
                if validators_time.is_empty() {
                    self_.not_found_response(
                        &serde_json::to_value("Validators time database is empty").unwrap(),
                    )
                } else {
                    self_.ok_response(&serde_json::to_value(validators_time).unwrap())
                }
            };
            router.get(
                "/v1/validators_time",
                get_validators_time,
                "validators_time",
            );
        }
    }
}

// SERVICE DECLARATION

struct Service;

impl blockchain::Service for Service {
    fn service_name(&self) -> &'static str {
        "time"
    }

    fn service_id(&self) -> u16 {
        SERVICE_ID
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, encoding::Error> {
        match raw.message_type() {
            TX_TIME_ID => Ok(Box::new(TxTime::from_raw(raw)?)),
            _ => {
                let error =
                    encoding::Error::IncorrectMessageType { message_type: raw.message_type() };
                Err(error)
            }
        }
    }

    fn initialize(&self, _fork: &mut Fork) -> Value {
        Value::Null
    }

    fn handle_commit(&self, context: &mut ServiceContext) {
        let (pub_key, sec_key) = (context.public_key().clone(), context.secret_key().clone());
        context.add_transaction(Box::new(TxTime::new(SystemTime::now(), &pub_key, &sec_key)));
    }

    fn private_api_handler(&self, ctx: &ApiContext) -> Option<Box<Handler>> {
        let mut router = Router::new();
        let blockchain = ctx.blockchain().clone();
        let channel = ctx.node_channel().clone();
        let api = private::Api::new(blockchain, channel);
        api.wire(&mut router);
        Some(Box::new(router))
    }

    fn public_api_handler(&self, ctx: &ApiContext) -> Option<Box<Handler>> {
        let mut router = Router::new();
        let blockchain = ctx.blockchain().clone();
        let channel = ctx.node_channel().clone();
        let api = public::Api::new(blockchain, channel);
        api.wire(&mut router);
        Some(Box::new(router))
    }
}



fn main() {
    exonum::helpers::init_logger().unwrap();

    println!("Creating in-memory database...");
    let db = MemoryDB::new();
    let services: Vec<Box<blockchain::Service>> = vec![Box::new(Service)];
    let blockchain = Blockchain::new(Box::new(db), services);

    let (consensus_public_key, consensus_secret_key) = exonum::crypto::gen_keypair();
    let (service_public_key, service_secret_key) = exonum::crypto::gen_keypair();

    let validator_keys = ValidatorKeys {
        consensus_key: consensus_public_key,
        service_key: service_public_key,
    };
    let genesis = GenesisConfig::new(vec![validator_keys].into_iter());

    let api_address = "0.0.0.0:8000".parse().unwrap();
    let api_cfg = NodeApiConfig {
        public_api_address: Some(api_address),
        ..Default::default()
    };

    let peer_address = "0.0.0.0:2000".parse().unwrap();

    let node_cfg = NodeConfig {
        listen_address: peer_address,
        peers: vec![],
        service_public_key,
        service_secret_key,
        consensus_public_key,
        consensus_secret_key,
        genesis,
        external_address: None,
        network: Default::default(),
        whitelist: Default::default(),
        api: api_cfg,
        mempool: Default::default(),
        services_configs: Default::default(),
    };

    println!("Starting a single node...");
    let node = Node::new(blockchain, node_cfg);

    println!("Blockchain is ready for transactions!");
    node.run().unwrap();
}
