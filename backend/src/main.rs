#![feature(type_ascription)]

extern crate env_logger;
extern crate clap;
extern crate serde;
extern crate serde_json;
extern crate time;
extern crate rand;
#[macro_use]
extern crate log;
extern crate colored;

extern crate iron;
extern crate params;
extern crate hyper;
extern crate router;

extern crate exonum;
extern crate blockchain_explorer;
extern crate timestamping;

use std::env;
use std::io;
use std::path::Path;
use std::thread;
use std::collections::BTreeMap;

use iron::prelude::*;
use iron::status;
use iron::mime::{Mime, TopLevel, SubLevel};
use params::{Params, Value};
use router::Router;

use serde_json::value::ToJson;
use clap::{Arg, App, SubCommand};
use log::{LogRecord, LogLevel};
use env_logger::LogBuilder;
use colored::*;

use exonum::node::{Node, Configuration, TxSender, NodeChannel};
use exonum::storage::{Database, MemoryDB, LevelDB, LevelDBOptions, Map};
use exonum::storage::Error as StorageError;
use exonum::crypto::{gen_keypair, HexValue, FromHexError, Hash};
use exonum::config::ConfigFile;
use exonum::node::config::GenesisConfig;
use exonum::events::Error as EventsError;
use exonum::blockchain::Blockchain;

use timestamping::{TimestampingBlockchain, TimestampTx, Content};

pub type TimestampTxSender<B> = TxSender<B, NodeChannel<B>>;

#[derive(Clone)]
pub struct TimestampingApi<D: Database> {
    blockchain: TimestampingBlockchain<D>,
    channel: TimestampTxSender<TimestampingBlockchain<D>>,
}

#[derive(Debug)]
enum ApiError {
    Storage(StorageError),
    Events(EventsError),
    FromHex(FromHexError),
    Io(std::io::Error),
    FileNotFound(Hash),
    FileToBig,
    FileExists(Hash),
    IncorrectRequest,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for ApiError {
    fn description(&self) -> &str {
        match *self {
            ApiError::Storage(_) => "Storage",
            ApiError::Events(_) => "Events",
            ApiError::FromHex(_) => "FromHex",
            ApiError::Io(_) => "Io",
            ApiError::FileNotFound(_) => "FileNotFound",
            ApiError::FileToBig => "FileToBig",
            ApiError::FileExists(_) => "FileExists",
            ApiError::IncorrectRequest => "IncorrectRequest",
        }
    }
}

impl From<io::Error> for ApiError {
    fn from(e: io::Error) -> ApiError {
        ApiError::Io(e)
    }
}

impl From<StorageError> for ApiError {
    fn from(e: StorageError) -> ApiError {
        ApiError::Storage(e)
    }
}

impl From<EventsError> for ApiError {
    fn from(e: EventsError) -> ApiError {
        ApiError::Events(e)
    }
}

impl From<FromHexError> for ApiError {
    fn from(e: FromHexError) -> ApiError {
        ApiError::FromHex(e)
    }
}

impl From<ApiError> for IronError {
    fn from(e: ApiError) -> IronError {
        use std::error::Error;

        let mut body = BTreeMap::new();
        body.insert("type", e.description().into());
        let code = match e {
            ApiError::FileExists(hash) => {
                body.insert("hash", hash.to_hex());
                status::Conflict
            }
            ApiError::FileNotFound(hash) => {
                body.insert("hash", hash.to_hex());
                status::Conflict
            }
            _ => status::Conflict,
        };
        IronError {
            error: Box::new(e),
            response: Response::with((code, body.to_json().to_string())),
        }
    }
}

impl<D> TimestampingApi<D>
    where D: Database
{
    fn put_content(&self, hash_str: &str, description: &str) -> Result<TimestampTx, ApiError> {
        let hash = Hash::from_hex(hash_str)?;
        if self.blockchain.view().contents().get(&hash)?.is_some() {
            return Err(ApiError::FileExists(hash));
        }
        // Create transaction
        let (_, dummy_key) = gen_keypair();
        let ts = time::now().to_timespec();
        let tx = TimestampTx::new(&description, ts, &hash, &dummy_key);
        self.channel.send(tx.clone())?;
        Ok(tx)
    }

    fn get_content(&self, hash_str: &str) -> Result<Content, ApiError> {
        let hash = Hash::from_hex(hash_str)?;
        let view = self.blockchain.view();
        view.contents()
            .get(&hash)?
            .ok_or_else(|| ApiError::FileNotFound(hash))
    }
}

fn run_node<D: Database>(blockchain: TimestampingBlockchain<D>,
                         node_cfg: Configuration,
                         port: Option<u16>) {
    if let Some(port) = port {
        let mut node = Node::new(blockchain.clone(), node_cfg.clone());
        let channel = node.channel();

        let api_thread = thread::spawn(move || {
            let timestamping_api = TimestampingApi {
                channel: channel.clone(),
                blockchain: blockchain.clone(),
            };

            let mut router = Router::new();

            // Receive a message by POST and play it back.
            let api = timestamping_api.clone();
            let put_content = move |req: &mut Request| -> IronResult<Response> {
                let map = req.get_ref::<Params>().unwrap();

                fn find_str<'a>(map: &'a params::Map, path: &[&str]) -> Result<&'a str, ApiError> {
                    let value = map.find(path);
                    if let Some(&Value::String(ref s)) = value {
                        Ok(s)
                    } else {
                        Err(ApiError::IncorrectRequest)
                    }
                };

                let hash = find_str(map, &["hash"])?;
                let description = find_str(map, &["description"]).unwrap_or("");

                let tx = api.put_content(hash, description)?;
                let content_type = Mime(TopLevel::Application, SubLevel::Json, Vec::new());
                let response = Response::with((content_type, status::Ok, tx.to_json().to_string()));
                return Ok(response);
            };

            let api = timestamping_api.clone();
            let get_content = move |req: &mut Request| -> IronResult<Response> {
                let ref hash = req.extensions.get::<Router>().unwrap().find("hash").unwrap();
                let content = api.get_content(&hash)?;

                let content_type = Mime(TopLevel::Application, SubLevel::Json, Vec::new());
                let response =
                    Response::with((content_type, status::Ok, content.to_json().to_string()));
                Ok(response)
            };

            router.get("/timestamping/content/:hash", get_content, "get_content");
            router.post("/timestamping/content", put_content, "put_content");

            let host = format!("localhost:{}", port);
            Iron::new(router).http(host.as_str()).unwrap();
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
