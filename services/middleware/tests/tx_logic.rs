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

//! Tests related to transaction logic.

use exonum::{
    crypto::gen_keypair,
    runtime::{
        rust::{DefaultInstance, ServiceFactory, TxStub},
        DispatcherError, ErrorMatch, InstanceId, SnapshotExt,
    },
};
use exonum_testkit::{TestKit, TestKitBuilder};
use semver::Version;

use exonum_middleware_service::{
    Batch, CheckedCall, Error as TxError, MiddlewareInterface, MiddlewareService,
};

mod inc;
use crate::inc::{IncFactory, IncInterface, IncSchema};

const INSTANCE_ID: InstanceId = MiddlewareService::INSTANCE_ID;

fn create_testkit(inc_versions: Vec<Version>) -> TestKit {
    let mut builder = TestKitBuilder::validator().with_default_rust_service(MiddlewareService);
    for (i, version) in inc_versions.into_iter().enumerate() {
        let service_factory = IncFactory::new(version);
        builder = builder
            .with_artifact(service_factory.artifact_id())
            .with_instance(
                service_factory
                    .artifact_id()
                    .into_default_instance(100 + i as InstanceId, format!("inc-{}", i)),
            )
            .with_rust_service(service_factory);
    }
    builder.create()
}

#[test]
fn checked_call_normal_workflow() {
    let mut testkit = create_testkit(vec!["1.1.3".parse().unwrap()]);
    let service_id = 100;
    let mut checked_call = CheckedCall {
        artifact_name: IncFactory::ARTIFACT_NAME.to_owned(),
        artifact_version: "1".parse().unwrap(),
        inner: TxStub.increment(service_id, 0),
    };

    // All versions match the installed predicate version.
    const VERSIONS: &[&str] = &[
        "1",
        "1.1",
        "1.*",
        "^1.0.0",
        "^1.1.0",
        "~1.1.0",
        "=1.1.3",
        ">=1.1.2, <1.5",
        "^1.0.0, <=1.1.4",
        "<2",
    ];

    let keypair = gen_keypair();
    for (i, &version) in VERSIONS.iter().enumerate() {
        checked_call.artifact_version = version
            .parse()
            .unwrap_or_else(|e| panic!("version req = {}: {}", version, e));

        let signed = keypair.checked_call(INSTANCE_ID, checked_call.clone());
        let block = testkit.create_block_with_transaction(signed);
        block[0]
            .status()
            .unwrap_or_else(|e| panic!("version req = {}: {}", version, e));
        let snapshot = testkit.snapshot();
        assert_eq!(
            IncSchema::new(snapshot.for_service(100).unwrap())
                .counts
                .get(&keypair.0),
            Some(i as u64 + 1),
            "version req = {}",
            version
        );
    }
}

#[test]
fn checked_call_for_non_existing_service() {
    let mut testkit = create_testkit(vec!["1.1.3".parse().unwrap()]);
    let bogus_service_id = 200;
    let checked_call = CheckedCall {
        artifact_name: IncFactory::ARTIFACT_NAME.to_owned(),
        artifact_version: "1".parse().unwrap(),
        inner: TxStub.increment(bogus_service_id, 0),
    };
    let keypair = gen_keypair();
    let checked_call = keypair.checked_call(INSTANCE_ID, checked_call);

    let block = testkit.create_block_with_transaction(checked_call);
    let err = block[0].status().unwrap_err();
    assert_eq!(
        *err,
        ErrorMatch::from_fail(&DispatcherError::IncorrectInstanceId)
    );
}

#[test]
fn checked_call_with_mismatched_artifact() {
    let mut testkit = create_testkit(vec!["1.1.3".parse().unwrap()]);
    let service_id = 100;
    let checked_call = CheckedCall {
        artifact_name: "bogus_artifact".to_owned(),
        artifact_version: "1".parse().unwrap(),
        inner: TxStub.increment(service_id, 0),
    };
    let checked_call = gen_keypair().checked_call(INSTANCE_ID, checked_call);

    let block = testkit.create_block_with_transaction(checked_call);
    let err = block[0].status().unwrap_err();
    assert_eq!(*err, ErrorMatch::from_fail(&TxError::ArtifactMismatch));
}

#[test]
fn checked_call_with_mismatched_version() {
    let mut testkit = create_testkit(vec!["1.1.3".parse().unwrap()]);
    let service_id = 100;
    let mut checked_call = CheckedCall {
        artifact_name: IncFactory::ARTIFACT_NAME.to_owned(),
        artifact_version: "1".parse().unwrap(),
        inner: TxStub.increment(service_id, 0),
    };

    // All versions match the installed predicate version.
    const VERSIONS: &[&str] = &[
        "2",
        "1.2",
        "2.*",
        "^1.3.0",
        "~1.0.0",
        "=1.1.2",
        ">=1.1.4, <1.5",
        "^1.0.0, <=1.1.2",
        "<1",
        ">=2",
    ];

    let keypair = gen_keypair();
    for &version in VERSIONS {
        checked_call.artifact_version = version
            .parse()
            .unwrap_or_else(|e| panic!("version req = {}: {}", version, e));

        let signed = keypair.checked_call(INSTANCE_ID, checked_call.clone());
        let block = testkit.create_block_with_transaction(signed);
        let err = block[0].status().unwrap_err();
        assert_eq!(*err, ErrorMatch::from_fail(&TxError::VersionMismatch));
    }
}

