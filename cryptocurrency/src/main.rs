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
extern crate env_logger;
extern crate clap;
extern crate serde;
extern crate time;
extern crate base64;
extern crate rand;

extern crate exonum;
extern crate cryptocurrency;

use std::net::SocketAddr;
use std::path::Path;
use std::thread;
use std::cmp::min;

use clap::{Arg, App, SubCommand};
use rustless::json::ToJson;
use rustless::{Application, Api, Nesting, Versioning};
use rustless::batteries::cookie::{Cookie, CookieExt};
use valico::json_dsl;
use hyper::status::StatusCode;
use serde::{Serialize, Serializer};
use rand::{Rng, thread_rng};

use exonum::node::{Node, NodeChannel, Configuration};
use exonum::storage::{Database, MemoryDB, LevelDB, LevelDBOptions, List, Map, Error};
use exonum::blockchain::{Block, Blockchain, View};
use exonum::crypto::{Hash, gen_keypair, PublicKey, SecretKey};

use cryptocurrency::config_file::ConfigFile;
use cryptocurrency::config::NodeConfig;
use cryptocurrency::{CurrencyBlockchain, CurrencyTx, CurrencyView, TxIssue, TxTransfer};

pub trait BlockchainExplorer<D: Database> {
    type BlockInfo: Serialize;
    type TxInfo: Serialize;

    fn blocks_range(&self, from: u64, to: Option<u64>) -> Result<Vec<Self::BlockInfo>, Error>;
    fn get_tx_info(&self, hash: &Hash) -> Result<Option<Self::TxInfo>, Error>;
    fn get_tx_hashes_from_block(&self, height: u64) -> Result<Vec<Hash>, Error>;

    fn get_block_info(&self, height: u64) -> Result<Option<Self::BlockInfo>, Error> {
        let range = self.blocks_range(height, Some(height + 1))?;
        Ok(range.into_iter().next())
    }
    fn get_txs<H: AsRef<[Hash]>>(&self, hashes: H) -> Result<Vec<Self::TxInfo>, Error> {
        let mut v = Vec::new();
        for h in hashes.as_ref() {
            if let Some(tx_info) = self.get_tx_info(h)? {
                v.push(tx_info)
            }
        }
        Ok(v)
    }
    fn get_txs_for_block(&self, height: u64) -> Result<Vec<Self::TxInfo>, Error> {
        let hashes = self.get_tx_hashes_from_block(height)?;
        self.get_txs(&hashes)
    }
}

pub struct BlockInfo {
    inner: Block,
    tx_count: u32,
}

pub struct TxInfo {
    inner: CurrencyTx,
}

pub trait Base64Value: Sized {
    fn to_base64(&self) -> String;
    fn from_base64<T: AsRef<str>>(v: T) -> Result<Self, base64::Base64Error>;
}

impl Base64Value for Hash {
    fn to_base64(&self) -> String {
        base64::encode(self.as_ref())
    }
    fn from_base64<T: AsRef<str>>(v: T) -> Result<Self, base64::Base64Error> {
        let bytes = base64::decode(v.as_ref())?;
        if let Some(hash) = Hash::from_slice(bytes.as_ref()) {
            Ok(hash)
        } else {
            Err(base64::Base64Error::InvalidLength)
        }
    }
}

pub struct Base64<T>(T) where T: AsRef<[u8]>;

impl<T> Serialize for Base64<T>
    where T: AsRef<[u8]>
{
    fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
        where S: Serializer
    {
        serializer.serialize_str(&base64::encode(self.0.as_ref()))
    }
}

impl Serialize for BlockInfo {
    fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
        where S: Serializer
    {
        let b = &self.inner;
        // TODO think about timespec serialize
        let tm = ::time::at(b.time()).rfc3339().to_string();
        let mut state = serializer.serialize_struct("block", 7)?;
        serializer.serialize_struct_elt(&mut state, "height", b.height())?;

        serializer.serialize_struct_elt(&mut state, "hash", Base64(b.hash()))?;
        serializer.serialize_struct_elt(&mut state, "prev_hash", Base64(b.prev_hash()))?;
        serializer.serialize_struct_elt(&mut state, "state_hash", Base64(b.state_hash()))?;
        serializer.serialize_struct_elt(&mut state, "tx_hash", Base64(b.tx_hash()))?;

        serializer.serialize_struct_elt(&mut state, "time", tm)?;
        serializer.serialize_struct_elt(&mut state, "tx_count", self.tx_count)?;
        serializer.serialize_struct_end(state)
    }
}

