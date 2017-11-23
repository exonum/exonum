extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate exonum;
extern crate router;
extern crate bodyparser;
extern crate iron;

use exonum::blockchain::{Blockchain, Service, ServiceContext, Schema, GenesisConfig,
                         ValidatorKeys, Transaction, ApiContext};
use exonum::node::{Node, NodeConfig, NodeApiConfig, ApiSender};
use exonum::messages::{RawTransaction, FromRaw, Message};
use exonum::encoding::serialize::json::reexport::Value;
use exonum::storage::{Fork, Snapshot, MemoryDB, MapIndex, Entry};
use exonum::crypto::{PublicKey, Hash};
use exonum::encoding;
use exonum::api::Api;
use iron::prelude::*;
use iron::Handler;
use router::Router;
use std::time::SystemTime;

const SERVICE_ID: u16 = 1;
const TX_TIME_ID: u16 = 1;

// SCHEMA

encoding_struct! {
    struct Time {
        const SIZE = 12;

        field time:     SystemTime  [00 => 12]
    }
}

pub struct TimeSchema<T> {
    view: T,
}

impl<T: AsRef<Snapshot>> TimeSchema<T> {
    pub fn new(view: T) -> Self {
        TimeSchema { view }
    }

    pub fn validators_time(&self) -> MapIndex<&Snapshot, PublicKey, Time> {
        MapIndex::new("time.validators_time", self.view.as_ref())
    }

    pub fn time(&self) -> Entry<&Snapshot, Time> {
        Entry::new("time.time", self.view.as_ref())
    }
}


impl<'a> TimeSchema<&'a mut Fork> {
    pub fn validators_time_mut(&mut self) -> MapIndex<&mut Fork, PublicKey, Time> {
        MapIndex::new("time.validators_time", self.view)
    }

    pub fn time_mut(&mut self) -> Entry<&mut Fork, Time> {
        Entry::new("time.time", self.view)
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
        let validator_keys = Schema::new(&view).actual_configuration().validator_keys;

        if validator_keys.iter().any(|&validator| {
            validator.service_key == *self.pub_key()
        })
        {
            let mut schema = TimeSchema::new(view);
            if let Some(storage_time) = schema.validators_time().get(self.pub_key()) {
                if storage_time.time() < self.time() {
                    schema.validators_time_mut().put(
                        self.pub_key(),
                        Time::new(self.time()),
                    );
                }
                else {
                    return;
                }
            }
            else {
                schema.validators_time_mut().put(
                    self.pub_key(),
                    Time::new(self.time()),
                );
            }

            let mut validators_time = Vec::new();
            {
                let idx = schema.validators_time();

                for pair in idx.iter() {
                    let (pub_key, time) = (pair.0, pair.1.time());
                    if validator_keys.iter().any(|validator| {
                        validator.service_key == pub_key
                    })
                        {
                            validators_time.push(time);
                        }
                }
            }

            let f = validator_keys.len() / 3;
            if validators_time.len() > f {
                validators_time.sort();
                validators_time.reverse();
                if let Some(current_time) = schema.time().get() {
                    if current_time.time() < validators_time[f] {
                        schema.time_mut().set(Time::new(validators_time[f]));
                    }
                }
                else {
                    schema.time_mut().set(Time::new(validators_time[f]));
                }
            }
        }
    }
}

// API

#[derive(Serialize, Deserialize)]
pub struct TxResponse {
    pub tx_hash: Hash,
}

#[derive(Clone)]
struct TimeApi {
    channel: ApiSender,
    blockchain: Blockchain,
}

impl TimeApi {
    fn get_current_time(&self, _: &mut Request) -> IronResult<Response> {
        let view = self.blockchain.snapshot();
        let schema = TimeSchema::new(&view);
        let current_time = schema.time().get();
        self.ok_response(&serde_json::to_value(current_time).unwrap())
    }

    fn get_validators_time(&self, _: &mut Request) -> IronResult<Response> {
        let view = self.blockchain.snapshot();
        let schema = TimeSchema::new(&view);
        let idx = schema.validators_time();
        let validators_time: Vec<Time> = idx.values().collect();
        if validators_time.is_empty() {
            self.not_found_response(&serde_json::to_value("Validators time database if empty")
                .unwrap())
        } else {
            self.ok_response(&serde_json::to_value(validators_time).unwrap())
        }
    }

    fn wire_private(&self, router: &mut Router) {
        let self_ = self.clone();
        let get_validators_time = move |req: &mut Request| self_.get_validators_time(req);
        router.get(
            "/validators_time",
            get_validators_time,
            "get_validators_time",
        );
    }
}

impl Api for TimeApi {
    fn wire(&self, router: &mut Router) {
        let self_ = self.clone();
        let get_current_time = move |req: &mut Request| self_.get_current_time(req);
        router.get("/current_time", get_current_time, "get_current_time");
    }
}



// SERVICE DECLARATION

struct TimeService;

impl Service for TimeService {
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
        let (pub_key, sec_key) = (*context.public_key(), context.secret_key().clone());
        context.add_transaction(Box::new(TxTime::new(SystemTime::now(), &pub_key, &sec_key)));
    }

    fn private_api_handler(&self, ctx: &ApiContext) -> Option<Box<Handler>> {
        let mut router = Router::new();
        let api = TimeApi {
            channel: ctx.node_channel().clone(),
            blockchain: ctx.blockchain().clone(),
        };
        api.wire(&mut router);
        Some(Box::new(router))
    }

    fn public_api_handler(&self, ctx: &ApiContext) -> Option<Box<Handler>> {
        let mut router = Router::new();
        let api = TimeApi {
            channel: ctx.node_channel().clone(),
            blockchain: ctx.blockchain().clone(),
        };
        api.wire_private(&mut router);
        Some(Box::new(router))
    }
}

fn main() {
    exonum::helpers::init_logger().unwrap();

    println!("Creating in-memory database...");
    let db = MemoryDB::new();
    let services: Vec<Box<Service>> = vec![Box::new(TimeService)];
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
