extern crate exonum;
extern crate configuration_service;

use exonum::helpers::fabric::NodeBuilder;

use configuration_service::ConfigurationService;

fn main() {
    exonum::helpers::init_logger().unwrap();
    NodeBuilder::new()
                .with_service::<ConfigurationService>()
                .run();
}
