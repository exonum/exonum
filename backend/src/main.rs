extern crate env_logger;
extern crate clap;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;

extern crate router;
extern crate iron;

extern crate exonum;
extern crate blockchain_explorer;
extern crate timestamping;

use clap::{App, Arg};
use std::thread;
use std::net::SocketAddr;

use router::Router;

use exonum::blockchain::{Blockchain, Service};
use exonum::node::{Node, NodeConfig};
use exonum::config::ConfigFile;
use blockchain_explorer::helpers::{GenerateCommand, RunCommand, generate_testnet_config};
use blockchain_explorer::api::Api;

use timestamping::TimestampingService;
use timestamping::api::PublicApi;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServicesConfig {
    pub node: NodeConfig,
}

fn run_node(blockchain: Blockchain, node_cfg: NodeConfig, port: Option<u16>) {

    let mut node = Node::new(blockchain.clone(), node_cfg.clone());

    let api_thread = match port {
        Some(port) => {
            let channel_clone = node.channel().clone();
            let blockchain_clone = blockchain.clone();
            let thread = thread::spawn(move || {

                let config_api = PublicApi::new(blockchain_clone, channel_clone);
                let listen_address: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
                info!("Timestamping service api started on {}", listen_address);

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
    if let Some(api_thread) = api_thread {
        api_thread.join().unwrap();
    }
}

fn main() {
    exonum::crypto::init();
    blockchain_explorer::helpers::init_logger().unwrap();

    let app = App::new("Simple timestamping demo program")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Aleksey S. <aleksei.sidorov@bitfury.com>")
        .subcommand(GenerateCommand::new())
        .subcommand(RunCommand::new().arg(Arg::with_name("HTTP_PORT")
                                              .short("p")
                                              .long("port")
                                              .value_name("HTTP_PORT")
                                              .help("Run http server on given port")
                                              .takes_value(true)));
    let matches = app.get_matches();

    match matches.subcommand() {
        ("generate", Some(matches)) => {
            let count = GenerateCommand::validators_count(matches);
            let dir = GenerateCommand::output_dir(matches);
            let start_port = GenerateCommand::start_port(matches).unwrap_or(9000);

            let node_cfgs = generate_testnet_config(count, start_port);
            let dir = dir.join("validators");
            for (idx, node_cfg) in node_cfgs.into_iter().enumerate() {
                let cfg = ServicesConfig { node: node_cfg };
                let file_name = format!("{}.toml", idx);
                ConfigFile::save(&cfg, &dir.join(file_name)).unwrap();
            }
        }
        ("run", Some(matches)) => {
            let path = RunCommand::node_config_path(matches);
            let db = RunCommand::db(matches);
            let cfg: ServicesConfig = ConfigFile::load(&path).unwrap();
            let port: Option<u16> = matches.value_of("HTTP_PORT").map(|x| x.parse().unwrap());

            let services: Vec<Box<Service>> = vec![Box::new(TimestampingService::new())];
            let blockchain = Blockchain::new(db, services);
            run_node(blockchain, cfg.node, port);
        }
        _ => {
            panic!("Wrong subcommand");
        }
    }
}
