extern crate iron;
extern crate env_logger;
extern crate clap;
extern crate serde;
#[macro_use]
extern crate log;
extern crate serde_json;
extern crate bodyparser;

extern crate exonum;
extern crate router;
extern crate configuration_service;

use std::net::SocketAddr;
use std::thread;
use clap::{Arg, App};
use router::Router;

use exonum::blockchain::{Blockchain, Service};
use exonum::node::{Node, NodeConfig};
use configuration_service::ConfigurationService;
use configuration_service::config_api::{PublicConfigApi, PrivateConfigApi};

use blockchain_explorer::helpers::{GenerateCommand, RunCommand};
use blockchain_explorer::api::Api;

fn run_node(blockchain: Blockchain,
            node_cfg: NodeConfig,
            private_port: Option<u16>,
            public_port: Option<u16>) {

    let mut node = Node::new(blockchain.clone(), node_cfg.clone());

    let private_config_api_thread = match private_port {
        Some(private_port) => {
            let channel_clone = node.channel().clone();
            let thread = thread::spawn(move || {

                let config_api = PrivateConfigApi {
                    channel: channel_clone,
                    config: (node_cfg.public_key, node_cfg.secret_key),
                };

                let listen_address: SocketAddr =
                    format!("127.0.0.1:{}", private_port).parse().unwrap();
                info!("Private config service api started on {}", listen_address);

                let mut router = Router::new();
                config_api.wire(&mut router);
                let chain = iron::Chain::new(router);
                iron::Iron::new(chain).http(listen_address).unwrap();
            });
            Some(thread)
        }
        None => None,
    };

    let public_config_api_thread = match public_port {
        Some(public_port) => {
            let thread = thread::spawn(move || {

                let config_api = PublicConfigApi { blockchain: blockchain };

                let listen_address: SocketAddr =
                    format!("127.0.0.1:{}", public_port).parse().unwrap();
                info!("Config service api started on {}", listen_address);

                let mut router = Router::new();
                config_api.wire(&mut router);
                let chain = iron::Chain::new(router);
                iron::Iron::new(chain).http(listen_address).unwrap();
            });
            Some(thread)
        }
        None => None,
    };
    node.run().unwrap();
    if let Some(private_config_api_thread) = private_config_api_thread {
        private_config_api_thread.join().unwrap();
    }
    if let Some(public_config_api_thread) = public_config_api_thread {
        public_config_api_thread.join().unwrap();
    }
}

fn main() {
    exonum::crypto::init();
    blockchain_explorer::helpers::init_logger().unwrap();

    let app = App::new("Simple configuration api demo program")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Aleksey S. <aleksei.sidorov@xdev.re>")
        .about("Demo validator node")
        .subcommand(GenerateCommand::new())
        .subcommand(RunCommand::new()
                        .arg(Arg::with_name("CFG_PUB_HTTP_PORT")
                                 .short("p")
                                 .long("public-port")
                                 .value_name("CFG_PUB_HTTP_PORT")
                                 .help("Run public config api http server on given port")
                                 .takes_value(true))
                        .arg(Arg::with_name("CFG_PRIV_HTTP_PORT")
                                 .short("s")
                                 .long("private-port")
                                 .value_name("CFG_PRIV_HTTP_PORT")
                                 .help("Run config api http server on given port")
                                 .takes_value(true)));
    let matches = app.get_matches();

    match matches.subcommand() {
        ("generate", Some(matches)) => GenerateCommand::execute(matches),
        ("run", Some(matches)) => {
            let pub_port: Option<u16> = matches
                .value_of("CFG_PUB_HTTP_PORT")
                .map(|x| x.parse().unwrap());
            let priv_port: Option<u16> = matches
                .value_of("CFG_PRIV_HTTP_PORT")
                .map(|x| x.parse().unwrap());
            let node_cfg = RunCommand::node_config(matches);
            let db = RunCommand::db(matches);

            let services: Vec<Box<Service>> = vec![Box::new(ConfigurationService::new())];
            let blockchain = Blockchain::new(db, services);
            run_node(blockchain, node_cfg, priv_port, pub_port)
        }
        _ => {
            unreachable!("Wrong subcommand");
        }
    }
}
