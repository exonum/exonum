#![feature(type_ascription)]

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
#[macro_use]
extern crate log;

extern crate exonum;
extern crate blockchain_explorer;
extern crate cryptocurrency;

use std::net::SocketAddr;
use std::thread;
use std::default::Default;

use clap::{Arg, App};
use rustless::json::ToJson;
use rustless::{Application, Api, Nesting, Versioning, Response};
use rustless::batteries::cookie::{Cookie, CookieExt, CookieJar};
use rustless::batteries::swagger;
use rustless::errors;
use valico::json_dsl;
use hyper::status::StatusCode;
use rand::{Rng, thread_rng};

use exonum::blockchain::{GenesisConfig, Blockchain};
use exonum::node::{Node, NodeConfig, TxSender, NodeChannel};
use exonum::storage::{Result as StorageResult, Error as StorageError};
use exonum::crypto::{gen_keypair, PublicKey, SecretKey, HexValue, FromHexError};
use exonum::messages::Message;
use exonum::events::Error as EventsError;

use blockchain_explorer::ValueNotFound;
use blockchain_explorer::helpers::{GenerateCommand, RunCommand};

use cryptocurrency::{CurrencyService, CurrencyTx, TxIssue, TxTransfer, TxCreateWallet};
use cryptocurrency::api::CurrencyApi;

pub type CurrencyTxSender = TxSender<NodeChannel>;

fn save_keypair_in_cookies(storage: &mut CookieJar,
                           public_key: &PublicKey,
                           secret_key: &SecretKey) {
    let p = storage.permanent();

    p.add(Cookie::new("public_key".to_string(), public_key.to_hex()));
    p.add(Cookie::new("secret_key".to_string(), secret_key.to_hex()));
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

    let public_key = PublicKey::from_slice(load_hex_value_from_cookie(&p, "public_key")?.as_ref());
    let secret_key = SecretKey::from_slice(load_hex_value_from_cookie(&p, "secret_key")?.as_ref());

    let public_key = public_key.ok_or(StorageError::new("Unable to read public key"))?;
    let secret_key = secret_key.ok_or(StorageError::new("Unable to read secret key"))?;
    Ok((public_key, secret_key))
}

fn blockchain_explorer_api(api: &mut Api, b: Blockchain, cfg: GenesisConfig) {
    blockchain_explorer::make_api::<CurrencyTx>(api, b, cfg);
}

fn cryptocurrency_api(api: &mut Api,
                      blockchain: Blockchain,
                      channel: CurrencyTxSender,
                      cfg: GenesisConfig) {
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
                match ch.send(CurrencyTx::from(tx)) {
                    Ok(_) => {
                        let json = &jsonway::object(|json| json.set("tx_hash", tx_hash)).unwrap();
                        client.json(json)
                    }
                    Err(e) => client.error(e),
                }
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
                match ch.send(CurrencyTx::from(tx)) {
                    Ok(_) => {
                        let json = &jsonway::object(|json| json.set("tx_hash", tx_hash)).unwrap();
                        client.json(json)
                    }
                    Err(e) => client.error(e),
                }
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
                        match ch.send(CurrencyTx::from(tx)) {
                            Ok(_) => {
                                let json = &jsonway::object(|json| json.set("tx_hash", tx_hash))
                                    .unwrap();
                                client.json(json)
                            }
                            Err(e) => client.error(e),
                        }
                    }
                    Err(e) => return client.error(e),
                }
            })
        });

        let b = blockchain.clone();
        let c = cfg.clone();
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
                let view = b.view();
                let currency_api = CurrencyApi::new(&view, c.clone());
                match currency_api.wallet_info(&public_key) {
                    Ok(Some(info)) => client.json(&info.to_json()),
                    _ => client.error(StorageError::new("Unable to get wallet info")),
                }
            })
        })
    });
}

fn run_node(blockchain: Blockchain, node_cfg: NodeConfig, port: Option<u16>) {
    if let Some(port) = port {
        let mut node = Node::new(blockchain.clone(), node_cfg.clone());
        let channel = node.channel();

        let api_thread = thread::spawn(move || {
            let channel = channel.clone();
            let blockchain = blockchain.clone();

            let api = Api::build(move |api| {
                // Specify API version
                api.version("v1", Versioning::Path);
                api.prefix("api");

                api.error_formatter(|err, _media| {
                    let body;
                    let code;
                    if let Some(e) = err.downcast::<StorageError>() {
                        code = StatusCode::InternalServerError;
                        body = format!("An error in backend occured: {}", e);
                    } else if let Some(e) = err.downcast::<errors::NotMatch>() {
                        code = StatusCode::NotFound;
                        body = e.to_string();
                    } else if let Some(e) = err.downcast::<errors::Validation>() {
                        code = StatusCode::BadRequest;
                        body = e.to_string();
                    } else if let Some(e) = err.downcast::<ValueNotFound>() {
                        code = StatusCode::NotFound;
                        body = e.to_string();
                    } else if let Some(e) = err.downcast::<FromHexError>() {
                        code = StatusCode::BadRequest;
                        body = e.to_string();
                    } else if let Some(e) = err.downcast::<EventsError>() {
                        code = StatusCode::ServiceUnavailable;
                        body = e.to_string();
                    } else {
                        code = StatusCode::NotImplemented;
                        body = format!("Unspecified error: {:?}", err);
                    }
                    trace!("RestApi: code={}, body={}", code, body);

                    let json = &jsonway::object(|json| json.set("message", body)).unwrap();
                    Some(Response::from_json(code, &json))
                });

                blockchain_explorer_api(api, blockchain.clone(), node_cfg.genesis.clone());
                cryptocurrency_api(api, blockchain.clone(), channel.clone(), node_cfg.genesis);
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
    exonum::crypto::init();
    blockchain_explorer::helpers::init_logger().unwrap();

    let app = App::new("Simple cryptocurrency demo program")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Aleksey S. <aleksei.sidorov@xdev.re>")
        .about("Demo cryptocurrency validator node")
        .subcommand(GenerateCommand::new())
        .subcommand(RunCommand::new().arg(Arg::with_name("HTTP_PORT")
            .short("p")
            .long("port")
            .value_name("HTTP_PORT")
            .help("Run http server on given port")
            .takes_value(true)));
    let matches = app.get_matches();

    match matches.subcommand() {
        ("generate", Some(matches)) => GenerateCommand::execute(matches),
        ("run", Some(matches)) => {
            let port: Option<u16> = matches.value_of("HTTP_PORT").map(|x| x.parse().unwrap());
            let node_cfg = RunCommand::node_config(matches);
            let db = RunCommand::db(matches);

            let blockchain = Blockchain::new(db, vec![Box::new(CurrencyService::new())]);
            run_node(blockchain, node_cfg, port)
        }
        _ => {
            unreachable!("Wrong subcommand");
        }
    }
}