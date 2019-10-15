// Copyright 2019 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#[macro_use]
extern crate exonum_testkit;
#[macro_use]
extern crate pretty_assertions;

use exonum_merkledb::{IndexAccess, Snapshot};

use chrono::{DateTime, Duration, TimeZone, Utc};
use exonum::{
    blockchain::{ExecutionErrorKind, ExecutionStatus, Schema},
    crypto::{gen_keypair, PublicKey},
    helpers::Height,
    messages::Verified,
    runtime::{rust::Transaction, AnyTx, InstanceId},
};
use exonum_merkledb::ObjectHash;
use exonum_testkit::{
    simple_supervisor::{ConfigPropose, SimpleSupervisor},
    ApiKind, InstanceCollection, TestKitApi, TestKitBuilder, TestNode,
};
use exonum_time::{
    api::ValidatorTime, schema::TimeSchema, time_provider::MockTimeProvider, transactions::Error,
    transactions::TxTime, TimeServiceFactory,
};

use std::{collections::HashMap, iter::FromIterator, rc::Rc};

const INSTANCE_ID: InstanceId = 112;
const INSTANCE_NAME: &str = "my-time";

struct TimeServiceInstance;

impl From<TimeServiceInstance> for InstanceCollection {
    fn from(_: TimeServiceInstance) -> Self {
        InstanceCollection::new(TimeServiceFactory::default()).with_instance(
            INSTANCE_ID,
            INSTANCE_NAME,
            (),
        )
    }
}

fn assert_storage_times_eq<T: IndexAccess>(
    snapshot: T,
    validators: &[TestNode],
    expected_current_time: Option<DateTime<Utc>>,
    expected_validators_times: &[Option<DateTime<Utc>>],
) {
    let schema = TimeSchema::new(INSTANCE_NAME, snapshot);

    assert_eq!(schema.time().get(), expected_current_time);

    let validators_times = schema.validators_times();
    for (i, validator) in validators.iter().enumerate() {
        let public_key = &validator.public_keys().service_key;

        assert_eq!(
            validators_times.get(&public_key),
            expected_validators_times[i]
        );
    }
}

fn assert_transaction_result<S: IndexAccess>(
    snapshot: S,
    transaction: &Verified<AnyTx>,
    expected_code: u8,
) -> String {
    let result = Schema::new(snapshot)
        .transaction_results()
        .get(&transaction.object_hash());
    match result {
        Some(ExecutionStatus(Err(e))) => {
            assert_eq!(e.kind, ExecutionErrorKind::service(expected_code));
            e.description
        }
        _ => {
            panic!("Expected Err(), found None or Ok()");
        }
    }
}

#[test]
fn test_exonum_time_service_with_3_validators() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(3)
        .with_rust_service(TimeServiceInstance)
        .create();

    let validators = testkit.network().validators().to_vec();

    // Validators time, that is saved in storage, look like this:
    // number | 0    | 1    | 2    |
    // time   | None | None | None |
    //
    // Consolidated time is None.

    assert_storage_times_eq(&testkit.snapshot(), &validators, None, &[None, None, None]);

    // Add first transaction `tx0` from first validator with time `time0`.
    // After that validators time look like this:
    // number | 0       | 1    | 2    |
    // time   | `time0` | None | None |
    //
    // Consolidated time will have the value `time0`.

    let time0 = Utc::now();
    let tx0 = {
        let (pub_key, sec_key) = validators[0].service_keypair();
        TxTime { time: time0 }.sign(INSTANCE_ID, pub_key, &sec_key)
    };
    testkit.create_block_with_transactions(txvec![tx0]);

    assert_storage_times_eq(
        &testkit.snapshot(),
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

    let time1 = time0 + Duration::seconds(10);
    let tx1 = {
        let (pub_key, sec_key) = validators[1].service_keypair();
        TxTime { time: time1 }.sign(INSTANCE_ID, pub_key, &sec_key)
    };
    testkit.create_block_with_transactions(txvec![tx1]);

    assert_storage_times_eq(
        &testkit.snapshot(),
        &validators,
        Some(time1),
        &[Some(time0), Some(time1), None],
    );
}

