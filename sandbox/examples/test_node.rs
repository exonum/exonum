#![feature(custom_derive)]
#![feature(plugin)]
#![plugin(serde_macros)]
#![feature(question_mark)]

extern crate exonum;
extern crate timestamping;
extern crate env_logger;
extern crate clap;
extern crate toml;
extern crate serde;

use std::path::Path;
use std::fs;
use std::error::Error;
use std::io::prelude::*;
use std::fs::File;

use clap::{Arg, App, SubCommand};
use toml::Encoder;
use serde::Serialize;

use exonum::node::{Node, Configuration};
use exonum::events::{NetworkConfiguration, EventsConfiguration};
use exonum::crypto::{gen_keypair_from_seed, Seed, PublicKey, SecretKey};
use exonum::storage::{MemoryDB, LevelDB, LevelDBOptions};

use timestamping::TimestampingBlockchain;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct TestnetValidator {
    public_key: PublicKey,
    secret_key: SecretKey,
    address: ::std::net::SocketAddr,
}

#[derive(Debug, Serialize, Deserialize)]
struct TestnetConfiguration {
    validators: Vec<TestnetValidator>,
    round_timeout: u32,
    status_timeout: u32,
    peers_timeout: u32,
    max_incoming_connections: usize,
    max_outgoing_connections: usize,
}

impl TestnetConfiguration {
    fn from_file(path: &Path) -> Result<TestnetConfiguration, Box<Error>> {
        let mut file = File::open(path)?;
        let mut toml = String::new();
        file.read_to_string(&mut toml)?;
        let cfg = toml::decode_str(&toml);
        return Ok(cfg.unwrap());
    }

    fn save_to_file(&self, path: &Path) -> Result<(), Box<Error>> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).unwrap();
        }

        let mut e = Encoder::new();
        self.serialize(&mut e).unwrap();
        let mut file = File::create(path).unwrap();
        file.write_all(toml::encode_str(&e.toml).as_bytes())?;

        Ok(())
    }

    fn gen(validators_count: u8) -> TestnetConfiguration {
        let mut pairs = Vec::new();
        for i in 0..validators_count {
            let keys = gen_keypair_from_seed(&Seed::from_slice(&vec![i; 32]).unwrap());
            let addr = format!("127.0.0.1:{}", 7000 + i as u32).parse().unwrap();
            let pair = TestnetValidator {
                public_key: keys.0.clone(),
                secret_key: keys.1.clone(),
                address: addr,
            };
            pairs.push(pair);
        }

        TestnetConfiguration {
            validators: pairs,
            round_timeout: 1000,
            status_timeout: 5000,
            peers_timeout: 10000,
            max_incoming_connections: 128,
            max_outgoing_connections: 128,
        }
    }

    fn to_node_configuration(&self,
                             idx: usize,
                             known_peers: Vec<::std::net::SocketAddr>)
                             -> Configuration {
        let validator = self.validators[idx].clone();
        let validators: Vec<_> = self.validators
            .iter()
            .map(|v| v.public_key)
            .collect();

        Configuration {
            public_key: validator.public_key,
            secret_key: validator.secret_key,
            round_timeout: self.round_timeout,
            status_timeout: self.status_timeout,
            peers_timeout: self.peers_timeout,
            network: NetworkConfiguration {
                listen_address: validator.address,
                max_incoming_connections: self.max_incoming_connections,
                max_outgoing_connections: self.max_outgoing_connections,
            },
            events: EventsConfiguration::new(),
            peer_discovery: known_peers,
            validators: validators,
        }
    }
}

fn main() {
    ::std::env::set_var("RUST_LOG", "exonum=info");
    env_logger::init().unwrap();

    let app = App::new("Testnet node")
        .version("0.1")
        .author("Aleksey S. <aleksei.sidorov@xdev.re>")
        .about("Test network node")
        .arg(Arg::with_name("CONFIG")
            .short("c")
            .long("config")
            .value_name("CONFIG_PATH")
            .help("Sets a testnet config file")
            .required(true)
            .takes_value(true))
        .subcommand(SubCommand::with_name("generate")
            .about("Generates default configuration file")
            .version("0.1")
            .author("Aleksey S. <aleksei.sidorov@xdev.re>")
            .arg(Arg::with_name("COUNT")
                .help("Validators count")
                .required(true)
                .index(1)))
        .subcommand(SubCommand::with_name("run")
            .about("Run test node with the given validator id")
            .version("0.1")
            .author("Aleksey S. <aleksei.sidorov@xdev.re>")
            .arg(Arg::with_name("LEVELDB_PATH")
                .short("d")
                .long("leveldb-path")
                .value_name("LEVELDB_PATH")
                .help("Use leveldb database with the given path")
                .takes_value(true))
            .arg(Arg::with_name("PEERS")
                .short("p")
                .long("known-peers")
                .value_name("PEERS")
                .help("Comma separated list of known validator ids")
                .takes_value(true))
            .arg(Arg::with_name("VALIDATOR")
                .help("Sets a validator id")
                .required(true)
                .index(1)));

    let matches = app.get_matches();
    let path = Path::new(matches.value_of("CONFIG").unwrap());
    match matches.subcommand() {
        ("generate", Some(matches)) => {
            let count: u8 = matches.value_of("COUNT").unwrap().parse().unwrap();
            let cfg = TestnetConfiguration::gen(count);
            cfg.save_to_file(&path).unwrap();
            println!("The configuration was successfully written to file {:?}",
                     path);
        }
        ("run", Some(matches)) => {
            let cfg = TestnetConfiguration::from_file(path).unwrap();
            let idx: usize = matches.value_of("VALIDATOR").unwrap().parse().unwrap();
            let peers = match matches.value_of("PEERS") {
                Some(string) => {
                    string.split(" ")
                        .map(|x| -> usize { x.parse().unwrap() })
                        .map(|x| cfg.validators[x].address)
                        .collect()
                }
                None => {
                    cfg.validators
                        .iter()
                        .map(|v| v.address)
                        .collect()
                }
            };
            println!("Known peers is {:#?}", peers);
            let node_cfg = cfg.to_node_configuration(idx, peers);
            match matches.value_of("LEVELDB_PATH") {
                Some(ref db_path) => {
                    println!("Using levedb storage with path: {}", db_path);
                    let mut options = LevelDBOptions::new();
                    options.create_if_missing = true;
                    let leveldb = LevelDB::new(&Path::new(db_path), options).unwrap();

                    let blockchain = TimestampingBlockchain { db: leveldb };
                    Node::with_config(blockchain, node_cfg).run();
                }
                None => {
                    println!("Using memorydb storage");
                    let blockchain = TimestampingBlockchain { db: MemoryDB::new() };
                    Node::with_config(blockchain, node_cfg).run();
                }
            };
        }
        _ => {
            unreachable!("Wrong subcommand");
        }
    }
}
