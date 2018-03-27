extern crate exonum;
extern crate advanced_cryptocurrency;
extern crate exonum_configuration;

use exonum::helpers;
use exonum::helpers::fabric::NodeBuilder;
use exonum_configuration as configuration;
use advanced_cryptocurrency as cryptocurrency;

fn main() {
    exonum::crypto::init();
    helpers::init_logger().unwrap();

    let node = NodeBuilder::new()
        .with_service(Box::new(configuration::ServiceFactory))
        .with_service(Box::new(cryptocurrency::CurrencyService));
    node.run();
}
