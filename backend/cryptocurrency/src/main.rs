extern crate clap;
extern crate exonum;
extern crate cryptocurrency;
// extern crate configuration_service;

use clap::App;

use exonum::blockchain::{Blockchain, Service};
use exonum::node::Node;
use exonum::helpers::clap::{GenerateCommand, RunCommand};
// use configuration_service::ConfigurationService;

use cryptocurrency::CurrencyService;

fn main() {
    exonum::crypto::init();
    exonum::helpers::init_logger().unwrap();

    let app = App::new("Simple cryptocurrency demo program")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Exonum Team <exonum@bitfury.com>")
        .about("Demo cryptocurrency validator node")
        .subcommand(GenerateCommand::new())
        .subcommand(RunCommand::new());
    let matches = app.get_matches();

    match matches.subcommand() {
        ("generate", Some(matches)) => GenerateCommand::execute(matches),
        ("run", Some(matches)) => {
            let services: Vec<Box<Service>> = vec![Box::new(CurrencyService::new()),
                                                   // Box::new(ConfigurationService::new())
            ];
            let blockchain = Blockchain::new(RunCommand::db(matches), services);
            let mut node = Node::new(blockchain, RunCommand::node_config(matches));
            node.run().unwrap();
        }
        _ => {
            panic!("Wrong subcommand");
        }
    }
}
