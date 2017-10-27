extern crate cryptocurrency;
extern crate exonum;

use exonum::node::Node;
use cryptocurrency::{blockchain, node_config};

fn main() {
    exonum::helpers::init_logger().unwrap();

    println!("Creating in-memory database...");
    let mut node = Node::new(blockchain(), node_config());
    println!("Starting a single node...");
    node.run().unwrap();
    println!("Blockchain is ready for transactions!");
}
