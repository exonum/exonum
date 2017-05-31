extern crate env_logger;
extern crate clap;
#[macro_use]
extern crate serde_derive;

extern crate router;
extern crate iron;
extern crate bitcoin;

extern crate exonum;
extern crate anchoring_btc_service;
extern crate configuration_service;
extern crate timestamping;

use clap::{App, Arg};

use bitcoin::network::constants::Network;

use exonum::blockchain::{Blockchain, Service};
use exonum::node::{Node, NodeConfig};
use exonum::config::ConfigFile;
use exonum::helpers::clap::{GenerateCommand, RunCommand};
use exonum::helpers::generate_testnet_config;

use configuration_service::ConfigurationService;
use anchoring_btc_service::AnchoringService;
use anchoring_btc_service::AnchoringRpc;
use anchoring_btc_service::{AnchoringNodeConfig, AnchoringConfig, AnchoringRpcConfig,
                            gen_anchoring_testnet_config};

use timestamping::TimestampingService;

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

fn main() {
    exonum::crypto::init();
    exonum::helpers::init_logger().unwrap();

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
                        .arg(Arg::with_name("ANCHORING_FEE")
                                 .long("anchoring-fee")
                                 .value_name("ANCHORING_FEE")
                                 .takes_value(true))
                        .arg(Arg::with_name("ANCHORING_NETWORK")
                                 .help("Bitcoin network")
                                 .long("anchoring-network")
                                 .takes_value(true)
                                 .required(true)))
        .subcommand(RunCommand::new());
    let matches = app.get_matches();

    match matches.subcommand() {
        ("generate", Some(matches)) => {
            let count = GenerateCommand::validators_count(matches);
            let dir = GenerateCommand::output_dir(matches);
            let start_port = GenerateCommand::start_port(matches).unwrap_or(9000);

            let host = matches.value_of("ANCHORING_RPC_HOST").unwrap().to_string();
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
            let fee: u64 = matches.value_of("ANCHORING_FEE").unwrap().parse().unwrap();
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
            let (mut anchoring_common, anchoring_nodes) =
                gen_anchoring_testnet_config(&AnchoringRpc::new(rpc.clone()),
                                             network,
                                             count,
                                             total_funds);
            anchoring_common.fee = fee;

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
            let path = RunCommand::node_config_path(matches);
            let db = RunCommand::db(matches);
            let cfg: ServicesConfig = ConfigFile::load(&path).unwrap();

            let anchoring_cfg = cfg.btc_anchoring;

            let services: Vec<Box<Service>> =
                vec![Box::new(TimestampingService::new()),
                     Box::new(ConfigurationService::new()),
                     Box::new(AnchoringService::new(anchoring_cfg.common, anchoring_cfg.node))];

            let blockchain = Blockchain::new(db, services);
            let mut node = Node::new(blockchain.clone(), cfg.node.clone());
            node.run().unwrap();
        }
        _ => {
            panic!("Wrong subcommand");
        }
    }
}
