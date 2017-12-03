extern crate exonum;
extern crate exonum_time;
#[macro_use]
extern crate exonum_testkit;
#[macro_use]
extern crate pretty_assertions;

use std::time::{SystemTime, Duration};

use exonum_time::TimeSchema;
use exonum_time::{TimeService, TxTime, Time};
use exonum_testkit::{TestKitBuilder};

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
    // Time, that is saved in storage, does not change.

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
    assert_eq!(schema.time().get(), None);

    // Add second transaction 'tx1' from second validator with time 'time1' = 'time0' + 10 sec.
    // After that validators time look like this:
    // number | 0       | 1       | 2    |
    // time   | 'time0' | 'time1' | None |
    //
    // In sorted order: 'time1' >= 'time0'
    // Time, that is saved in storage, will have the value 'time0'.

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
    assert_eq!(schema.time().get(), Some(Time::new(time0)));

    // Add third transaction 'tx2' from third validator with time 'time2' = 'time1' + 10 sec.
    // After that validators time look like this:
    // number | 0       | 1       | 2       |
    // time   | 'time0' | 'time1' | 'time2' |
    //
    // In sorted order: 'time2' >= 'time1' >= 'time0'
    // Time, that is saved in storage, will have the value 'time1'.

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
    assert_eq!(schema.time().get(), Some(Time::new(time1)));
}
