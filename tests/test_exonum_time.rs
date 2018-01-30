extern crate exonum;
extern crate exonum_time;
#[macro_use]
extern crate exonum_testkit;
#[macro_use]
extern crate pretty_assertions;

use std::collections::HashMap;
use std::iter::FromIterator;
use std::time::{SystemTime, Duration, UNIX_EPOCH};

use exonum::helpers::{Height, ValidatorId};
use exonum::crypto::{gen_keypair, PublicKey};
use exonum::storage::Snapshot;

use exonum_time::{MockTimeProvider, TimeService, TimeSchema, TxTime, Time, ValidatorTime};
use exonum_testkit::{ApiKind, TestKitApi, TestKitBuilder, TestNode};

fn assert_storage_times_eq<T: AsRef<Snapshot>>(
    snapshot: T,
    validators: &[TestNode],
    expected_current_time: Option<SystemTime>,
    expected_validators_times: &[Option<SystemTime>],
) {
    let schema = TimeSchema::new(snapshot);

    assert_eq!(
        schema.time().get().map(|time| time.time()),
        expected_current_time
    );

    let validators_times = schema.validators_time();
    for (i, validator) in validators.iter().enumerate() {
        let public_key = &validator.public_keys().service_key;

        assert_eq!(
            validators_times.get(public_key).map(|time| time.time()),
            expected_validators_times[i]
        );
    }
}

#[test]
fn test_exonum_time_service_with_3_validators() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(3)
        .with_service(TimeService::new())
        .create();

    let validators = testkit.network().validators().to_vec();

    // Validators time, that is saved in storage, look like this:
    // number | 0    | 1    | 2    |
    // time   | None | None | None |
    //
    // Consolidated time is None.

    assert_storage_times_eq(testkit.snapshot(), &validators, None, &[None, None, None]);

    // Add first transaction `tx0` from first validator with time `time0`.
    // After that validators time look like this:
    // number | 0       | 1    | 2    |
    // time   | `time0` | None | None |
    //
    // Consolidated time will have the value `time0`.

    let time0 = SystemTime::now();
    let tx0 = {
        let (pub_key, sec_key) = validators[0].service_keypair();
        TxTime::new(time0, pub_key, sec_key)
    };
    testkit.create_block_with_transactions(txvec![tx0]);

    assert_storage_times_eq(
        testkit.snapshot(),
        &validators,
        Some(time0),
        &[Some(time0), None, None],
    );

    // Add second transaction `tx1` from second validator with time `time1` = `time0` + 10 sec.
    // After that validators time look like this:
    // number | 0       | 1       | 2    |
    // time   | `time0` | `time1` | None |
    //
    // In sorted order: `time1` >= `time0`.
    // Consolidated time will have the value `time1`.

    let time1 = time0 + Duration::new(10, 0);
    let tx1 = {
        let (pub_key, sec_key) = validators[1].service_keypair();
        TxTime::new(time1, pub_key, sec_key)
    };
    testkit.create_block_with_transactions(txvec![tx1]);

    assert_storage_times_eq(
        testkit.snapshot(),
        &validators,
        Some(time1),
        &[Some(time0), Some(time1), None],
    );
}