impl Serialize for TxInfo {
    fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
        where S: Serializer
    {
        let tx = &self.inner;
        let mut state;
        match *tx {
            CurrencyTx::Issue(ref issue) => {
                state = serializer.serialize_struct("transaction", 4)?;
                serializer.serialize_struct_elt(&mut state, "type", "issue")?;
                serializer.serialize_struct_elt(&mut state, "wallet", Base64(issue.wallet()))?;
                serializer.serialize_struct_elt(&mut state, "amount", issue.amount())?;
                serializer.serialize_struct_elt(&mut state, "seed", issue.seed())?;
            }
            CurrencyTx::Transfer(ref transfer) => {
                state = serializer.serialize_struct("transaction", 5)?;
                serializer.serialize_struct_elt(&mut state, "type", "transfer")?;
                serializer.serialize_struct_elt(&mut state, "from", Base64(transfer.from()))?;
                serializer.serialize_struct_elt(&mut state, "to", Base64(transfer.to()))?;
                serializer.serialize_struct_elt(&mut state, "amount", transfer.amount())?;
                serializer.serialize_struct_elt(&mut state, "seed", transfer.seed())?;
            }
        }
        serializer.serialize_struct_end(state)
    }
}

impl<D> BlockchainExplorer<D> for CurrencyView<D::Fork>
    where D: Database
{
    type BlockInfo = BlockInfo;
    type TxInfo = TxInfo;

    fn blocks_range(&self, from: u64, to: Option<u64>) -> Result<Vec<Self::BlockInfo>, Error> {
        let heights = self.heights();
        let blocks = self.blocks();

        let max_len = heights.len()?;
        let len = min(max_len, to.unwrap_or(max_len));

        let mut v = Vec::new();
        for i in from..len {
            if let Some(ref h) = heights.get(i)? {
                let tx_count = self.block_txs(i).len()?;
                if let Some(block) = blocks.get(h)? {
                    v.push(BlockInfo {
                        inner: block,
                        tx_count: tx_count,
                    });
                }
            }
        }
        Ok(v)
    }

    fn get_tx_info(&self, hash: &Hash) -> Result<Option<Self::TxInfo>, Error> {
        let tx = self.transactions().get(hash)?;
        Ok(tx.map(|tx| TxInfo { inner: tx }))
    }

    fn get_tx_hashes_from_block(&self, height: u64) -> Result<Vec<Hash>, Error> {
        let txs = self.block_txs(height);
        let tx_count = txs.len()?;
        let mut v = Vec::new();
        for i in 0..tx_count {
            if let Some(tx_hash) = txs.get(i)? {
                v.push(tx_hash);
            }
        }
        Ok(v)
    }
}

