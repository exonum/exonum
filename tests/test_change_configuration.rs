extern crate exonum;
extern crate exonum_testkit;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

use exonum::helpers::{Height, ValidatorId};
use exonum_testkit::TestKitBuilder;
use exonum::blockchain::Schema;
use exonum::storage::StorageValue;

#[test]
fn test_add_to_validators() {
    let mut testkit = TestKitBuilder::auditor().with_validators(1).create();

    let cfg_change_height = Height(5);
    let proposal = {
        let mut cfg = testkit.configuration_change_proposal();
        let mut validators = cfg.validators().to_vec();
        validators.push(testkit.network().us().clone());
        cfg.set_actual_from(cfg_change_height);
        cfg.set_validators(validators);
        cfg
    };
    let stored = proposal.stored_configuration().clone();
    testkit.commit_configuration_change(proposal);

    testkit.create_blocks_until(cfg_change_height.previous());

    assert_eq!(testkit.network().us().validator_id(), Some(ValidatorId(1)));
    assert_eq!(&testkit.network().validators()[1], testkit.network().us());
    assert_eq!(testkit.actual_configuration(), stored);
    assert_eq!(
        Schema::new(&testkit.snapshot())
            .previous_configuration()
            .unwrap()
            .hash(),
        stored.previous_cfg_hash
    );
}

#[test]
fn test_exclude_from_validators() {
    let mut testkit = TestKitBuilder::validator().with_validators(2).create();

    let cfg_change_height = Height(5);
    let proposal = {
        let mut cfg = testkit.configuration_change_proposal();
        let validator = cfg.validators()[1].clone();
        cfg.set_actual_from(cfg_change_height);
        cfg.set_validators(vec![validator]);
        cfg
    };
    let stored = proposal.stored_configuration().clone();
    testkit.commit_configuration_change(proposal);

    testkit.create_blocks_until(cfg_change_height.previous());

    assert_eq!(testkit.network().us().validator_id(), None);
    assert_eq!(testkit.network().validators().len(), 1);
    assert_eq!(testkit.actual_configuration(), stored);
    assert_eq!(
        Schema::new(&testkit.snapshot())
            .previous_configuration()
            .unwrap()
            .hash(),
        stored.previous_cfg_hash
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
    let cfg_change_height = Height(5);
    let proposal = {
        let mut cfg = testkit.configuration_change_proposal();
        cfg.set_service_config("my_service", service_cfg.clone());
        cfg.set_actual_from(cfg_change_height);
        cfg
    };
    testkit.commit_configuration_change(proposal);

    testkit.create_blocks_until(cfg_change_height.previous());

    assert_eq!(
        serde_json::to_value(service_cfg).unwrap(),
        testkit.actual_configuration().services["my_service"]
    );
}

#[test]
#[should_panic(expected = "The `actual_from` height should be greater than the current")]
fn test_incorrect_actual_from_field() {
    let mut testkit = TestKitBuilder::auditor().with_validators(1).create();
    testkit.create_blocks_until(Height(2));
    let proposal = {
        let mut cfg = testkit.configuration_change_proposal();
        cfg.set_actual_from(Height(2));
        cfg
    };
    testkit.commit_configuration_change(proposal);
}

#[test]
#[should_panic(expected = "There is an active configuration change proposal")]
fn test_another_configuration_change_proposal() {
    let mut testkit = TestKitBuilder::auditor().with_validators(1).create();
    let first_proposal = {
        let mut cfg = testkit.configuration_change_proposal();
        cfg.set_actual_from(Height(10));
        cfg
    };
    testkit.commit_configuration_change(first_proposal);
    let second_proposal = {
        let mut cfg = testkit.configuration_change_proposal();
        cfg.set_actual_from(Height(11));
        cfg
    };
    testkit.commit_configuration_change(second_proposal);
}