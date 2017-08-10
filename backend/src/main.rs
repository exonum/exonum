extern crate exonum;
extern crate exonum_configuration;
#[cfg(feature = "anchoring")]
extern crate exonum_btc_anchoring;
extern crate timestamping;

use exonum::helpers::fabric::NodeBuilder;

use exonum_configuration::ConfigurationService;
#[cfg(feature = "anchoring")]
use exonum_btc_anchoring::AnchoringService;
use timestamping::TimestampingService;

#[cfg(feature = "anchoring")]
fn main() {
    exonum::helpers::init_logger().unwrap();
    NodeBuilder::new()
        .with_service::<ConfigurationService>()
        .with_service::<TimestampingService>()
        .with_service::<AnchoringService>()
        .run();
}

#[cfg(not(feature = "anchoring"))]
fn main() {
    exonum::helpers::init_logger().unwrap();
    NodeBuilder::new()
        .with_service::<ConfigurationService>()
        .with_service::<TimestampingService>()
        .run();
}