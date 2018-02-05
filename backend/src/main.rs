extern crate exonum;
extern crate exonum_configuration;
#[cfg(feature = "anchoring")]
extern crate exonum_btc_anchoring;
extern crate timestamping;

use exonum::helpers::fabric::NodeBuilder;

use exonum_configuration::ConfigurationServiceFactory;
#[cfg(feature = "anchoring")]
use exonum_btc_anchoring::AnchoringServiceFactory;
use timestamping::TimestampingService;

#[cfg(feature = "anchoring")]
fn main() {
    exonum::helpers::init_logger().unwrap();
    NodeBuilder::new()
        .with_service(Box::new(ConfigurationServiceFactory))
        .with_service(Box::new(TimestampingService::new()))
        .with_service(Box::new(AnchoringServiceFactory))
        .run();
}

#[cfg(not(feature = "anchoring"))]
fn main() {
    exonum::helpers::init_logger().unwrap();
    NodeBuilder::new()
        .with_service(Box::new(ConfigurationServiceFactory))
        .with_service(Box::new(TimestampingService::new()))
        .run();
}
