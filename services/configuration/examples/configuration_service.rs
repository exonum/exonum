extern crate env_logger;
extern crate clap;
#[macro_use]
extern crate log;

extern crate exonum;
extern crate blockchain_explorer;
extern crate configuration_service;

use clap::App;

use exonum::blockchain::{Blockchain, Service};
use exonum::node::Node;
use blockchain_explorer::helpers::{GenerateCommand, RunCommand};

use configuration_service::ConfigurationService;

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
        ("generate", Some(matches)) => GenerateCommand::execute(matches),
        ("run", Some(matches)) => {
            let node_cfg = RunCommand::node_config(matches);
            let db = RunCommand::db(matches);

            let services: Vec<Box<Service>> = vec![Box::new(ConfigurationService::new())];
            let blockchain = Blockchain::new(db, services);
            Node::new(blockchain, node_cfg).run().unwrap();
        }
        _ => {
            panic!("Wrong subcommand");
        }
    }
}