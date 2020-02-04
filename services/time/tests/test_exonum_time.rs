// Copyright 2020 The Exonum Team
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

use chrono::{DateTime, Duration, TimeZone, Utc};
use exonum::{
    crypto::{KeyPair, PublicKey},
    helpers::Height,
    merkledb::{access::Access, Snapshot},
    runtime::{CommonError, ErrorMatch, InstanceId, SnapshotExt, SUPERVISOR_INSTANCE_ID},
};
use exonum_rust_runtime::ServiceFactory;
use exonum_supervisor::{ConfigPropose, Supervisor, SupervisorInterface};
use exonum_testkit::{ApiKind, TestKit, TestKitApi, TestKitBuilder, TestNode};
use pretty_assertions::assert_eq;

use std::{collections::HashMap, iter::FromIterator};

use exonum_time::{
    Error, MockTimeProvider, TimeOracleInterface, TimeSchema, TimeServiceFactory, TxTime,
    ValidatorTime,
};

const INSTANCE_ID: InstanceId = 112;
const INSTANCE_NAME: &str = "my-time";

fn get_schema<'a>(snapshot: &'a dyn Snapshot) -> TimeSchema<impl Access + 'a> {
    snapshot.service_schema(INSTANCE_NAME).unwrap()
}

fn assert_storage_times_eq(
    snapshot: &dyn Snapshot,
    validators: &[TestNode],
    expected_current_time: Option<DateTime<Utc>>,
    expected_validators_times: &[Option<DateTime<Utc>>],
) {
    let schema = get_schema(snapshot);
    assert_eq!(schema.time.get(), expected_current_time);

    for (i, validator) in validators.iter().enumerate() {
        let public_key = &validator.public_keys().service_key;
        assert_eq!(
            schema.validators_times.get(&public_key),
            expected_validators_times[i]
        );
    }
}

#[test]
fn test_exonum_time_service_with_3_validators() {
    let mut testkit = create_testkit_with_validators(3);
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
    let tx0 = validators[0]
        .service_keypair()
        .report_time(INSTANCE_ID, TxTime::new(time0));
    testkit.create_block_with_transaction(tx0);

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
    let tx1 = validators[1]
        .service_keypair()
        .report_time(INSTANCE_ID, TxTime::new(time1));
    testkit.create_block_with_transaction(tx1);

    assert_storage_times_eq(
        &testkit.snapshot(),
        &validators,
        Some(time1),
        &[Some(time0), Some(time1), None],
    );
}

#[test]
fn test_exonum_time_service_with_4_validators() {
    let mut testkit = create_testkit_with_validators(4);
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
    let tx0 = validators[0]
        .service_keypair()
        .report_time(INSTANCE_ID, TxTime::new(time0));
    testkit.create_block_with_transaction(tx0);

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
    let tx1 = validators[1]
        .service_keypair()
        .report_time(INSTANCE_ID, TxTime::new(time1));
    testkit.create_block_with_transaction(tx1);

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
    let tx2 = validators[2]
        .service_keypair()
        .report_time(INSTANCE_ID, TxTime::new(time2));
    testkit.create_block_with_transaction(tx2);

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
    let tx3 = validators[3]
        .service_keypair()
        .report_time(INSTANCE_ID, TxTime::new(time3));
    testkit.create_block_with_transaction(tx3);

    assert_storage_times_eq(
        &testkit.snapshot(),
        &validators,
        Some(time2),
        &[Some(time0), Some(time1), Some(time2), Some(time3)],
    );
}