#[test]
fn batch_normal_workflow() {
    let mut testkit = create_testkit(vec!["1.1.3".parse().unwrap()]);
    let service_id = 100;
    let checked_call = CheckedCall {
        artifact_name: IncFactory::ARTIFACT_NAME.to_owned(),
        artifact_version: "^1.0.0".parse().unwrap(),
        inner: TxStub.increment(service_id, 0),
    };
    let batch = Batch::new()
        .with_call(TxStub.increment(service_id, 0))
        .with_call(TxStub.increment(service_id, 0))
        .with_call(TxStub.checked_call(INSTANCE_ID, checked_call));

    let keypair = gen_keypair();
    let batch = keypair.batch(INSTANCE_ID, batch);
    let block = testkit.create_block_with_transaction(batch);
    block[0].status().unwrap();

    let snapshot = testkit.snapshot();
    let schema = IncSchema::new(snapshot.for_service(100).unwrap());
    assert_eq!(schema.counts.get(&keypair.0), Some(3));
}

#[test]
fn batch_with_calls_to_different_services() {
    let mut testkit = create_testkit(vec![
        "1.1.3".parse().unwrap(),
        "2.0.0-rc.0".parse().unwrap(),
    ]);
    let service_id = 100;
    let inner_batch = Batch::new()
        .with_call(TxStub.increment(service_id, 0))
        .with_call(TxStub.increment(service_id + 1, 0))
        .with_call(TxStub.increment(service_id, 0));
    let batch = Batch::new()
        .with_call(TxStub.increment(service_id, 1))
        .with_call(TxStub.batch(INSTANCE_ID, inner_batch))
        .with_call(TxStub.increment(service_id + 1, 1));

    let keypair = gen_keypair();
    let batch = keypair.batch(INSTANCE_ID, batch);
    let block = testkit.create_block_with_transaction(batch);
    block[0].status().unwrap();

    let snapshot = testkit.snapshot();
    let schema = IncSchema::new(snapshot.for_service(service_id).unwrap());
    assert_eq!(schema.counts.get(&keypair.0), Some(3));
    let schema = IncSchema::new(snapshot.for_service(service_id + 1).unwrap());
    assert_eq!(schema.counts.get(&keypair.0), Some(2));
}

#[test]
fn batch_with_call_to_non_existing_service() {
    let mut testkit = create_testkit(vec!["1.1.3".parse().unwrap()]);
    let service_id = 100;
    let batch = Batch::new()
        .with_call(TxStub.increment(service_id, 0))
        .with_call(TxStub.increment(service_id + 1, 0)) // <- service doesn't exist
        .with_call(TxStub.increment(service_id, 0));

    let keypair = gen_keypair();
    let batch = keypair.batch(INSTANCE_ID, batch);
    let block = testkit.create_block_with_transaction(batch);
    let err = block[0].status().unwrap_err();
    assert_eq!(
        *err,
        ErrorMatch::from_fail(&DispatcherError::IncorrectInstanceId)
    );

    let snapshot = testkit.snapshot();
    let schema = IncSchema::new(snapshot.for_service(100).unwrap());
    assert_eq!(schema.counts.get(&keypair.0), None);
}

#[test]
fn batch_with_service_error() {
    let mut testkit = create_testkit(vec!["1.1.3".parse().unwrap()]);
    let service_id = 100;
    let checked_call = CheckedCall {
        artifact_name: "bogus-artifact".to_owned(),
        artifact_version: "*".parse().unwrap(),
        inner: TxStub.increment(service_id, 0),
    };
    let batch = Batch::new()
        .with_call(TxStub.increment(service_id, 0))
        .with_call(TxStub.checked_call(INSTANCE_ID, checked_call))
        .with_call(TxStub.increment(service_id, 0));

    let keypair = gen_keypair();
    let batch = keypair.batch(INSTANCE_ID, batch);
    let block = testkit.create_block_with_transaction(batch);
    let err = block[0].status().unwrap_err();
    assert_eq!(*err, ErrorMatch::from_fail(&TxError::ArtifactMismatch));

    let snapshot = testkit.snapshot();
    let schema = IncSchema::new(snapshot.for_service(100).unwrap());
    assert_eq!(schema.counts.get(&keypair.0), None);
}
