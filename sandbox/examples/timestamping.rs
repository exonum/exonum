extern crate exonum;
extern crate timestamping;
extern crate sandbox;
extern crate clap;
extern crate blockchain_explorer;

use clap::App;

use exonum::node::Node;
use exonum::blockchain::Blockchain;

use timestamping::TimestampingService;
use blockchain_explorer::helpers::{GenerateCommand, RunCommand};

fn main() {
    exonum::crypto::init();
    blockchain_explorer::helpers::init_logger().unwrap();

    let app = App::new("Simple exonum demo program")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Aleksey S. <aleksei.sidorov@xdev.re>")
        .about("Exonum demo validator node")
        .subcommand(GenerateCommand::new())
        .subcommand(RunCommand::new());
    let matches = app.get_matches();

    match matches.subcommand() {
        ("generate", Some(matches)) => GenerateCommand::execute(matches),
        ("run", Some(matches)) => {
            let node_cfg = RunCommand::node_config(matches);
            let db = RunCommand::db(matches);

            let blockchain = Blockchain::new(db, vec![Box::new(TimestampingService::new())]);
            let mut node = Node::new(blockchain, node_cfg);
            node.run().unwrap();
        }
        _ => {
            unreachable!("Wrong subcommand");
        }
    }
}