extern crate exonum;
extern crate timestamping;
extern crate sandbox;
extern crate clap;
extern crate blockchain_explorer;

use clap::App;

use exonum::node::{Node, NodeConfig};
use exonum::blockchain::Blockchain;

use timestamping::TimestampingBlockchain;
use blockchain_explorer::helpers::{GenerateCommand, RunCommand, DatabaseType};

fn run_node<B: Blockchain>(blockchain: B, node_cfg: NodeConfig) {
    let mut node = Node::new(blockchain, node_cfg);
    node.run().unwrap();
}

fn main() {
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
            match RunCommand::db(matches) {
                DatabaseType::LevelDB(db) => run_node(TimestampingBlockchain { db: db }, node_cfg),
                DatabaseType::MemoryDB(db) => run_node(TimestampingBlockchain { db: db }, node_cfg),
            }
        }
        _ => {
            unreachable!("Wrong subcommand");
        }
    }
}