#[test]
fn test_exonum_time_service_with_4_validators() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(4)
        .with_service(TimeService::new())
        .create();

    let validators = testkit.network().validators().to_vec();

    // Validators time, that is saved in storage, look like this:
    // number | 0    | 1    | 2    | 3    |
    // time   | None | None | None | None |
    //
    // max_byzantine_nodes = (4 - 1) / 3 = 1.
    //
    // Consolidated time is None.

    assert_storage_times_eq(
        testkit.snapshot(),
        &validators,
        None,
        &[None, None, None, None],
    );

    // Add first transaction `tx0` from first validator with time `time0`.
    // After that validators time look like this:
    // number | 0       | 1    | 2    | 3    |
    // time   | `time0` | None | None | None |
    //
    // Consolidated time doesn't change.

    let time0 = SystemTime::now();
    let tx0 = {
        let (pub_key, sec_key) = validators[0].service_keypair();
        TxTime::new(time0, pub_key, sec_key)
    };
    testkit.create_block_with_transactions(txvec![tx0]);

    assert_storage_times_eq(
        testkit.snapshot(),
        &validators,
        None,
        &[Some(time0), None, None, None],
    );

    // Add second transaction `tx1` from second validator with time `time1` = `time0` + 10 sec.
    // After that validators time look like this:
    // number | 0       | 1       | 2    | 3    |
    // time   | `time0` | `time1` | None | None |
    //
    // In sorted order: `time1` >= `time0`.
    // Consolidated time doesn't change.

    let time1 = time0 + Duration::new(10, 0);
    let tx1 = {
        let (pub_key, sec_key) = validators[1].service_keypair();
        TxTime::new(time1, pub_key, sec_key)
    };
    testkit.create_block_with_transactions(txvec![tx1]);

    assert_storage_times_eq(
        testkit.snapshot(),
        &validators,
        None,
        &[Some(time0), Some(time1), None, None],
    );

    // Add third transaction `tx2` from third validator with time `time2` = `time1` + 10 sec.
    // After that validators time look like this:
    // number | 0       | 1       | 2       | 3    |
    // time   | `time0` | `time1` | `time2` | None |
    //
    // In sorted order: `time2` >= `time1` >= `time0`.
    // Consolidated time will have the value `time1`.

    let time2 = time1 + Duration::new(10, 0);
    let tx2 = {
        let (pub_key, sec_key) = validators[2].service_keypair();
        TxTime::new(time2, pub_key, sec_key)
    };
    testkit.create_block_with_transactions(txvec![tx2]);

    assert_storage_times_eq(
        testkit.snapshot(),
        &validators,
        Some(time1),
        &[Some(time0), Some(time1), Some(time2), None],
    );

    // Add fourth transaction `tx3` from fourth validator with time `time3` = `time2` + 10 sec.
    // After that validators time look like this:
    // number | 0       | 1       | 2       | 3       |
    // time   | `time0` | `time1` | `time2` | `time3` |
    //
    // In sorted order: `time3` >= `time2` >= `time1` >= `time0`.
    // Consolidated time will have the value `time2`.

    let time3 = time2 + Duration::new(10, 0);
    let tx3 = {
        let (pub_key, sec_key) = validators[3].service_keypair();
        TxTime::new(time3, pub_key, sec_key)
    };
    testkit.create_block_with_transactions(txvec![tx3]);

    assert_storage_times_eq(
        testkit.snapshot(),
        &validators,
        Some(time2),
        &[Some(time0), Some(time1), Some(time2), Some(time3)],
    );
}