fn run_node<D: Database>(blockchain: CurrencyBlockchain<D>,
                         node_cfg: Configuration,
                         port: Option<u16>) {
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
                api.namespace("blockchain", move |api| {
                    api.get("block", |endpoint| {
                        let blockchain = blockchain.clone();

                        endpoint.summary("Returns block chain");
                        endpoint.params(|params| {
                            params.opt_typed("from", json_dsl::u64());
                            params.opt_typed("to", json_dsl::u64())
                        });

                        endpoint.handle(move |client, params| {
                            println!("{:?}", params);
                            let view: CurrencyView<D::Fork> = blockchain.clone().view();
                            let from =
                                params.find("from").map(|x| x.as_u64().unwrap()).unwrap_or(0);
                            let to = params.find("to").map(|x| x.as_u64().unwrap());

                            match BlockchainExplorer::<D>::blocks_range(&view, from, to) {
                                Ok(blocks) => client.json(&blocks.to_json()),
                                Err(e) => client.error(e),
                            }
                        })
                    });
                    api.get("block/:height", |endpoint| {
                        let blockchain = blockchain.clone();

                        endpoint.summary("Returns block with given height");
                        endpoint.params(|params| {
                            params.req_typed("height", json_dsl::u64());
                        });

                        endpoint.handle(move |client, params| {
                            println!("{:?}", params);
                            let view = blockchain.clone().view();
                            let height = params.find("height").unwrap().as_u64().unwrap();

                            match BlockchainExplorer::<D>::get_block_info(&view, height) {
                                Ok(Some(blocks)) => client.json(&blocks.to_json()),
                                Ok(None) => Ok(client),
                                Err(e) => client.error(e),
                            }
                        })
                    });
                    api.get("transactions/:height", |endpoint| {
                        let blockchain = blockchain.clone();

                        endpoint.summary("Returns transactions for block with given height");
                        endpoint.params(|params| {
                            params.req_typed("height", json_dsl::u64());
                        });

                        endpoint.handle(move |client, params| {
                            println!("{:?}", params);
                            let view = blockchain.clone().view();
                            let height = params.find("height").unwrap().as_u64().unwrap();

                            match BlockchainExplorer::<D>::get_txs_for_block(&view, height) {
                                Ok(txs) => client.json(&txs.to_json()),
                                Err(e) => client.error(e),
                            }
                        })
                    });
                    api.get("transaction/:hash", |endpoint| {
                        let blockchain = blockchain.clone();

                        endpoint.summary("Returns transaction info");
                        endpoint.params(|params| {
                            params.req_typed("hash", json_dsl::string());
                        });

                        endpoint.handle(move |client, params| {
                            println!("{:?}", params);
                            let view = blockchain.clone().view();
                            let hash = params.find("hash").unwrap().to_string();
                            match Hash::from_base64(hash) {
                                Ok(hash) => {
                                    match BlockchainExplorer::<D>::get_tx_info(&view, &hash) {
                                        Ok(tx_info) => client.json(&tx_info.to_json()),
                                        Err(e) => client.error(e),
                                    }
                                }
                                Err(_) => {
                                    client.error(Error::new("Unable to decode transaction hash"))
                                }
                            }
                        })
                    });
                });

                // Cryptocurrency api
                api.namespace("wallets", move |api| {
                    api.post("create", move |endpoint| {
                        endpoint.summary("Create a new wallet for user with given name");
                        endpoint.params(|params| {
                            params.req_typed("name", json_dsl::string());
                        });

                        endpoint.handle(move |client, params| {
                            println!("{:?}", params);
                            let name = params.find("name").unwrap().to_string();
                            // TODO make secure
                            let (public_key, secret_key) = gen_keypair();
                            {
                                let cookies = client.request.cookies();
                                let p = cookies.permanent();
                                let c = p.encrypted();
                                c.add(Cookie::new("public_key".to_string(),
                                                  base64::encode(public_key.as_ref())));
                                c.add(Cookie::new("secret_key".to_string(),
                                                  base64::encode(secret_key.0.as_ref())));
                            }
                            Ok(client)
                        })
                    });

                    let channel = channel.clone();
                    api.post("issue", move |endpoint| {
                        endpoint.params(|params| {
                            params.req_typed("amount", json_dsl::i64());
                        });

                        endpoint.handle(move |client, params| {
                            let channel = channel.clone();

                            let mut public_key = None;
                            let mut secret_key = None;
                            {
                                let cookies = client.request.cookies();
                                let p = cookies.permanent();
                                let c = p.encrypted();
                                if let Some(cookie) = c.find("public_key") {
                                    match base64::decode(&cookie.value) {
                                        Ok(data) => {
                                            public_key = PublicKey::from_slice(data.as_ref());
                                        }
                                        Err(_) => {}
                                    }
                                }
                                if let Some(cookie) = c.find("secret_key") {
                                    match base64::decode(&cookie.value) {
                                        Ok(data) => {
                                            secret_key = SecretKey::from_slice(data.as_ref());
                                        }
                                        Err(_) => {}
                                    }
                                }
                            }

                            match (public_key, secret_key) {
                                (Some(public_key), Some(secret_key)) => {
                                    let amount = params.find("amount").unwrap().as_i64().unwrap();
                                    let seed = thread_rng().gen::<u64>();
                                    let tx = TxIssue::new(&public_key, amount, seed, &secret_key);
                                    channel.send(CurrencyTx::Issue(tx));
                                    println!("Issue with amount: {}", amount);
                                    Ok(client)
                                }
                                _ => client.error(Error::new("Unable to read keypair")),
                            }
                        })
                    })
                });
            });

            let listen_address: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
            println!("Cryptocurrency node server started on {}", listen_address);

            let app = Application::new(api);
            let mut chain = iron::Chain::new(app);
            chain.link(::rustless::batteries::cookie::new("secretsecretsecretsecretsecretsecretsecret".as_bytes()));
            iron::Iron::new(chain).http(listen_address).unwrap();
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
