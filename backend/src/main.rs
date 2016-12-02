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
use std::fs::File;
use std::io;
use std::io::Read;
use std::path::Path;
use std::thread;

use iron::prelude::*;
use iron::status;
use iron::mime::{Mime, TopLevel, SubLevel};
use iron::headers;
use hyper::header::{ContentDisposition, DispositionType, DispositionParam, Charset};
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
use exonum::crypto::{gen_keypair, HexValue, FromHexError, hash, Hash};
use exonum::config::ConfigFile;
use exonum::node::config::GenesisConfig;
use exonum::events::Error as EventsError;
use exonum::blockchain::Blockchain;

use timestamping::{TimestampingBlockchain, TimestampTx, Content, TIMESTAMPING_FILE_SIZE_LIMIT};

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
    FileNotFound,
    FileToBig,
    FileExists,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for ApiError {
    fn description(&self) -> &str {
        "Api error"
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
        let code = status::BadRequest;
        let string = e.to_string();

        IronError {
            error: Box::new(e),
            response: Response::with((code, string)),
        }
    }
}

impl<D> TimestampingApi<D>
    where D: Database
{
    fn put_file(&self, file: &params::File, description: &str) -> Result<TimestampTx, ApiError> {
        if file.size > TIMESTAMPING_FILE_SIZE_LIMIT {
            return Err(ApiError::FileToBig);
        }

        let file_name = file.filename.clone().unwrap_or("None".into());
        let mime = file.content_type.to_string();
        let mut file = File::open(&file.path)?;

        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;
        let hash = hash(buf.as_ref());
        // TODO add checks for already stored files
        if self.blockchain.view().contents().get(&hash)?.is_some() {
            return Err(ApiError::FileExists);
        }

        // Create transaction
        let (_, dummy_key) = gen_keypair();
        let ts = time::now().to_timespec();
        // FIXME avoid reallocations
        let tx = TimestampTx::new(&file_name, &description, &mime, ts, &hash, buf.as_ref(), &dummy_key);
        self.channel.send(tx.clone())?;

        Ok(tx)
    }

    fn get_file(&self, hash_str: &str) -> Result<Content, ApiError> {
        let hash = Hash::from_hex(hash_str)?;
        let view = self.blockchain.view();
        view.contents()
            .get(&hash)?
            .ok_or_else(|| ApiError::FileNotFound)
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
            let api = timestamping_api.clone();
            let get_file = move |req: &mut Request| -> IronResult<Response> {
                let ref hash = req.extensions.get::<Router>().unwrap().find("hash").unwrap();
                let content = api.get_file(&hash)?;

                let mime: Mime = content.mime().parse().unwrap();
                let data = content.data().to_vec();
                let len = data.len() as u64;

                let mut response = Response::with((status::Ok, data));
                response.set_mut(mime);
                response.headers.set(headers::ContentLength(len));
                response.headers.set(ContentDisposition {
                    disposition: DispositionType::Attachment,
                    parameters: vec![DispositionParam::Filename(
                    Charset::Iso_8859_1, // The character set for the bytes of the filename
                    None, // The optional language tag (see `language-tag` crate)
                    content.file_name().as_bytes().to_vec() // the actual bytes of the filename
                    )]
                });
                Ok(response)
            };

            // Receive a message by POST and play it back.
            let api = timestamping_api.clone();
            let put_file = move |req: &mut Request| -> IronResult<Response> {
                let map = req.get_ref::<Params>().unwrap();

                let description = if let Some(&Value::String(ref txt)) = map.find(&["description"]) {
                    txt
                } else {
                    ""
                };

                if let Some(&Value::File(ref file)) = map.find(&["content"]) {
                    let tx = api.put_file(file, description)?;
                    let content_type = Mime(TopLevel::Application, SubLevel::Json, Vec::new());
                    let response = Response::with((content_type, status::Ok, tx.to_json().to_string()));
                    return Ok(response);
                } else {
                    Err(ApiError::FileNotFound.into())
                }
            };

            router.get("/timestamping/:hash", get_file, "hash");
            router.post("/timestamping", put_file, "put");

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