#[test]
fn test_exonum_time_service_with_4_validators() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(4)
        .with_rust_service(TimeServiceInstance)
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
        &testkit.snapshot(),
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

    let time0 = Utc::now();
    let tx0 = {
        let (pub_key, sec_key) = validators[0].service_keypair();
        TxTime { time: time0 }.sign(INSTANCE_ID, pub_key, &sec_key)
    };
    testkit.create_block_with_transactions(txvec![tx0]);

    assert_storage_times_eq(
        &testkit.snapshot(),
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

    let time1 = time0 + Duration::seconds(10);
    let tx1 = {
        let (pub_key, sec_key) = validators[1].service_keypair();
        TxTime { time: time1 }.sign(INSTANCE_ID, pub_key, &sec_key)
    };
    testkit.create_block_with_transactions(txvec![tx1]);

    assert_storage_times_eq(
        &testkit.snapshot(),
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

    let time2 = time1 + Duration::seconds(10);
    let tx2 = {
        let (pub_key, sec_key) = validators[2].service_keypair();
        TxTime { time: time2 }.sign(INSTANCE_ID, pub_key, &sec_key)
    };
    testkit.create_block_with_transactions(txvec![tx2]);

    assert_storage_times_eq(
        &testkit.snapshot(),
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

    let time3 = time2 + Duration::seconds(10);
    let tx3 = {
        let (pub_key, sec_key) = validators[3].service_keypair();
        TxTime { time: time3 }.sign(INSTANCE_ID, pub_key, &sec_key)
    };
    testkit.create_block_with_transactions(txvec![tx3]);

    assert_storage_times_eq(
        &testkit.snapshot(),
        &validators,
        Some(time2),
        &[Some(time0), Some(time1), Some(time2), Some(time3)],
    );
}

#[test]
fn test_exonum_time_service_with_7_validators() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(7)
        .with_rust_service(TimeServiceInstance)
        .create();

    let validators = testkit.network().validators().to_vec();
    let mut validators_times = vec![None; 7];

    assert_storage_times_eq(&testkit.snapshot(), &validators, None, &validators_times);

    let time = Utc::now();
    let times = (0..7)
        .map(|x| time + Duration::seconds(x * 10))
        .collect::<Vec<_>>();
    let expected_storage_times = vec![
        None,
        None,
        None,
        None,
        Some(times[2]),
        Some(times[3]),
        Some(times[4]),
    ];

    for (i, validator) in validators.iter().enumerate() {
        let tx = {
            let (pub_key, sec_key) = validator.service_keypair();
            TxTime { time: times[i] }.sign(INSTANCE_ID, pub_key, &sec_key)
        };
        testkit.create_block_with_transactions(txvec![tx.clone()]);
        assert_eq!(
            Schema::new(&testkit.snapshot())
                .transaction_results()
                .get(&tx.object_hash()),
            Some(ExecutionStatus::ok())
        );

        validators_times[i] = Some(times[i]);

        assert_storage_times_eq(
            &testkit.snapshot(),
            &validators,
            expected_storage_times[i],
            &validators_times,
        );
    }
}

#[test]
fn test_mock_provider() {
    let mock_provider = MockTimeProvider::default();
    let mut testkit = TestKitBuilder::validator()
        .with_rust_service(
            InstanceCollection::new(TimeServiceFactory::with_provider(mock_provider.clone()))
                .with_instance(INSTANCE_ID, INSTANCE_NAME, ()),
        )
        .create();

    let validators = testkit.network().validators().to_vec();
    let assert_storage_times = |snapshot: Box<dyn Snapshot>| {
        assert_storage_times_eq(
            &snapshot,
            &validators,
            Some(mock_provider.time()),
            &[Some(mock_provider.time())],
        );
    };

    mock_provider.add_time(Duration::seconds(10));
    assert_eq!(Utc.timestamp(10, 0), mock_provider.time());
    testkit.create_blocks_until(Height(2));
    assert_storage_times(testkit.snapshot());

    mock_provider.set_time(Utc.timestamp(50, 0));
    assert_eq!(Utc.timestamp(50, 0), mock_provider.time());
    testkit.create_blocks_until(Height(4));
    assert_storage_times(testkit.snapshot());

    mock_provider.add_time(Duration::seconds(20));
    assert_eq!(Utc.timestamp(70, 0), mock_provider.time());
    testkit.create_blocks_until(Height(6));
    assert_storage_times(testkit.snapshot());

    mock_provider.set_time(Utc.timestamp(30, 0));
    assert_eq!(Utc.timestamp(30, 0), mock_provider.time());
    testkit.create_blocks_until(Height(8));
    assert_storage_times_eq(
        &testkit.snapshot(),
        &validators,
        Some(Utc.timestamp(70, 0)),
        &[Some(Utc.timestamp(70, 0))],
    );
}

#[test]
fn test_selected_time_less_than_time_in_storage() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(1)
        .with_rust_service(TimeServiceInstance)
        .with_rust_service(SimpleSupervisor)
        .create();

    let validators = testkit.network().validators().to_vec();

    let (pub_key_0, _) = validators[0].service_keypair();

    let cfg_change_height = Height(5);
    let new_cfg = {
        let mut cfg = testkit.consensus_config();
        cfg.validator_keys = vec![testkit.network_mut().add_node().public_keys()];
        cfg
    };

    testkit.create_block_with_transaction(
        ConfigPropose::actual_from(cfg_change_height)
            .consensus_config(new_cfg)
            .into_tx(),
    );
    testkit.create_blocks_until(cfg_change_height);

    let validators = testkit.network().validators().to_vec();
    let (pub_key_1, sec_key_1) = validators[0].service_keypair();

    let snapshot = testkit.snapshot();
    let schema = TimeSchema::new(INSTANCE_NAME, &snapshot);

    assert!(schema.time().get().is_some());
    assert!(schema.validators_times().get(&pub_key_0).is_some());
    assert!(schema.validators_times().get(&pub_key_1).is_none());
    assert_eq!(
        schema.time().get(),
        schema.validators_times().get(&pub_key_0)
    );

    if let Some(time_in_storage) = schema.time().get() {
        let time_tx = time_in_storage - Duration::seconds(10);
        let tx = TxTime { time: time_tx }.sign(INSTANCE_ID, pub_key_1, &sec_key_1);
        testkit.create_block_with_transactions(txvec![tx.clone()]);
        assert_eq!(
            Schema::new(&testkit.snapshot())
                .transaction_results()
                .get(&tx.object_hash()),
            Some(ExecutionStatus::ok())
        );
    }

    let snapshot = testkit.snapshot();
    let schema = TimeSchema::new(INSTANCE_NAME, &snapshot);
    assert!(schema.time().get().is_some());
    assert!(schema.validators_times().get(&pub_key_0).is_some());
    assert!(schema.validators_times().get(&pub_key_1).is_some());
    assert_eq!(
        schema.time().get(),
        schema.validators_times().get(&pub_key_0)
    );
}

