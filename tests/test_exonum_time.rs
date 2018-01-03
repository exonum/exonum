extern crate exonum;
extern crate exonum_time;
#[macro_use]
extern crate exonum_testkit;
#[macro_use]
extern crate pretty_assertions;

use std::time::{SystemTime, Duration, UNIX_EPOCH};

use exonum::helpers::{Height, ValidatorId};
use exonum::crypto::gen_keypair;

use exonum_time::{TimeService, TimeSchema, TxTime, Time, TimeProvider};
use exonum_testkit::{TestKitBuilder, TestNode};

#[test]
fn test_exonum_time_service() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(3)
        .with_service(TimeService::new())
        .create();

    let validators = testkit.network().validators().to_vec();

    // Validators time, that is saved in storage, look like this:
    // number | 0    | 1    | 2    |
    // time   | None | None | None |
    //
    // Time, that is saved in storage, is None

    let validators_time_test: Vec<Option<Time>> = vec![None, None, None];

    let snapshot = testkit.snapshot();
    let schema = TimeSchema::new(snapshot);
    let validators_time_storage = schema.validators_time();

    for (num, validator) in validators.iter().enumerate() {
        let pub_key = &validator.public_keys().service_key;
        assert_eq!(
            validators_time_test[num],
            validators_time_storage.get(pub_key)
        );
    }
    assert_eq!(schema.time().get(), None);

    // Add first transaction 'tx0' from first validator with time 'time0'.
    // After that validators time look like this:
    // number | 0       | 0    | 0    |
    // time   | 'time0' | None | None |
    //
    // Time, that is saved in storage, will have the value 'time0'.

    let time0 = SystemTime::now();
    let tx0 = {
        let (pub_key, sec_key) = validators[0].service_keypair();
        TxTime::new(time0, pub_key, sec_key)
    };
    testkit.create_block_with_transactions(txvec![tx0.clone()]);

    let validators_time_test: Vec<Option<Time>> = vec![Some(Time::new(time0)), None, None];
    let snapshot = testkit.snapshot();
    let schema = TimeSchema::new(snapshot);
    let validators_time_storage = schema.validators_time();

    for (num, validator) in validators.iter().enumerate() {
        let pub_key = &validator.public_keys().service_key;
        assert_eq!(
            validators_time_test[num],
            validators_time_storage.get(pub_key)
        );
    }
    assert_eq!(schema.time().get(), Some(Time::new(time0)));

    // Add second transaction 'tx1' from second validator with time 'time1' = 'time0' + 10 sec.
    // After that validators time look like this:
    // number | 0       | 1       | 2    |
    // time   | 'time0' | 'time1' | None |
    //
    // In sorted order: 'time1' >= 'time0'
    // Time, that is saved in storage, will have the value 'time1'.

    let time1 = time0 + Duration::new(10, 0);
    let tx1 = {
        let (pub_key, sec_key) = validators[1].service_keypair();
        TxTime::new(time1, pub_key, sec_key)
    };
    testkit.create_block_with_transactions(txvec![tx1.clone()]);

    let validators_time_test: Vec<Option<Time>> =
        vec![Some(Time::new(time0)), Some(Time::new(time1)), None];

    let snapshot = testkit.snapshot();
    let schema = TimeSchema::new(snapshot);
    let validators_time_storage = schema.validators_time();

    for (num, validator) in validators.iter().enumerate() {
        let pub_key = &validator.public_keys().service_key;
        assert_eq!(
            validators_time_test[num],
            validators_time_storage.get(pub_key)
        );
    }
    assert_eq!(schema.time().get(), Some(Time::new(time1)));

    // Add third transaction 'tx2' from third validator with time 'time2' = 'time1' + 10 sec.
    // After that validators time look like this:
    // number | 0       | 1       | 2       |
    // time   | 'time0' | 'time1' | 'time2' |
    //
    // In sorted order: 'time2' >= 'time1' >= 'time0'
    // Time, that is saved in storage, will have the value 'time2'.

    let time2 = time1 + Duration::new(10, 0);
    let tx2 = {
        let (pub_key, sec_key) = validators[2].service_keypair();
        TxTime::new(time2, pub_key, sec_key)
    };
    testkit.create_block_with_transactions(txvec![tx2.clone()]);

    let validators_time_test: Vec<Option<Time>> = vec![
        Some(Time::new(time0)),
        Some(Time::new(time1)),
        Some(Time::new(time2)),
    ];

    let snapshot = testkit.snapshot();
    let schema = TimeSchema::new(snapshot);
    let validators_time_storage = schema.validators_time();

    for (num, validator) in validators.iter().enumerate() {
        let pub_key = &validator.public_keys().service_key;
        assert_eq!(
            validators_time_test[num],
            validators_time_storage.get(pub_key)
        );
    }
    assert_eq!(schema.time().get(), Some(Time::new(time2)));
}

