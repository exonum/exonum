extern crate iron;
extern crate env_logger;
extern crate clap;
extern crate serde;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate bodyparser;

extern crate exonum;
extern crate blockchain_explorer;
extern crate router;
extern crate configuration_service;

use std::net::SocketAddr;
use std::thread;
use clap::{Arg, App};
use router::Router;

use exonum::blockchain::{Blockchain, Service};
use exonum::node::{Node, NodeConfig};
use configuration_service::ConfigurationService;
use configuration_service::config_api::ConfigApi;

use blockchain_explorer::helpers::{GenerateCommand, RunCommand};
use blockchain_explorer::api::Api;

fn run_node(blockchain: Blockchain, node_cfg: NodeConfig, port: Option<u16>) {
    if let Some(port) = port {
        let mut node = Node::new(blockchain.clone(), node_cfg.clone());
        let channel = node.channel();
        let chanel_copy = channel.clone();
        let blockchain_copy = blockchain.clone();
        let config_api_thread = thread::spawn(move || {

            let config_api = ConfigApi {
                channel: chanel_copy, 
                blockchain: blockchain_copy, 
                config: (node_cfg.public_key, node_cfg.secret_key), 
            };

            let listen_address: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
            info!("Config service api started on {}", listen_address);

            let mut router  = Router::new();
            config_api.wire(&mut router);
            let chain = iron::Chain::new(router);
            iron::Iron::new(chain).http(listen_address).unwrap();
        });

        node.run().unwrap();
        config_api_thread.join().unwrap();
    } else {
        Node::new(blockchain, node_cfg).run().unwrap();
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
        .subcommand(RunCommand::new().arg(Arg::with_name("HTTP_PORT")
            .short("p")
            .long("port")
            .value_name("HTTP_PORT")
            .help("Run config api http server on given port")
            .takes_value(true)));
    let matches = app.get_matches();

    match matches.subcommand() {
        ("generate", Some(matches)) => GenerateCommand::execute(matches),
        ("run", Some(matches)) => {
            let port: Option<u16> = matches.value_of("HTTP_PORT").map(|x| x.parse().unwrap());
            let node_cfg = RunCommand::node_config(matches);
            let db = RunCommand::db(matches);

            let services: Vec<Box<Service>> = vec![Box::new(ConfigurationService::new())];
            let blockchain = Blockchain::new(db, services);
            run_node(blockchain, node_cfg, port)
        }
        _ => {
            unreachable!("Wrong subcommand");
        }
    }
}
