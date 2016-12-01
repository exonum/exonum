#![feature(type_ascription)]

extern crate iron;
extern crate params;
extern crate hyper;

extern crate env_logger;
extern crate clap;
extern crate serde;
extern crate time;
extern crate rand;
#[macro_use]
extern crate log;
extern crate colored;

extern crate exonum;
extern crate blockchain_explorer;
extern crate timestamping;

use std::net::SocketAddr;
use std::path::Path;
use std::thread;
use std::default::Default;
use std::env;

use iron::prelude::*;
use iron::status;
use params::Params;

use clap::{Arg, App, SubCommand};

use log::{LogRecord, LogLevel};
use env_logger::LogBuilder;
use colored::*;

use exonum::node::{Node, Configuration, TxSender, NodeChannel};
use exonum::storage::{Database, MemoryDB, LevelDB, LevelDBOptions};
use exonum::storage::{Result as StorageResult, Error as StorageError};
use exonum::crypto::{gen_keypair, PublicKey, SecretKey, HexValue, FromHexError, hash};
use exonum::messages::Message;
use exonum::config::ConfigFile;
use exonum::node::config::GenesisConfig;
use exonum::events::Error as EventsError;

use blockchain_explorer::ValueNotFound;

use timestamping::{TimestampingBlockchain, TimestampingView, TimestampTx, TIMESTAMPING_FILE_SIZE_LIMIT};

pub type TimestampTxSender<B> = TxSender<B, NodeChannel<B>>;

pub struct TimestampingApi<D: Database> {
    blockchain: TimestampingBlockchain<D>,
    channel: TimestampTxSender<TimestampingBlockchain<D>>
}

impl<D> TimestampingApi<D>
    where D: Database
{
    // TODO replace by error
    fn store_file(&self, params: &Params) -> Option<String> {
        // println!("{:?}", params);  
        None
    }
}


fn run_node<D: Database>(blockchain: TimestampingBlockchain<D>,
                         node_cfg: Configuration,
                         port: Option<u16>) {
    if let Some(port) = port {
        let mut node = Node::new(blockchain.clone(), node_cfg.clone());
        let channel = node.channel();

        let api_thread = thread::spawn(move || {
            let api = TimestampingApi {
                channel: channel.clone(),
                blockchain: blockchain.clone()
            };

            let put_file = Chain::new(|req: &mut Request| -> IronResult<Response> {
                println!("{:?}", req.get_ref::<Params>());
                let hash = hash(&[]).to_hex();
                let r = Response::with((status::Ok, hash));
                Ok(r)

                // let response = if let Some(hash) = {
                //     Response::with(api.store_file(req.get_ref::<Params>()))
                // } else {
                //     Response::with(status::InternalServerError)
                // };
                // Ok(response)
            });

            let host = format!("localhost:{}", port);
            Iron::new(put_file).http(host.as_str()).unwrap();
        });

        node.run().unwrap();
        api_thread.join().unwrap();
    } else {
        Node::new(blockchain, node_cfg).run().unwrap();
    }
}

fn main() {
    let format = |record: &LogRecord| {
        let now = time::now_utc();
        let level = match record.level() {
            LogLevel::Error => "ERROR".red(),
            LogLevel::Warn => "WARN".yellow(),
            LogLevel::Info => "INFO".green(),
            LogLevel::Debug => "DEBUG".cyan(),
            LogLevel::Trace => "TRACE".white(),
        };
        format!("{} - [ {} ] - {}",
                now.asctime().to_string().bold(),
                level,
                record.args())
    };

    let mut builder = LogBuilder::new();
    builder.format(format);

    if env::var("RUST_LOG").is_ok() {
        builder.parse(&env::var("RUST_LOG").unwrap());
    }

    builder.init().unwrap();

    let app = App::new("Simple timestamping demo program")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Aleksey S. <aleksei.sidorov@xdev.re>")
        .about("Demo timestamping validator node")
        .arg(Arg::with_name("CONFIG")
            .short("c")
            .long("config")
            .value_name("CONFIG_PATH")
            .help("Sets a node config file")
            .required(true)
            .takes_value(true))
        .subcommand(SubCommand::with_name("generate")
            .about("Generates default configuration file")
            .version(env!("CARGO_PKG_VERSION"))
            .author("Aleksey S. <aleksei.sidorov@xdev.re>")
            .arg(Arg::with_name("START_PORT")
                .short("p")
                .long("port")
                .value_name("START_PORT")
                .help("Port for first validator")
                .takes_value(true))
            .arg(Arg::with_name("COUNT")
                .help("Validators count")
                .required(true)
                .index(1)))
        .subcommand(SubCommand::with_name("run")
            .about("Run demo node with the given validator id")
            .version(env!("CARGO_PKG_VERSION"))
            .author("Aleksey S. <aleksei.sidorov@xdev.re>")
            .arg(Arg::with_name("LEVELDB_PATH")
                .short("d")
                .long("leveldb-path")
                .value_name("LEVELDB_PATH")
                .help("Use leveldb database with the given path")
                .takes_value(true))
            .arg(Arg::with_name("HTTP_PORT")
                .short("p")
                .long("port")
                .value_name("HTTP_PORT")
                .help("Run http server on given port")
                .takes_value(true))
            .arg(Arg::with_name("PEERS")
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
            let port: Option<u16> = matches.value_of("START_PORT").map(|x| x.parse().unwrap());
            let cfg = GenesisConfig::gen(count, port);
            ConfigFile::save(&cfg, &path).unwrap();
            println!("The configuration was successfully written to file {:?}",
                     path);
        }
        ("run", Some(matches)) => {
            let cfg: GenesisConfig = ConfigFile::load(path).unwrap();
            let idx: usize = matches.value_of("VALIDATOR").unwrap().parse().unwrap();
            let port: Option<u16> = matches.value_of("HTTP_PORT").map(|x| x.parse().unwrap());
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
            let node_cfg = cfg.gen_node_configuration(idx, peers);
            match matches.value_of("LEVELDB_PATH") {
                Some(ref db_path) => {
                    println!("Using levedb storage with path: {}", db_path);
                    let mut options = LevelDBOptions::new();
                    options.create_if_missing = true;
                    let leveldb = LevelDB::new(&Path::new(db_path), options).unwrap();

                    let blockchain = TimestampingBlockchain { db: leveldb };
                    run_node(blockchain, node_cfg, port);
                }
                None => {
                    println!("Using memorydb storage");

                    let blockchain = TimestampingBlockchain { db: MemoryDB::new() };
                    run_node(blockchain, node_cfg, port);
                }
            };
        }
        _ => {
            unreachable!("Wrong subcommand");
        }
    }
}