#[test]
fn test_mock_provider() {
    let mock_provider = MockTimeProvider::default();
    let mut testkit = TestKitBuilder::validator()
        .with_service(TimeService::with_provider(mock_provider.clone()))
        .create();

    let validators = testkit.network().validators().to_vec();

    mock_provider.add_time(Duration::new(10, 0));
    assert_eq!(UNIX_EPOCH + Duration::new(10, 0), mock_provider.time());
    testkit.create_blocks_until(Height(2));
    assert_storage_times_eq(
        testkit.snapshot(),
        &validators,
        Some(mock_provider.time()),
        &[Some(mock_provider.time())],
    );

    mock_provider.set_time(UNIX_EPOCH + Duration::new(50, 0));
    assert_eq!(UNIX_EPOCH + Duration::new(50, 0), mock_provider.time());
    testkit.create_blocks_until(Height(4));
    assert_storage_times_eq(
        testkit.snapshot(),
        &validators,
        Some(mock_provider.time()),
        &[Some(mock_provider.time())],
    );

    mock_provider.add_time(Duration::new(20, 0));
    assert_eq!(UNIX_EPOCH + Duration::new(70, 0), mock_provider.time());
    testkit.create_blocks_until(Height(6));
    assert_storage_times_eq(
        testkit.snapshot(),
        &validators,
        Some(mock_provider.time()),
        &[Some(mock_provider.time())],
    );

    mock_provider.set_time(UNIX_EPOCH + Duration::new(30, 0));
    assert_eq!(UNIX_EPOCH + Duration::new(30, 0), mock_provider.time());
    testkit.create_blocks_until(Height(8));
    assert_storage_times_eq(
        testkit.snapshot(),
        &validators,
        Some(UNIX_EPOCH + Duration::new(70, 0)),
        &[Some(UNIX_EPOCH + Duration::new(70, 0))],
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
        testkit.create_block_with_transactions(txvec![tx]);
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
    testkit.create_block_with_transactions(txvec![tx]);

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

    testkit.create_block_with_transactions(txvec![tx0]);

    let snapshot = testkit.snapshot();
    let schema = TimeSchema::new(snapshot);

    assert_eq!(schema.time().get(), Some(Time::new(time0)));
    assert_eq!(
        schema.validators_time().get(pub_key),
        Some(Time::new(time0))
    );

    let time1 = time0 - Duration::new(10, 0);
    let tx1 = TxTime::new(time1, pub_key, sec_key);

    testkit.create_block_with_transactions(txvec![tx1]);

    let snapshot = testkit.snapshot();
    let schema = TimeSchema::new(snapshot);

    assert_eq!(schema.time().get(), Some(Time::new(time0)));
    assert_eq!(
        schema.validators_time().get(pub_key),
        Some(Time::new(time0))
    );
}

fn get_current_time(api: &TestKitApi) -> Option<SystemTime> {
    api.get(ApiKind::Service("exonum_time"), "v1/current_time")
}

fn get_current_validators_times(api: &TestKitApi) -> Vec<ValidatorTime> {
    api.get_private(ApiKind::Service("exonum_time"), "v1/validators_times")
}

fn get_all_validators_times(api: &TestKitApi) -> Vec<ValidatorTime> {
    api.get_private(ApiKind::Service("exonum_time"), "v1/validators_times/all")
}

fn assert_current_time_eq(api: &TestKitApi, expected_time: Option<SystemTime>) {
    let current_time = get_current_time(api);
    assert_eq!(expected_time, current_time);
}

fn assert_current_validators_times_eq(
    api: &TestKitApi,
    expected_times: &HashMap<PublicKey, Option<SystemTime>>,
) {
    let validators_times =
        HashMap::from_iter(get_current_validators_times(api).iter().map(|validator| {
            (validator.public_key, validator.time)
        }));

    assert_eq!(*expected_times, validators_times);
}

fn assert_all_validators_times_eq(
    api: &TestKitApi,
    expected_validators_times: &HashMap<PublicKey, Option<SystemTime>>,
) {
    let validators_times =
        HashMap::from_iter(get_all_validators_times(api).iter().map(|validator| {
            (validator.public_key, validator.time)
        }));

    assert_eq!(*expected_validators_times, validators_times);
}

#[test]
fn test_endpoint_api() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(3)
        .with_service(TimeService::new())
        .create();

    let api = testkit.api();
    let validators = testkit.network().validators().to_vec();
    let mut current_validators_times: HashMap<PublicKey, Option<SystemTime>> =
        HashMap::from_iter(validators.iter().map(|validator| {
            (*validator.service_keypair().0, None)
        }));
    let mut all_validators_times = HashMap::new();

    assert_current_time_eq(&api, None);
    assert_current_validators_times_eq(&api, &current_validators_times);
    assert_all_validators_times_eq(&api, &all_validators_times);

    let time0 = SystemTime::now();
    let (pub_key, sec_key) = validators[0].service_keypair();
    testkit.create_block_with_transactions(txvec![TxTime::new(time0, pub_key, sec_key)]);
    current_validators_times.insert(*pub_key, Some(time0));
    all_validators_times.insert(*pub_key, Some(time0));

    assert_current_time_eq(&api, Some(time0));
    assert_current_validators_times_eq(&api, &current_validators_times);
    assert_all_validators_times_eq(&api, &all_validators_times);

    let time1 = time0 + Duration::new(10, 0);
    let (pub_key, sec_key) = validators[1].service_keypair();
    testkit.create_block_with_transactions(txvec![TxTime::new(time1, pub_key, sec_key)]);
    current_validators_times.insert(*pub_key, Some(time1));
    all_validators_times.insert(*pub_key, Some(time1));

    assert_current_time_eq(&api, Some(time1));
    assert_current_validators_times_eq(&api, &current_validators_times);
    assert_all_validators_times_eq(&api, &all_validators_times);

    let time2 = time1 + Duration::new(10, 0);
    let (pub_key, sec_key) = validators[2].service_keypair();
    testkit.create_block_with_transactions(txvec![TxTime::new(time2, pub_key, sec_key)]);
    current_validators_times.insert(*pub_key, Some(time2));
    all_validators_times.insert(*pub_key, Some(time2));

    assert_current_time_eq(&api, Some(time2));
    assert_current_validators_times_eq(&api, &current_validators_times);
    assert_all_validators_times_eq(&api, &all_validators_times);

    let public_key_0 = validators[0].service_keypair().0;
    let cfg_change_height = Height(10);
    let new_cfg = {
        let mut cfg = testkit.configuration_change_proposal();
        cfg.set_validators(vec![
            TestNode::new_validator(ValidatorId(3)),
            validators[1].clone(),
            validators[2].clone(),
        ]);
        cfg.set_actual_from(cfg_change_height);
        cfg
    };
    testkit.commit_configuration_change(new_cfg);
    testkit.create_blocks_until(cfg_change_height.previous());

    current_validators_times.remove(public_key_0);
    let validators = testkit.network().validators().to_vec();
    current_validators_times.insert(*validators[0].service_keypair().0, None);

    let snapshot = testkit.snapshot();
    let schema = TimeSchema::new(&snapshot);
    if let Some(time) = schema.validators_time().get(public_key_0) {
        all_validators_times.insert(*public_key_0, Some(time.time()));
    }

    assert_current_time_eq(&api, Some(time2));
    assert_current_validators_times_eq(&api, &current_validators_times);
    assert_all_validators_times_eq(&api, &all_validators_times);

    let time3 = time2 + Duration::new(10, 0);
    let (pub_key, sec_key) = validators[0].service_keypair();
    testkit.create_block_with_transactions(txvec![TxTime::new(time3, pub_key, sec_key)]);
    current_validators_times.insert(*pub_key, Some(time3));
    all_validators_times.insert(*pub_key, Some(time3));

    assert_current_time_eq(&api, Some(time3));
    assert_current_validators_times_eq(&api, &current_validators_times);
    assert_all_validators_times_eq(&api, &all_validators_times);
}
