extern crate iron;
extern crate env_logger;
extern crate clap;
extern crate serde;
extern crate bodyparser;
extern crate exonum;
extern crate router;
extern crate configuration_service;

use clap::App;

use exonum::blockchain::{Blockchain, Service};
use exonum::node::Node;
use exonum::helpers::clap::{GenerateCommand, RunCommand};

use configuration_service::ConfigurationService;

fn main() {
    exonum::crypto::init();
    exonum::helpers::init_logger().unwrap();

    let app = App::new("Simple configuration api demo program")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Exonum Team <exonum@bitfury.com>")
        .about("Demo validator node")
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

            let mut node = Node::new(blockchain, node_cfg);
            node.run().unwrap();
        }
        _ => {
            unreachable!("Wrong subcommand");
        }
    }
}
