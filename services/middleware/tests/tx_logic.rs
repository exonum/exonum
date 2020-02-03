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

//! Tests related to transaction logic.

use exonum::{
    crypto::KeyPair,
    merkledb::{access::Access, Snapshot},
    runtime::{versioning::Version, CoreError, ErrorMatch, InstanceId, SnapshotExt},
};
use exonum_rust_runtime::{DefaultInstance, ServiceFactory, TxStub};
use exonum_testkit::{TestKit, TestKitBuilder};

use exonum_middleware_service::{
    ArtifactReq, Batch, Error as TxError, MiddlewareInterface, MiddlewareInterfaceMut,
    MiddlewareService,
};

mod inc;
use crate::inc::{IncFactory, IncInterface, IncInterfaceMut, IncSchema};

const MIDDLEWARE_ID: InstanceId = MiddlewareService::INSTANCE_ID;
const INC_ID: InstanceId = 100;

fn create_testkit(inc_versions: Vec<Version>) -> TestKit {
    let mut builder = TestKitBuilder::validator().with_default_rust_service(MiddlewareService);
    for (i, version) in inc_versions.into_iter().enumerate() {
        let service_factory = IncFactory::new(version);
        builder = builder
            .with_artifact(service_factory.artifact_id())
            .with_instance(
                service_factory
                    .artifact_id()
                    .into_default_instance(INC_ID + i as InstanceId, format!("inc-{}", i)),
            )
            .with_rust_service(service_factory);
    }
    builder.build()
}

fn inc_schema<'a>(
    snapshot: &'a dyn Snapshot,
    service_id: InstanceId,
) -> IncSchema<impl Access + 'a> {
    snapshot.service_schema(service_id).unwrap()
}

#[test]
fn checked_call_normal_workflow() {
    let mut testkit = create_testkit(vec!["1.1.3".parse().unwrap()]);

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

    let keypair = KeyPair::random();
    for (i, &version) in VERSIONS.iter().enumerate() {
        let checked_call = IncFactory::req(version).increment(INC_ID, 0);
        let signed = keypair.checked_call(MIDDLEWARE_ID, checked_call);
        let block = testkit.create_block_with_transaction(signed);
        block[0]
            .status()
            .unwrap_or_else(|e| panic!("version req = {}: {}", version, e));
        let snapshot = testkit.snapshot();
        assert_eq!(
            inc_schema(&snapshot, INC_ID)
                .counts
                .get(&keypair.public_key()),
            Some(i as u64 + 1),
            "version req = {}",
            version
        );
    }
}

#[test]
fn checked_call_for_non_existing_service() {
    let mut testkit = create_testkit(vec!["1.1.3".parse().unwrap()]);
    let checked_call = IncFactory::req("1").increment(INC_ID + 100, 0);
    let keypair = KeyPair::random();
    let checked_call = keypair.checked_call(MIDDLEWARE_ID, checked_call);

    let block = testkit.create_block_with_transaction(checked_call);
    let err = block[0].status().unwrap_err();
    assert_eq!(*err, ErrorMatch::from_fail(&CoreError::IncorrectInstanceId));
}

#[test]
fn checked_call_with_mismatched_artifact() {
    let mut testkit = create_testkit(vec!["1.1.3".parse().unwrap()]);
    let checked_call = "bogus-artifact@1"
        .parse::<ArtifactReq>()
        .unwrap()
        .increment(INC_ID, 0);

    let signed = KeyPair::random().checked_call(MIDDLEWARE_ID, checked_call);
    let block = testkit.create_block_with_transaction(signed);
    let err = block[0].status().unwrap_err();
    assert_eq!(*err, ErrorMatch::from_fail(&TxError::ArtifactMismatch));
}

#[test]
fn checked_call_with_mismatched_version() {
    let mut testkit = create_testkit(vec!["1.1.3".parse().unwrap()]);

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

    let keypair = KeyPair::random();
    for &version in VERSIONS {
        let checked_call = IncFactory::req(version).increment(INC_ID, 0);
        let signed = keypair.checked_call(MIDDLEWARE_ID, checked_call.clone());
        let block = testkit.create_block_with_transaction(signed);
        let err = block[0].status().unwrap_err();
        assert_eq!(*err, ErrorMatch::from_fail(&TxError::VersionMismatch));
    }
}