#[test]
fn test_creating_transaction_is_not_validator() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(1)
        .with_rust_service(TimeServiceInstance)
        .create();

    let (pub_key, sec_key) = gen_keypair();
    let tx = TxTime { time: Utc::now() }.sign(INSTANCE_ID, pub_key, &sec_key);
    testkit.create_block_with_transactions(txvec![tx.clone()]);
    assert_transaction_result(&testkit.snapshot(), &tx, Error::UnknownSender as u8);

    let snapshot = testkit.snapshot();
    let schema = TimeSchema::new(INSTANCE_NAME, &snapshot);
    assert!(schema.time().get().is_none());
    assert!(schema.validators_times().get(&pub_key).is_none());
}

#[test]
fn test_transaction_time_less_than_validator_time_in_storage() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(1)
        .with_rust_service(TimeServiceInstance)
        .create();

    let validator = &testkit.network().validators().to_vec()[0];
    let (pub_key, sec_key) = validator.service_keypair();

    let time0 = Utc::now();
    let tx0 = TxTime { time: time0 }.sign(INSTANCE_ID, pub_key, &sec_key);

    testkit.create_block_with_transactions(txvec![tx0.clone()]);
    assert_eq!(
        Schema::new(&testkit.snapshot())
            .transaction_results()
            .get(&tx0.object_hash()),
        Some(ExecutionStatus::ok())
    );

    let schema = TimeSchema::new(INSTANCE_NAME, Rc::<dyn Snapshot>::from(testkit.snapshot()));

    assert_eq!(schema.time().get(), Some(time0));
    assert_eq!(schema.validators_times().get(&pub_key), Some(time0));

    let time1 = time0 - Duration::seconds(10);
    let tx1 = TxTime { time: time1 }.sign(INSTANCE_ID, pub_key, &sec_key);

    testkit.create_block_with_transactions(txvec![tx1.clone()]);
    assert_transaction_result(
        &testkit.snapshot(),
        &tx1,
        Error::ValidatorTimeIsGreater as u8,
    );

    let snapshot = testkit.snapshot();
    let schema = TimeSchema::new(INSTANCE_NAME, &snapshot);

    assert_eq!(schema.time().get(), Some(time0));
    assert_eq!(schema.validators_times().get(&pub_key), Some(time0));
}