#[test]
fn test_exonum_time_service_with_7_validators() {
    let mut testkit = create_testkit_with_validators(7);
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
        let tx = validator
            .service_keypair()
            .report_time(INSTANCE_ID, TxTime::new(times[i]));
        let block = testkit.create_block_with_transaction(tx);
        block[0].status().unwrap();

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
    let time_service = TimeServiceFactory::with_provider(mock_provider.clone());
    let artifact = time_service.artifact_id();
    let mut testkit = TestKitBuilder::validator()
        .with_artifact(artifact.clone())
        .with_instance(artifact.into_default_instance(INSTANCE_ID, INSTANCE_NAME))
        .with_rust_service(time_service)
        .build();

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
    let time_service = TimeServiceFactory::default();
    let artifact = time_service.artifact_id();
    let mut testkit = TestKitBuilder::validator()
        .with_validators(1)
        .with_artifact(artifact.clone())
        .with_instance(artifact.into_default_instance(INSTANCE_ID, INSTANCE_NAME))
        .with_rust_service(time_service)
        .with_rust_service(Supervisor)
        .with_artifact(Supervisor.artifact_id())
        .with_instance(Supervisor::simple())
        .build();

    let validators = testkit.network().validators().to_vec();
    let old_keypair = validators[0].service_keypair();

    let cfg_change_height = Height(5);
    let new_cfg = {
        let validator_keys = vec![testkit.network_mut().add_node().public_keys()];
        testkit
            .consensus_config()
            .with_validator_keys(validator_keys)
    };

    let change = ConfigPropose::new(0, cfg_change_height).consensus_config(new_cfg);
    let change = old_keypair.propose_config_change(SUPERVISOR_INSTANCE_ID, change);
    testkit.create_block_with_transaction(change);
    testkit.create_blocks_until(cfg_change_height);

    let validators = testkit.network().validators();
    let new_keypair = validators[0].service_keypair();
    let snapshot = testkit.snapshot();
    let schema = get_schema(&snapshot);

    assert!(schema.time.get().is_some());
    assert!(schema
        .validators_times
        .get(&old_keypair.public_key())
        .is_some());
    assert!(schema
        .validators_times
        .get(&new_keypair.public_key())
        .is_none());
    assert_eq!(
        schema.time.get(),
        schema.validators_times.get(&old_keypair.public_key())
    );

    if let Some(time_in_storage) = schema.time.get() {
        let time_tx = time_in_storage - Duration::seconds(10);
        let tx = new_keypair.report_time(INSTANCE_ID, TxTime::new(time_tx));
        let block = testkit.create_block_with_transaction(tx);
        block[0].status().unwrap();
    }

    let snapshot = testkit.snapshot();
    let schema = get_schema(&snapshot);
    assert!(schema.time.get().is_some());
    assert!(schema
        .validators_times
        .get(&old_keypair.public_key())
        .is_some());
    assert!(schema
        .validators_times
        .get(&new_keypair.public_key())
        .is_some());
    assert_eq!(
        schema.time.get(),
        schema.validators_times.get(&old_keypair.public_key())
    );
}

#[test]
fn test_creating_transaction_is_not_validator() {
    let mut testkit = create_testkit_with_validators(1);

    let keypair = KeyPair::random();
    let tx = keypair.report_time(INSTANCE_ID, TxTime::new(Utc::now()));
    let block = testkit.create_block_with_transaction(tx);
    assert_eq!(
        *block[0].status().unwrap_err(),
        ErrorMatch::from_fail(&CommonError::UnauthorizedCaller).for_service(INSTANCE_ID)
    );

    let snapshot = testkit.snapshot();
    let schema = get_schema(&snapshot);
    assert!(schema.time.get().is_none());
    assert!(schema.validators_times.get(&keypair.public_key()).is_none());
}

#[test]
fn test_transaction_time_less_than_validator_time_in_storage() {
    let mut testkit = create_testkit_with_validators(1);
    let validator = testkit.network().validators()[0].service_keypair();

    let time0 = Utc::now();
    let tx0 = validator.report_time(INSTANCE_ID, TxTime::new(time0));
    let block = testkit.create_block_with_transaction(tx0);
    block[0].status().unwrap();

    let snapshot = testkit.snapshot();
    let schema = get_schema(&snapshot);
    assert_eq!(schema.time.get(), Some(time0));
    assert_eq!(
        schema.validators_times.get(&validator.public_key()),
        Some(time0)
    );

    let time1 = time0 - Duration::seconds(10);
    let tx1 = validator.report_time(INSTANCE_ID, TxTime::new(time1));
    let block = testkit.create_block_with_transaction(tx1);
    assert_eq!(
        *block[0].status().unwrap_err(),
        ErrorMatch::from_fail(&Error::ValidatorTimeIsGreater).for_service(INSTANCE_ID),
    );

    let snapshot = testkit.snapshot();
    let schema = get_schema(&snapshot);
    assert_eq!(schema.time.get(), Some(time0));
    assert_eq!(
        schema.validators_times.get(&validator.public_key()),
        Some(time0)
    );
}