#[test]
fn batch_normal_workflow() {
    let mut testkit = create_testkit(vec!["1.1.3".parse().unwrap()]);

    let checked_call = IncFactory::req("^1.0.0").increment(INC_ID, 0);
    let mut batch = Batch::new();
    batch.increment(INC_ID, 0);
    batch.increment(INC_ID, 0);
    batch.checked_call(MIDDLEWARE_ID, checked_call);
    let keypair = KeyPair::random();
    let batch = keypair.batch(MIDDLEWARE_ID, batch);
    let block = testkit.create_block_with_transaction(batch);
    block[0].status().unwrap();

    let snapshot = testkit.snapshot();
    let schema = inc_schema(&snapshot, INC_ID);
    assert_eq!(schema.counts.get(&keypair.public_key()), Some(3));
}

#[test]
fn batch_with_calls_to_different_services() {
    let mut testkit = create_testkit(vec!["1.1.3".parse().unwrap(), "2.0.0".parse().unwrap()]);
    let mut inner_batch = Batch::new();
    inner_batch.increment(INC_ID, 0);
    inner_batch.increment(INC_ID + 1, 0);
    inner_batch.increment(INC_ID, 0);
    let mut batch = Batch::new();
    batch.increment(INC_ID, 1);
    batch.batch(MIDDLEWARE_ID, inner_batch);
    batch.increment(INC_ID + 1, 1);

    let keypair = KeyPair::random();
    let signed = keypair.batch(MIDDLEWARE_ID, batch);
    let block = testkit.create_block_with_transaction(signed);
    block[0].status().unwrap();

    let snapshot = testkit.snapshot();
    let schema = inc_schema(&snapshot, INC_ID);
    assert_eq!(schema.counts.get(&keypair.public_key()), Some(3));
    let schema = inc_schema(&snapshot, INC_ID + 1);
    assert_eq!(schema.counts.get(&keypair.public_key()), Some(2));
}

#[test]
fn batch_with_call_to_non_existing_service() {
    let mut testkit = create_testkit(vec!["1.1.3".parse().unwrap()]);
    let batch = Batch::new()
        .with_call(TxStub.increment(INC_ID, 0))
        .with_call(TxStub.increment(INC_ID + 1, 0)) // <- service doesn't exist
        .with_call(TxStub.increment(INC_ID, 0));

    let keypair = KeyPair::random();
    let batch = keypair.batch(MIDDLEWARE_ID, batch);
    let block = testkit.create_block_with_transaction(batch);
    let err = block[0].status().unwrap_err();
    assert_eq!(*err, ErrorMatch::from_fail(&CoreError::IncorrectInstanceId));

    let snapshot = testkit.snapshot();
    let schema = inc_schema(&snapshot, INC_ID);
    assert_eq!(schema.counts.get(&keypair.public_key()), None);
}

#[test]
fn batch_with_service_error() {
    let mut testkit = create_testkit(vec!["1.1.3".parse().unwrap()]);
    let checked_call = "bogus-artifact@*"
        .parse::<ArtifactReq>()
        .unwrap()
        .increment(INC_ID, 0);
    let mut batch = Batch::new();
    batch.increment(INC_ID, 0);
    batch.checked_call(MIDDLEWARE_ID, checked_call);
    batch.increment(INC_ID, 0);

    let keypair = KeyPair::random();
    let signed = keypair.batch(MIDDLEWARE_ID, batch);
    let block = testkit.create_block_with_transaction(signed);
    let err = block[0].status().unwrap_err();
    assert_eq!(
        *err,
        ErrorMatch::from_fail(&TxError::ArtifactMismatch).for_service(MIDDLEWARE_ID)
    );

    let snapshot = testkit.snapshot();
    let schema = inc_schema(&snapshot, INC_ID);
    assert_eq!(schema.counts.get(&keypair.public_key()), None);
}
