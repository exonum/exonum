extern crate exonum;
extern crate exonum_testkit;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

use exonum::helpers::{Height, ValidatorId};
use exonum_testkit::TestKitBuilder;
use exonum::blockchain::Schema;

#[test]
fn test_add_to_validators() {
    let mut testkit = TestKitBuilder::auditor().with_validators(1).create();

    let proposal = {
        let mut cfg = testkit.actual_configuration();
        let mut validators = cfg.validators().to_vec();
        validators.push(testkit.network().us().clone());
        cfg.set_actual_from(Height(5));
        cfg.set_validators(validators);
        cfg
    };
    let stored = proposal.stored_configuration().clone();
    testkit.propose_configuration_change(proposal);

    testkit.create_blocks_until(Height(6));

    assert_eq!(testkit.network().us().validator_id(), Some(ValidatorId(1)));
    assert_eq!(testkit.network().validators()[1], testkit.network().us());
    assert_eq!(
        Schema::new(&testkit.snapshot()).actual_configuration(),
        stored
    );
}

#[test]
fn test_exclude_from_validators() {
    let mut testkit = TestKitBuilder::validator().with_validators(2).create();

    let proposal = {
        let mut cfg = testkit.actual_configuration();
        let validator = cfg.validators()[1].clone();
        cfg.set_actual_from(Height(5));
        cfg.set_validators(vec![validator]);
        cfg
    };
    let stored = proposal.stored_configuration().clone();
    testkit.propose_configuration_change(proposal);

    testkit.create_blocks_until(Height(6));

    assert_eq!(testkit.network().us().validator_id(), None);
    assert_eq!(testkit.network().validators().len(), 1);
    assert_eq!(
        Schema::new(&testkit.snapshot()).actual_configuration(),
        stored
    );
}

#[test]
fn test_change_service_config() {
    #[derive(Debug, Serialize, Deserialize, Clone)]
    struct ServiceConfig {
        name: String,
        value: u64,
    };

    let service_cfg = ServiceConfig {
        name: String::from("Config"),
        value: 64,
    };

    let mut testkit = TestKitBuilder::validator().create();
    let proposal = {
        let mut cfg = testkit.actual_configuration();
        cfg.set_service_config("my_service", service_cfg.clone());
        cfg.set_actual_from(Height(5));
        cfg
    };
    testkit.propose_configuration_change(proposal);

    testkit.create_blocks_until(Height(6));

    assert_eq!(
        serde_json::to_value(service_cfg).unwrap(),
        Schema::new(&testkit.snapshot())
            .actual_configuration()
            .services["my_service"]
    );
}
