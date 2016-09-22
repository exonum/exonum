#![feature(type_ascription)]
#![feature(question_mark)]
#![feature(custom_derive)]
#![feature(plugin)]
#![plugin(serde_macros)]

#[macro_use]
extern crate rustless;
extern crate iron;
extern crate hyper;
extern crate valico;
extern crate exonum;
extern crate cryptocurrency;

extern crate env_logger;
extern crate clap;
extern crate serde;

use std::net::SocketAddr;
use std::path::Path;
use std::thread;
use std::cmp::min;

use clap::{Arg, App, SubCommand};
use rustless::json::ToJson;
use rustless::{Application, Api, Nesting, Versioning};
use valico::json_dsl;
use hyper::status::StatusCode;

use exonum::node::{Node, NodeChannel, Configuration};
use exonum::storage::{Database, MemoryDB, LevelDB, LevelDBOptions,
                      List, Map, Error};
use exonum::blockchain::{Block, Blockchain, View};

use cryptocurrency::config_file::{ConfigFile};
use cryptocurrency::config::{NodeConfig};
use cryptocurrency::CurrencyBlockchain;

pub trait BlockchainExplorer {
    fn blocks_range(&self, from: u64, to: Option<u64>) -> Result<Vec<Block>, Error>;
    fn get_block(&self, idx: u64) -> Result<Option<Block>, Error> {
        let range = self.blocks_range(idx, Some(idx + 1))?;
        Ok(range.into_iter().next())
    }
}

impl<D: Database> BlockchainExplorer for CurrencyBlockchain<D> {
    fn blocks_range(&self, from: u64, to: Option<u64>) -> Result<Vec<Block>, Error> {
        let v = self.view();
        let heights = v.heights();
        let blocks = v.blocks();

        let max_len = heights.len()?;
        let len = min(max_len, to.unwrap_or(max_len));

        let mut v = Vec::new();
        for i in from..len {
            if let Some(ref h) = heights.get(i)? {
                if let Some(block) = blocks.get(h)? {
                    v.push(block);
                }
            }
        }
        Ok(v)
    }
}

fn get_blocks<D: Database>(b: &CurrencyBlockchain<D>, from: u64, to: Option<u64>) -> Result<Vec<Block>, Error> {
    let v = b.view();

    let heights = v.heights();
    let blocks = v.blocks();

    let max_len = heights.len()?;
    let len = min(max_len, to.unwrap_or(max_len));

    let mut v = Vec::new();
    for i in from..len {
        if let Some(ref h) = heights.get(i)? {
            if let Some(block) = blocks.get(h)? {
                v.push(block);
            }
        }
    }
    Ok(v)
}

fn run_node<D: Database>(blockchain: CurrencyBlockchain<D>, node_cfg: Configuration, port: Option<u16>) {
    if let Some(port) = port {
        let mut node = Node::new(blockchain.clone(), node_cfg);
        let channel = node.channel();

        let api_thread = thread::spawn(move || {
            let channel = channel.clone();
            let blockchain = blockchain.clone();

            let api = Api::build(move |api| {
                // Specify API version
                api.version("v1", Versioning::Path);
                api.prefix("api");

                // Blockchain explorer api
                api.get("blockchain", |endpoint| {
                    let blockchain = blockchain.clone();

                    endpoint.summary("Returns block chain");
                    endpoint.params(|params| {
                        params.opt_typed("from", json_dsl::u64());
                        params.opt_typed("to", json_dsl::u64())
                    });

                    endpoint.handle(move |client, params| {
                        println!("{:?}", params);
                        let blockchain = blockchain.clone();
                        let from = params.find("from").map(|x| x.as_u64().unwrap()).unwrap_or(0);
                        let to = params.find("to").map(|x| x.as_u64().unwrap());

                        match blockchain.blocks_range(from, to) {
                            Ok(blocks) => client.json(&blocks.to_json()),
                            // TODO add error trait to blockchain errors
                            Err(e) => Ok(client)
                        }
                    })
                });
                api.get("blockchain/:height", |endpoint| {
                    let blockchain = blockchain.clone();

                    endpoint.summary("Returns block with given height");
                    endpoint.params(|params| {
                        params.req_typed("height", json_dsl::u64());
                    });

                    endpoint.handle(move |client, params| {
                        println!("{:?}", params);
                        let blockchain = blockchain.clone();
                        let height = params.find("height").unwrap().as_u64().unwrap();

                        match blockchain.get_block(height) {
                            Ok(Some(blocks)) => client.json(&blocks.to_json()),
                            Ok(None) => Ok(client),
                            // TODO add error trait to blockchain errors
                            Err(e) => Ok(client)
                        }
                    })
                });
            });

            let listen_address: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
            println!("Cryptocurrency node server started on {}!", listen_address);
            let app = Application::new(api);
            iron::Iron::new(app).http(listen_address).unwrap();
        });

        node.run().unwrap();
        api_thread.join().unwrap();
    } else {
        Node::new(blockchain, node_cfg).run().unwrap();
    }
}

fn main() {
    env_logger::init().unwrap();

    let app = App::new("Simple cryptocurrency demo program")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Aleksey S. <aleksei.sidorov@xdev.re>")
        .about("Demo cryptocurrency validator node")
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
                .short("h")
                .long("http_port")
                .value_name("HTTP_PORT")
                .help("Run http server on given port")
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
            let cfg = NodeConfig::gen(count);
            ConfigFile::save(&cfg, &path).unwrap();
            println!("The configuration was successfully written to file {:?}",
                     path);
        }
        ("run", Some(matches)) => {
            let cfg: NodeConfig = ConfigFile::load(path).unwrap();
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
            println!("Known peers is {:#?}", peers);
            let node_cfg = cfg.to_node_configuration(idx, peers);
            match matches.value_of("LEVELDB_PATH") {
                Some(ref db_path) => {
                    println!("Using levedb storage with path: {}", db_path);
                    let mut options = LevelDBOptions::new();
                    options.create_if_missing = true;
                    let leveldb = LevelDB::new(&Path::new(db_path), options).unwrap();

                    let blockchain = CurrencyBlockchain { db: leveldb };
                    run_node(blockchain, node_cfg, port);
                }
                None => {
                    println!("Using memorydb storage");

                    let blockchain = CurrencyBlockchain { db: MemoryDB::new() };
                    run_node(blockchain, node_cfg, port);
                }
            };
        }
        _ => {
            unreachable!("Wrong subcommand");
        }
    }
}
