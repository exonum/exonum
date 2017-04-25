extern crate env_logger;
extern crate clap;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;

extern crate router;
extern crate iron;
extern crate bitcoin;

extern crate exonum;
extern crate blockchain_explorer;
extern crate anchoring_btc_service;
extern crate configuration_service;
extern crate timestamping;

use clap::{App, Arg};
use std::thread;
use std::net::SocketAddr;

use router::Router;
use bitcoin::network::constants::Network;

use exonum::blockchain::{Blockchain, Service};
use exonum::node::{Node, NodeConfig};
use exonum::config::ConfigFile;
use blockchain_explorer::helpers::{GenerateCommand, RunCommand, generate_testnet_config};
use blockchain_explorer::api::Api;
use configuration_service::ConfigurationService;
use configuration_service::config_api::PrivateConfigApi;
use anchoring_btc_service::AnchoringService;
use anchoring_btc_service::AnchoringRpc;
use anchoring_btc_service::{AnchoringNodeConfig, AnchoringConfig, AnchoringRpcConfig,
                            gen_anchoring_testnet_config};

use timestamping::TimestampingService;
use timestamping::api::PublicApi;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnchoringServiceConfig {
    pub common: AnchoringConfig,
    pub node: AnchoringNodeConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServicesConfig {
    pub node: NodeConfig,
    pub btc_anchoring: AnchoringServiceConfig,
}

fn run_node(blockchain: Blockchain, node_cfg: NodeConfig, port: Option<u16>) {

    let mut node = Node::new(blockchain.clone(), node_cfg.clone());

    let api_thread = match port {
        Some(port) => {
            let channel = node.channel().clone();
            let blockchain_clone = blockchain.clone();
            let thread = thread::spawn(move || {
                let keys = (node_cfg.public_key, node_cfg.secret_key);
                let listen_address: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();

                let mut router = Router::new();
                {
                    let timestamping_api = PublicApi::new(blockchain_clone, channel.clone());
                    info!("Timestamping service api started on {}", listen_address);
                    timestamping_api.wire(&mut router);
                }
                {
                    let config_api = PrivateConfigApi {
                        channel: channel.clone(),
                        config: keys.clone(),
                    };
                    info!("Configuration service api started on {}", listen_address);
                    config_api.wire(&mut router);
                }
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
        .subcommand(GenerateCommand::new()
                        .arg(Arg::with_name("ANCHORING_RPC_HOST")
                                 .long("anchoring-host")
                                 .value_name("ANCHORING_RPC_HOST")
                                 .takes_value(true))
                        .arg(Arg::with_name("ANCHORING_RPC_USER")
                                 .long("anchoring-user")
                                 .value_name("ANCHORING_RPC_USER")
                                 .required(false)
                                 .takes_value(true))
                        .arg(Arg::with_name("ANCHORING_RPC_PASSWD")
                                 .long("anchoring-password")
                                 .value_name("ANCHORING_RPC_PASSWD")
                                 .required(false)
                                 .takes_value(true))
                        .arg(Arg::with_name("ANCHORING_FUNDS")
                                 .long("anchoring-funds")
                                 .value_name("ANCHORING_FUNDS")
                                 .takes_value(true))
                        .arg(Arg::with_name("ANCHORING_NETWORK")
                                 .help("Bitcoin network")
                                 .long("anchoring-network")
                                 .takes_value(true)
                                 .required(true)))
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

            let host = matches
                .value_of("ANCHORING_RPC_HOST")
                .unwrap()
                .to_string();
            let user = matches
                .value_of("ANCHORING_RPC_USER")
                .map(|x| x.to_string());
            let passwd = matches
                .value_of("ANCHORING_RPC_PASSWD")
                .map(|x| x.to_string());
            let total_funds: u64 = matches
                .value_of("ANCHORING_FUNDS")
                .unwrap()
                .parse()
                .unwrap();
            let network = match matches.value_of("ANCHORING_NETWORK").unwrap() {
                "testnet" => Network::Testnet,
                "bitcoin" => Network::Bitcoin,
                _ => panic!("Wrong network type"),
            };

            let rpc = AnchoringRpcConfig {
                host: host,
                username: user,
                password: passwd,
            };
            let (anchoring_common, anchoring_nodes) =
                gen_anchoring_testnet_config(&AnchoringRpc::new(rpc.clone()),
                                             network,
                                             count,
                                             total_funds);

            let node_cfgs = generate_testnet_config(count, start_port);
            let dir = dir.join("validators");
            for (idx, node_cfg) in node_cfgs.into_iter().enumerate() {
                let cfg = ServicesConfig {
                    node: node_cfg,
                    btc_anchoring: AnchoringServiceConfig {
                        common: anchoring_common.clone(),
                        node: anchoring_nodes[idx].clone(),
                    },
                };
                let file_name = format!("{}.toml", idx);
                ConfigFile::save(&cfg, &dir.join(file_name)).unwrap();
            }
        }
        ("run", Some(matches)) => {
            let port: Option<u16> = matches.value_of("HTTP_PORT").map(|x| x.parse().unwrap());
            let path = RunCommand::node_config_path(matches);
            let db = RunCommand::db(matches);
            let cfg: ServicesConfig = ConfigFile::load(&path).unwrap();

            let anchoring_cfg = cfg.btc_anchoring;

            let client = AnchoringRpc::new(anchoring_cfg.node.rpc.clone());
            let services: Vec<Box<Service>> =
                vec![Box::new(TimestampingService::new()),
                     Box::new(ConfigurationService::new()),
                     Box::new(AnchoringService::new(client,
                                                    anchoring_cfg.common,
                                                    anchoring_cfg.node))];
            let blockchain = Blockchain::new(db, services);
            run_node(blockchain, cfg.node, port);
        }
        _ => {
            panic!("Wrong subcommand");
        }
    }
}