fn create_testkit_with_validators(validators_count: u16) -> TestKit {
    let time_service = TimeServiceFactory::default();
    let artifact = time_service.artifact_id();
    TestKitBuilder::validator()
        .with_validators(validators_count)
        .with_artifact(artifact.clone())
        .with_instance(artifact.into_default_instance(INSTANCE_ID, INSTANCE_NAME))
        .with_rust_service(time_service)
        .build()
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
    let time_service = TimeServiceFactory::default();
    let artifact = time_service.artifact_id();
    let mut testkit = TestKitBuilder::validator()
        .with_validators(3)
        .with_artifact(artifact.clone())
        .with_instance(artifact.into_default_instance(INSTANCE_ID, INSTANCE_NAME))
        .with_rust_service(time_service)
        .with_rust_service(Supervisor)
        .with_artifact(Supervisor.artifact_id())
        .with_instance(Supervisor::simple())
        .build();

    let mut api = testkit.api();
    let validators = testkit.network().validators().to_vec();
    let mut current_validators_times: HashMap<_, _> = HashMap::from_iter(
        validators
            .iter()
            .map(|validator| (validator.service_keypair().public_key(), None)),
    );
    let mut all_validators_times = HashMap::new();

    assert_current_time_eq(&mut api, None);
    assert_current_validators_times_eq(&mut api, &current_validators_times);
    assert_all_validators_times_eq(&mut api, &all_validators_times);

    let time0 = Utc::now();
    let keypair = validators[0].service_keypair();
    let tx = keypair.report_time(INSTANCE_ID, TxTime::new(time0));
    testkit.create_block_with_transaction(tx);
    current_validators_times.insert(keypair.public_key(), Some(time0));
    all_validators_times.insert(keypair.public_key(), Some(time0));

    assert_current_time_eq(&mut api, Some(time0));
    assert_current_validators_times_eq(&mut api, &current_validators_times);
    assert_all_validators_times_eq(&mut api, &all_validators_times);

    let time1 = time0 + Duration::seconds(10);
    let keypair = validators[1].service_keypair();
    let tx = keypair.report_time(INSTANCE_ID, TxTime::new(time1));
    testkit.create_block_with_transaction(tx);
    current_validators_times.insert(keypair.public_key(), Some(time1));
    all_validators_times.insert(keypair.public_key(), Some(time1));

    assert_current_time_eq(&mut api, Some(time1));
    assert_current_validators_times_eq(&mut api, &current_validators_times);
    assert_all_validators_times_eq(&mut api, &all_validators_times);

    let time2 = time1 + Duration::seconds(10);
    let keypair = validators[2].service_keypair();
    let tx = keypair.report_time(INSTANCE_ID, TxTime::new(time2));
    testkit.create_block_with_transaction(tx);
    current_validators_times.insert(keypair.public_key(), Some(time2));
    all_validators_times.insert(keypair.public_key(), Some(time2));

    assert_current_time_eq(&mut api, Some(time2));
    assert_current_validators_times_eq(&mut api, &current_validators_times);
    assert_all_validators_times_eq(&mut api, &all_validators_times);

    let cfg_change_height = Height(10);
    let new_cfg = {
        let validator_keys = vec![
            testkit.network_mut().add_node().public_keys(),
            validators[1].public_keys(),
            validators[2].public_keys(),
        ];
        testkit
            .consensus_config()
            .with_validator_keys(validator_keys)
    };
    let change = ConfigPropose::new(0, cfg_change_height).consensus_config(new_cfg);
    let keypair = validators[0].service_keypair();
    let change = keypair.propose_config_change(SUPERVISOR_INSTANCE_ID, change);
    testkit.create_block_with_transaction(change);
    testkit.create_blocks_until(cfg_change_height);

    current_validators_times.remove(&keypair.public_key());
    let validators = testkit.network().validators().to_vec();
    current_validators_times.insert(validators[0].service_keypair().public_key(), None);

    let snapshot = testkit.snapshot();
    let schema = get_schema(&snapshot);
    if let Some(time) = schema.validators_times.get(&keypair.public_key()) {
        all_validators_times.insert(keypair.public_key(), Some(time));
    }

    assert_current_time_eq(&mut api, Some(time2));
    assert_current_validators_times_eq(&mut api, &current_validators_times);
    assert_all_validators_times_eq(&mut api, &all_validators_times);

    let time3 = time2 + Duration::seconds(10);
    let keypair = validators[0].service_keypair();
    let tx = keypair.report_time(INSTANCE_ID, TxTime::new(time3));
    testkit.create_block_with_transaction(tx);
    current_validators_times.insert(keypair.public_key(), Some(time3));
    all_validators_times.insert(keypair.public_key(), Some(time3));

    assert_current_time_eq(&mut api, Some(time3));
    assert_current_validators_times_eq(&mut api, &current_validators_times);
    assert_all_validators_times_eq(&mut api, &all_validators_times);
}
