extern crate env_logger;
extern crate clap;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;

extern crate exonum;
extern crate blockchain_explorer;
extern crate propose_timeout_adjuster;

use clap::App;

use exonum::blockchain::{Blockchain, Service};
use exonum::node::{Node, NodeConfig};
use exonum::config::ConfigFile;
use blockchain_explorer::helpers::{GenerateCommand, RunCommand, generate_testnet_config};

use propose_timeout_adjuster::{ProposeTimeoutAdjusterService, ProposeTimeoutAdjusterConfig};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServicesConfig {
    pub node: NodeConfig,
    pub propose_timeout_adjuster: ProposeTimeoutAdjusterConfig,
}

fn main() {
    exonum::crypto::init();
    blockchain_explorer::helpers::init_logger().unwrap();

    let app = App::new("Simple configuration service demo program")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Aleksey S. <aleksei.sidorov@xdev.re>")
        .subcommand(GenerateCommand::new())
        .subcommand(RunCommand::new());
    let matches = app.get_matches();

    match matches.subcommand() {
        ("generate", Some(matches)) => {
            let count = GenerateCommand::validators_count(matches);
            let dir = GenerateCommand::output_dir(matches);
            let start_port = GenerateCommand::start_port(matches).unwrap_or(2000);

            let node_cfgs = generate_testnet_config(count, start_port);
            let dir = dir.join("validators");
            for (idx, node_cfg) in node_cfgs.into_iter().enumerate() {
                let cfg = ServicesConfig {
                    node: node_cfg,
                    propose_timeout_adjuster: ProposeTimeoutAdjusterConfig::default(),
                };
                let file_name = format!("{}.toml", idx);
                ConfigFile::save(&cfg, &dir.join(file_name)).unwrap();
            }
        }
        ("run", Some(matches)) => {
            // TODO add service with transactions

            let path = RunCommand::node_config_path(matches);
            let db = RunCommand::db(matches);
            let cfg: ServicesConfig = ConfigFile::load(&path).unwrap();

            let propose_timeout_adjuster_cfg = cfg.propose_timeout_adjuster;
            let services: Vec<Box<Service>> =
                vec![Box::new(ProposeTimeoutAdjusterService::new(propose_timeout_adjuster_cfg))];
            let blockchain = Blockchain::new(db, services);
            Node::new(blockchain, cfg.node).run().unwrap();
        }
        _ => {
            panic!("Wrong subcommand");
        }
    }
}