fn get_current_time(api: &mut TestKitApi) -> Option<DateTime<Utc>> {
    api.public(ApiKind::Service(INSTANCE_NAME))
        .get("v1/current_time")
        .unwrap()
}

fn get_current_validators_times(api: &mut TestKitApi) -> Vec<ValidatorTime> {
    api.private(ApiKind::Service(INSTANCE_NAME))
        .get("v1/validators_times")
        .unwrap()
}

fn get_all_validators_times(api: &mut TestKitApi) -> Vec<ValidatorTime> {
    api.private(ApiKind::Service(INSTANCE_NAME))
        .get("v1/validators_times/all")
        .unwrap()
}

fn assert_current_time_eq(api: &mut TestKitApi, expected_time: Option<DateTime<Utc>>) {
    let current_time = get_current_time(api);
    assert_eq!(expected_time, current_time);
}

fn assert_current_validators_times_eq(
    api: &mut TestKitApi,
    expected_times: &HashMap<PublicKey, Option<DateTime<Utc>>>,
) {
    let validators_times = HashMap::from_iter(
        get_current_validators_times(api)
            .iter()
            .map(|validator| (validator.public_key, validator.time)),
    );

    assert_eq!(*expected_times, validators_times);
}

fn assert_all_validators_times_eq(
    api: &mut TestKitApi,
    expected_validators_times: &HashMap<PublicKey, Option<DateTime<Utc>>>,
) {
    let validators_times = HashMap::from_iter(
        get_all_validators_times(api)
            .iter()
            .map(|validator| (validator.public_key, validator.time)),
    );

    assert_eq!(*expected_validators_times, validators_times);
}

