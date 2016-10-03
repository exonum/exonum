#![feature(type_ascription)]
#![feature(question_mark)]
#![feature(custom_derive)]
#![feature(plugin)]
#![plugin(serde_macros)]

#[macro_use]
extern crate rustless;
extern crate jsonway;
extern crate iron;
extern crate hyper;
extern crate valico;
extern crate env_logger;
extern crate clap;
extern crate serde;
extern crate time;
extern crate rand;

extern crate exonum;
extern crate utils;
extern crate cryptocurrency;

use std::net::SocketAddr;
use std::path::Path;
use std::thread;
use std::default::Default;

use clap::{Arg, App, SubCommand};
use rustless::json::ToJson;
use rustless::{Application, Api, Nesting, Versioning, Response};
use rustless::batteries::cookie::{Cookie, CookieExt, CookieJar};
use rustless::batteries::swagger;
use valico::json_dsl;
use hyper::status::StatusCode;
use rand::{Rng, thread_rng};

use exonum::node::{Node, Configuration, TxSender, NodeChannel};
use exonum::storage::{Database, MemoryDB, LevelDB, LevelDBOptions};
use exonum::storage::{Result as StorageResult, Error as StorageError};
use exonum::crypto::{gen_keypair, PublicKey, SecretKey};
use exonum::messages::Message;

use utils::config_file::ConfigFile;
use utils::config::NodeConfig;
use utils::HexValue;
use utils::blockchain_explorer;

use cryptocurrency::{CurrencyBlockchain, CurrencyTx, TxIssue, TxTransfer,
                     TxCreateWallet};
use cryptocurrency::api::CurrencyApi;

pub type CurrencyTxSender<B> = TxSender<B, NodeChannel<B>>;


fn save_keypair_in_cookies(storage: &mut CookieJar,
                           public_key: &PublicKey,
                           secret_key: &SecretKey) {
    let p = storage.permanent();
    let e = p.encrypted();

    e.add(Cookie::new("public_key".to_string(), public_key.to_hex()));
    e.add(Cookie::new("secret_key".to_string(), secret_key.to_hex()));
}

fn load_hex_value_from_cookie<'a>(storage: &'a CookieJar, key: &str) -> StorageResult<Vec<u8>> {
    if let Some(cookie) = storage.find(key) {
        if let Ok(value) = HexValue::from_hex(cookie.value) {
            return Ok(value);
        }
    }
    Err(StorageError::new(format!("Unable to find value with given key {}", key)))
}

fn load_keypair_from_cookies(storage: &CookieJar) -> StorageResult<(PublicKey, SecretKey)> {
    let p = storage.permanent();
    let e = p.encrypted();

    let public_key = PublicKey::from_slice(load_hex_value_from_cookie(&e, "public_key")?.as_ref());
    let secret_key = SecretKey::from_slice(load_hex_value_from_cookie(&e, "secret_key")?.as_ref());

    let public_key = public_key.ok_or(StorageError::new("Unable to read public key"))?;
    let secret_key = secret_key.ok_or(StorageError::new("Unable to read secret key"))?;
    Ok((public_key, secret_key))
}

fn blockchain_explorer_api<D: Database>(api: &mut Api, b1: CurrencyBlockchain<D>) {
    blockchain_explorer::make_api::<CurrencyBlockchain<D>, CurrencyTx>(api, b1);
}

