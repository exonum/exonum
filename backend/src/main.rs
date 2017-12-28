extern crate exonum;
extern crate cryptocurrency;
extern crate exonum_configuration;

use exonum::helpers;
use exonum::helpers::fabric::NodeBuilder;
use exonum_configuration::ConfigurationServiceFactory;
use cryptocurrency::CurrencyService;

fn main() {
    exonum::crypto::init();
    helpers::init_logger().unwrap();

    let node = NodeBuilder::new()
        .with_service(Box::new(ConfigurationServiceFactory))
        .with_service(Box::new(CurrencyService::new()));
    node.run();
}
