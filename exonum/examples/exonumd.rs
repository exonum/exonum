extern crate exonum;

use exonum::helpers::fabric::NodeBuilder;
use exonum::helpers;

fn main() {
    exonum::crypto::init();
    helpers::init_logger().unwrap();
    let node = NodeBuilder::new();
    node.run();
}