fn cryptocurrency_api<D: Database>(api: &mut Api,
                                   blockchain: CurrencyBlockchain<D>,
                                   channel: CurrencyTxSender<CurrencyBlockchain<D>>) {
    api.namespace("wallets", move |api| {
        let ch = channel.clone();
        api.post("create", move |endpoint| {
            endpoint.summary("Create a new wallet for user with given name");
            endpoint.params(|params| {
                params.req_typed("name", json_dsl::string());
            });

            endpoint.handle(move |client, params| {
                let name = params.find("name").unwrap().as_str().unwrap();
                // TODO make secure
                let (public_key, secret_key) = gen_keypair();
                {
                    let mut cookies = client.request.cookies();
                    save_keypair_in_cookies(&mut cookies, &public_key, &secret_key);
                }
                let tx = TxCreateWallet::new(&public_key, &name, &secret_key);

                let tx_hash = tx.hash().to_hex();
                ch.send(CurrencyTx::CreateWallet(tx));
                let json = &jsonway::object(|json| json.set("tx_hash", tx_hash)).unwrap();
                client.json(json)
            })
        });

        let ch = channel.clone();
        api.post("issue", move |endpoint| {
            endpoint.params(|params| {
                params.req_typed("amount", json_dsl::i64());
            });

            endpoint.handle(move |client, params| {
                let (public_key, secret_key) = {
                    let r = {
                        let cookies = client.request.cookies();
                        load_keypair_from_cookies(&cookies)
                    };
                    match r {
                        Ok((p, s)) => (p, s),
                        Err(e) => return client.error(e),
                    }
                };

                let amount = params.find("amount").unwrap().as_i64().unwrap();
                let seed = thread_rng().gen::<u64>();
                let tx = TxIssue::new(&public_key, amount, seed, &secret_key);

                let tx_hash = tx.hash().to_hex();
                ch.send(CurrencyTx::Issue(tx));
                let json = &jsonway::object(|json| json.set("tx_hash", tx_hash)).unwrap();
                client.json(json)
            })
        });

        let ch = channel.clone();
        api.post("transfer", move |endpoint| {
            endpoint.params(|params| {
                params.req_typed("amount", json_dsl::i64());
                params.req_typed("to", json_dsl::string());
            });

            endpoint.handle(move |client, params| {
                let (from_key, secret_key) = {
                    let r = {
                        let cookies = client.request.cookies();
                        load_keypair_from_cookies(&cookies)
                    };
                    match r {
                        Ok((p, s)) => (p, s),
                        Err(e) => return client.error(e),
                    }
                };
                // TODO remove unwrap
                let amount = params.find("amount").unwrap().as_i64().unwrap();
                let to = params.find("to").unwrap().as_str().unwrap();
                let seed = thread_rng().gen::<u64>();
                match PublicKey::from_hex(to) {
                    Ok(to_key) => {
                        let tx = TxTransfer::new(&from_key, &to_key, amount, seed, &secret_key);

                        let tx_hash = tx.hash().to_hex();
                        ch.send(CurrencyTx::Transfer(tx));
                        let json = &jsonway::object(|json| json.set("tx_hash", tx_hash)).unwrap();
                        client.json(json)
                    }
                    Err(e) => return client.error(e),
                }
            })
        });

        let b = blockchain.clone();
        api.get("info", move |endpoint| {
            endpoint.handle(move |client, _| {
                let (public_key, _) = {
                    let r = {
                        let cookies = client.request.cookies();
                        load_keypair_from_cookies(&cookies)
                    };
                    match r {
                        Ok((p, s)) => (p, s),
                        Err(e) => return client.error(e),
                    }
                };
                let currency_api = CurrencyApi::<D>::new(b.clone());
                match currency_api.wallet_info(&public_key) {
                    Ok(Some(info)) => client.json(&info.to_json()),
                    _ => client.error(StorageError::new("Unable to get wallet info")),
                }
            })
        })
    });
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

                api.error_formatter(|err, _media| {
                    if let Some(e) = err.downcast::<StorageError>() {
                        let body = format!("An internal error occured: {}", e);
                        Some(Response::from(StatusCode::InternalServerError, Box::new(body)))
                    } else {
                        None
                    }
                });

                blockchain_explorer_api(api, blockchain.clone());
                cryptocurrency_api(api, blockchain.clone(), channel.clone());
                api.mount(swagger::create_api("docs"));
            });

            let listen_address: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
            println!("Cryptocurrency node server started on {}", listen_address);

            let mut app = Application::new(api);

            swagger::enable(&mut app,
                            swagger::Spec {
                                info: swagger::Info {
                                    title: "Cryptocurrency API".to_string(),
                                    description: Some("Simple API to demonstration".to_string()),
                                    contact: Some(swagger::Contact {
                                        name: "Aleksey Sidorov".to_string(),
                                        url: Some("aleksei.sidorov@xdev.re".to_string()),
                                        ..Default::default()
                                    }),
                                    license: Some(swagger::License {
                                        name: "Demo".to_string(),
                                        url: "http://exonum.com".to_string(),
                                    }),
                                    ..Default::default()
                                },
                                ..Default::default()
                            });

            let mut chain = iron::Chain::new(app);
            let api_key = b"abacabasecretcoinblabla23nx8Hasojd8";
            let cookie = ::rustless::batteries::cookie::new(api_key);
            chain.link(cookie);
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
