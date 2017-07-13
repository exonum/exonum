extern crate exonum;
extern crate exonum_configuration;

use exonum::helpers::fabric::NodeBuilder;

use exonum_configuration::ConfigurationService;

fn main() {
    exonum::helpers::init_logger().unwrap();
    NodeBuilder::new()
        .with_service::<ConfigurationService>()
        .run();
}
