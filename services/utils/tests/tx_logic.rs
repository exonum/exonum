//! Tests related to transaction logic.

use exonum::{
    blockchain::InstanceCollection,
    crypto::gen_keypair,
    merkledb::BinaryValue,
    runtime::{rust::Transaction, AnyTx, CallInfo, InstanceId, SnapshotExt},
};
use exonum_testkit::{TestKit, TestKitBuilder};
use semver::Version;

use exonum_utils_service::{CheckedCall, Error as TxError, UtilsService};

mod inc;
use crate::inc::{Inc, IncFactory, IncSchema};

fn create_testkit(inc_versions: Vec<Version>) -> TestKit {
    let mut builder = TestKitBuilder::validator().with_rust_service(UtilsService);
    for (i, version) in inc_versions.into_iter().enumerate() {
        let service = InstanceCollection::new(IncFactory::new(version)).with_instance(
            100 + i as InstanceId,
            format!("inc-{}", i),
            (),
        );
        builder = builder.with_rust_service(service);
    }
    builder.create()
}

#[test]
fn normal_workflow() {
    let mut testkit = create_testkit(vec!["1.1.3".parse().unwrap()]);
    let mut checked_call = CheckedCall {
        artifact_name: IncFactory::ARTIFACT_NAME.to_owned(),
        artifact_version: "1".parse().unwrap(),
        inner: AnyTx {
            call_info: CallInfo::new(100, 0),
            arguments: Inc::new(0).into_bytes(),
        },
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

    let (pk, sk) = gen_keypair();
    for (i, &version) in VERSIONS.iter().enumerate() {
        checked_call.artifact_version = version
            .parse()
            .unwrap_or_else(|e| panic!("version req = {}: {}", version, e));

        let signed = checked_call.clone().sign(UtilsService::DEFAULT_ID, pk, &sk);
        let block = testkit.create_block_with_transaction(signed);
        block[0]
            .status()
            .unwrap_or_else(|e| panic!("version req = {}: {}", version, e));
        let snapshot = testkit.snapshot();
        assert_eq!(
            IncSchema::new(snapshot.for_service(100).unwrap())
                .counts
                .get(&pk),
            Some(i as u64 + 1),
            "version req = {}",
            version
        );
    }
}

#[test]
fn non_existing_service() {
    let mut testkit = create_testkit(vec!["1.1.3".parse().unwrap()]);
    let checked_call = CheckedCall {
        artifact_name: IncFactory::ARTIFACT_NAME.to_owned(),
        artifact_version: "1".parse().unwrap(),
        inner: AnyTx {
            call_info: CallInfo::new(200, 0),
            arguments: Inc::new(0).into_bytes(),
        },
    };
    let (pk, sk) = gen_keypair();
    let checked_call = checked_call.sign(UtilsService::DEFAULT_ID, pk, &sk);

    let block = testkit.create_block_with_transaction(checked_call);
    let err = block[0].status().unwrap_err();
    assert_eq!(*err, TxError::NoService.into());
}

#[test]
fn service_with_mismatched_artifact() {
    let mut testkit = create_testkit(vec!["1.1.3".parse().unwrap()]);
    let checked_call = CheckedCall {
        artifact_name: "bogus_artifact".to_owned(),
        artifact_version: "1".parse().unwrap(),
        inner: AnyTx {
            call_info: CallInfo::new(100, 0),
            arguments: Inc::new(0).into_bytes(),
        },
    };
    let (pk, sk) = gen_keypair();
    let checked_call = checked_call.sign(UtilsService::DEFAULT_ID, pk, &sk);

    let block = testkit.create_block_with_transaction(checked_call);
    let err = block[0].status().unwrap_err();
    assert_eq!(*err, TxError::ArtifactMismatch.into());
}

#[test]
fn service_with_mismatched_version() {
    let mut testkit = create_testkit(vec!["1.1.3".parse().unwrap()]);
    let mut checked_call = CheckedCall {
        artifact_name: IncFactory::ARTIFACT_NAME.to_owned(),
        artifact_version: "1".parse().unwrap(),
        inner: AnyTx {
            call_info: CallInfo::new(100, 0),
            arguments: Inc::new(0).into_bytes(),
        },
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

    let (pk, sk) = gen_keypair();
    for &version in VERSIONS {
        checked_call.artifact_version = version
            .parse()
            .unwrap_or_else(|e| panic!("version req = {}: {}", version, e));

        let signed = checked_call.clone().sign(UtilsService::DEFAULT_ID, pk, &sk);
        let block = testkit.create_block_with_transaction(signed);
        let err = block[0].status().unwrap_err();
        assert_eq!(*err, TxError::VersionMismatch.into());
    }
}
