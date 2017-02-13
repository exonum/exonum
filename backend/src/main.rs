extern crate jsonway;
extern crate iron;
extern crate hyper;
extern crate env_logger;
extern crate clap;
extern crate serde;
extern crate time;
extern crate rand;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate bodyparser;

extern crate exonum;
extern crate blockchain_explorer;
extern crate cryptocurrency;
extern crate router;
extern crate cookie;
extern crate configuration_service;

use std::net::SocketAddr;
use std::thread;
use clap::{Arg, App};
use router::Router;

use exonum::blockchain::{Blockchain, Service};
use exonum::node::{Node, NodeConfig};
use configuration_service::ConfigurationService;

use cryptocurrency::CurrencyService;
use cryptocurrency::api::CryptocurrencyApi;

use blockchain_explorer::helpers::{GenerateCommand, RunCommand};
use blockchain_explorer::api::Api;

fn run_node(blockchain: Blockchain, node_cfg: NodeConfig, port: Option<u16>) {
    if let Some(port) = port {
        let mut node = Node::new(blockchain.clone(), node_cfg.clone());
        let channel = node.channel();

        let api_thread = thread::spawn(move || {

            let cryptocurrency_api = CryptocurrencyApi {
                channel: channel.clone(),
                blockchain: blockchain.clone(),
            };
            let listen_address: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
            println!("Cryptocurrency node server started on {}", listen_address);

            let mut router = Router::new();
            cryptocurrency_api.wire(&mut router);
            let chain = iron::Chain::new(router);
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
        .author("Aleksey S. <aleksei.sidorov@bitfury.com>")
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

            let services: Vec<Box<Service>> = vec![Box::new(CurrencyService::new()),
                                                   Box::new(ConfigurationService::new())];
            let blockchain = Blockchain::new(db, services);
            run_node(blockchain, node_cfg, port)
        }
        _ => {
            unreachable!("Wrong subcommand");
        }
    }
}