#[test]
fn test_endpoint_api() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(3)
        .with_rust_service(TimeServiceInstance)
        .with_rust_service(SimpleSupervisor)
        .create();

    let mut api = testkit.api();
    let validators = testkit.network().validators().to_vec();
    let mut current_validators_times: HashMap<PublicKey, Option<DateTime<Utc>>> =
        HashMap::from_iter(
            validators
                .iter()
                .map(|validator| (validator.service_keypair().0, None)),
        );
    let mut all_validators_times = HashMap::new();

    assert_current_time_eq(&mut api, None);
    assert_current_validators_times_eq(&mut api, &current_validators_times);
    assert_all_validators_times_eq(&mut api, &all_validators_times);

    let time0 = Utc::now();
    let (pub_key, sec_key) = validators[0].service_keypair();
    testkit.create_block_with_transactions(txvec![
        //
        TxTime { time: time0 }.sign(INSTANCE_ID, pub_key, &sec_key)
    ]);
    current_validators_times.insert(pub_key, Some(time0));
    all_validators_times.insert(pub_key, Some(time0));

    assert_current_time_eq(&mut api, Some(time0));
    assert_current_validators_times_eq(&mut api, &current_validators_times);
    assert_all_validators_times_eq(&mut api, &all_validators_times);

    let time1 = time0 + Duration::seconds(10);
    let (pub_key, sec_key) = validators[1].service_keypair();
    testkit.create_block_with_transactions(txvec![TxTime { time: time1 }.sign(
        INSTANCE_ID,
        pub_key,
        &sec_key
    )]);
    current_validators_times.insert(pub_key, Some(time1));
    all_validators_times.insert(pub_key, Some(time1));

    assert_current_time_eq(&mut api, Some(time1));
    assert_current_validators_times_eq(&mut api, &current_validators_times);
    assert_all_validators_times_eq(&mut api, &all_validators_times);

    let time2 = time1 + Duration::seconds(10);
    let (pub_key, sec_key) = validators[2].service_keypair();
    testkit.create_block_with_transactions(txvec![TxTime { time: time2 }.sign(
        INSTANCE_ID,
        pub_key,
        &sec_key
    )]);
    current_validators_times.insert(pub_key, Some(time2));
    all_validators_times.insert(pub_key, Some(time2));

    assert_current_time_eq(&mut api, Some(time2));
    assert_current_validators_times_eq(&mut api, &current_validators_times);
    assert_all_validators_times_eq(&mut api, &all_validators_times);

    let public_key_0 = validators[0].service_keypair().0;
    let cfg_change_height = Height(10);
    let new_cfg = {
        let mut cfg = testkit.consensus_config();
        cfg.validator_keys = vec![
            testkit.network_mut().add_node().public_keys(),
            validators[1].public_keys(),
            validators[2].public_keys(),
        ];
        cfg
    };
    testkit.create_block_with_transaction(
        ConfigPropose::actual_from(cfg_change_height)
            .consensus_config(new_cfg)
            .into_tx(),
    );
    testkit.create_blocks_until(cfg_change_height);

    current_validators_times.remove(&public_key_0);
    let validators = testkit.network().validators().to_vec();
    current_validators_times.insert(validators[0].service_keypair().0, None);

    let snapshot = testkit.snapshot();
    let schema = TimeSchema::new(INSTANCE_NAME, &snapshot);
    if let Some(time) = schema.validators_times().get(&public_key_0) {
        all_validators_times.insert(public_key_0, Some(time));
    }

    assert_current_time_eq(&mut api, Some(time2));
    assert_current_validators_times_eq(&mut api, &current_validators_times);
    assert_all_validators_times_eq(&mut api, &all_validators_times);

    let time3 = time2 + Duration::seconds(10);
    let (pub_key, sec_key) = validators[0].service_keypair();
    testkit.create_block_with_transactions(txvec![TxTime { time: time3 }.sign(
        INSTANCE_ID,
        pub_key,
        &sec_key
    )]);
    current_validators_times.insert(pub_key, Some(time3));
    all_validators_times.insert(pub_key, Some(time3));

    assert_current_time_eq(&mut api, Some(time3));
    assert_current_validators_times_eq(&mut api, &current_validators_times);
    assert_all_validators_times_eq(&mut api, &all_validators_times);
}