// A struct that provides the node with the current time.
#[derive(Debug)]
struct MyTimeProvider;
impl TimeProvider for MyTimeProvider {
    fn current_time(&self) -> SystemTime {
        UNIX_EPOCH
    }
}

#[test]
fn test_mock_provider() {
    // Create a simple testkit network.
    let mut testkit = TestKitBuilder::validator()
        .with_service(TimeService::with_provider(
            Box::new(MyTimeProvider) as Box<TimeProvider>,
        ))
        .create();

    // Get the validator public key.
    let validator_public_key = &testkit.network().validators().to_vec()[0]
        .public_keys()
        .service_key;

    let snapshot = testkit.snapshot();
    let schema = TimeSchema::new(snapshot);

    // Check that the blockchain does not contain time.
    assert_eq!(schema.time().get(), None);
    // Check that the time for the validator is unknown.
    assert_eq!(schema.validators_time().get(validator_public_key), None);

    // Create two blocks.
    testkit.create_blocks_until(Height(2));

    let snapshot = testkit.snapshot();
    let schema = TimeSchema::new(snapshot);

    // Check that the time in the blockchain and for the validator has been updated.
    assert_eq!(schema.time().get(), Some(Time::new(UNIX_EPOCH)));
    assert_eq!(
        schema.validators_time().get(validator_public_key),
        Some(Time::new(UNIX_EPOCH))
    );
}

#[test]
fn test_selected_time_less_than_time_in_storage() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(1)
        .with_service(TimeService::new())
        .create();

    let validators = testkit.network().validators().to_vec();

    let (pub_key_0, _) = validators[0].service_keypair();

    let cfg_change_height = Height(5);
    let new_cfg = {
        let mut cfg = testkit.configuration_change_proposal();
        cfg.set_validators(vec![TestNode::new_validator(ValidatorId(0))]);
        cfg.set_actual_from(cfg_change_height);
        cfg
    };
    testkit.commit_configuration_change(new_cfg);
    testkit.create_blocks_until(cfg_change_height.previous());

    let validators = testkit.network().validators().to_vec();
    let (pub_key_1, sec_key_1) = validators[0].service_keypair();

    let snapshot = testkit.snapshot();
    let schema = TimeSchema::new(snapshot);

    assert!(schema.time().get().is_some());
    assert!(schema.validators_time().get(pub_key_0).is_some());
    assert!(schema.validators_time().get(pub_key_1).is_none());
    assert_eq!(schema.time().get(), schema.validators_time().get(pub_key_0));

    if let Some(time_in_storage) = schema.time().get() {
        let time_tx = time_in_storage.time() - Duration::new(10, 0);
        let tx = {
            TxTime::new(time_tx, pub_key_1, sec_key_1)
        };
        testkit.create_block_with_transactions(txvec![tx.clone()]);
    }

    let snapshot = testkit.snapshot();
    let schema = TimeSchema::new(snapshot);
    assert!(schema.time().get().is_some());
    assert!(schema.validators_time().get(pub_key_0).is_some());
    assert!(schema.validators_time().get(pub_key_1).is_some());
    assert_eq!(schema.time().get(), schema.validators_time().get(pub_key_0));
}

#[test]
fn test_creating_transaction_is_not_validator() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(1)
        .with_service(TimeService::new())
        .create();

    let (pub_key, sec_key) = gen_keypair();
    let tx = TxTime::new(SystemTime::now(), &pub_key, &sec_key);
    testkit.create_block_with_transactions(txvec![tx.clone()]);

    let snapshot = testkit.snapshot();
    let schema = TimeSchema::new(snapshot);
    assert!(schema.time().get().is_none());
    assert!(schema.validators_time().get(&pub_key).is_none());
}

#[test]
fn test_transaction_time_less_than_validator_time_in_storage() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(1)
        .with_service(TimeService::new())
        .create();

    let validator = &testkit.network().validators().to_vec()[0];
    let (pub_key, sec_key) = validator.service_keypair();

    let time0 = SystemTime::now();
    let tx0 = TxTime::new(time0, pub_key, sec_key);

    testkit.create_block_with_transactions(txvec![tx0.clone()]);

    let snapshot = testkit.snapshot();
    let schema = TimeSchema::new(snapshot);

    assert_eq!(schema.time().get(), Some(Time::new(time0)));
    assert_eq!(
        schema.validators_time().get(pub_key),
        Some(Time::new(time0))
    );

    let time1 = time0 - Duration::new(10, 0);
    let tx1 = TxTime::new(time1, pub_key, sec_key);

    testkit.create_block_with_transactions(txvec![tx1.clone()]);

    let snapshot = testkit.snapshot();
    let schema = TimeSchema::new(snapshot);

    assert_eq!(schema.time().get(), Some(Time::new(time0)));
    assert_eq!(
        schema.validators_time().get(pub_key),
        Some(Time::new(time0))
    );
}
