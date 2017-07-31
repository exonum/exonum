extern crate exonum;
extern crate exonum_configuration;
extern crate timestamping;

use exonum::helpers::fabric::NodeBuilder;

use exonum_configuration::ConfigurationService;
use timestamping::TimestampingService;

fn main() {
    exonum::helpers::init_logger().unwrap();
    NodeBuilder::new()
        .with_service::<ConfigurationService>()
        .with_service::<TimestampingService>()
        .run();
}
