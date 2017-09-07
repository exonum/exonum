extern crate exonum;
extern crate currency;
extern crate exonum_configuration;

use exonum::helpers;
use exonum::helpers::fabric::NodeBuilder;
use exonum_configuration::ConfigurationService;
use currency::CurrencyService;

fn main() {
    exonum::crypto::init();
    helpers::init_logger().unwrap();

    let node = NodeBuilder::new()
        .with_service::<ConfigurationService>()
        .with_service::<CurrencyService>();
    node.run();
}
