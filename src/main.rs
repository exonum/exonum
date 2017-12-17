extern crate cryptocurrency;
extern crate exonum;

use exonum::node::Node;
use exonum::storage::MemoryDB;

use cryptocurrency::{CurrencyService, node_config};

fn main() {
    exonum::helpers::init_logger().unwrap();

    println!("Creating in-memory database...");
    let node = Node::new(
        Box::new(MemoryDB::new()),
        vec![Box::new(CurrencyService)],
        node_config(),
    );
    println!("Starting a single node...");
    println!("Blockchain is ready for transactions!");
    node.run().unwrap();
}
