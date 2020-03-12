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

use assert_matches::assert_matches;
use exonum::{
    helpers::Height,
    runtime::{InstanceId, InstanceStatus, SnapshotExt, SUPERVISOR_INSTANCE_ID},
};
use exonum_explorer::BlockchainExplorer;
use exonum_merkledb::{BinaryValue, ObjectHash};
use exonum_rust_runtime::{RustRuntime, ServiceFactory};
use exonum_testkit::{Spec, TestKitBuilder};
use pretty_assertions::assert_eq;

pub use crate::{
    hooks_service::{
        AfterCommitInterface, AfterCommitService, AfterCommitServiceV2, SERVICE_ID, SERVICE_NAME,
    },
    supervisor::{StartMigration, Supervisor, SupervisorInterface},
};

mod hooks_service;
mod supervisor;

const SUPERVISOR_ID: InstanceId = SUPERVISOR_INSTANCE_ID;

#[tokio::test]
async fn test_after_commit() {
    let service = AfterCommitService::new();
    let mut testkit = TestKitBuilder::validator()
        .with(Spec::new(service.clone()).with_default_instance())
        .build();

    // Check that `after_commit` invoked on the correct height.
    for i in 1..5 {
        let block = testkit.create_block();
        if i > 1 {
            let arguments = &block[0].message().payload().arguments;
            let height_from_tx = u64::from_bytes(arguments.into()).unwrap();
            assert_eq!(height_from_tx, i - 1);
        }

        assert_eq!(service.counter() as u64, i);

        let blockchain = testkit.blockchain();
        let keypair = blockchain.service_keypair();
        let tx = keypair.after_commit(SERVICE_ID, i);
        assert!(testkit.is_tx_in_pool(&tx.object_hash()));
    }

    let snapshot = testkit.snapshot();
    let expected_block_sizes = BlockchainExplorer::new(snapshot.as_ref())
        .blocks(Height(1)..)
        .all(|block| block.len() == if block.height() == Height(1) { 0 } else { 1 });
    assert!(expected_block_sizes);
}

/// An auditor should not broadcast transactions.
#[tokio::test]
async fn test_after_commit_with_auditor() {
    let service = AfterCommitService::new();
    let mut testkit = TestKitBuilder::auditor()
        .with_validators(2)
        .with(Spec::new(service.clone()).with_default_instance())
        .build();

    for i in 1..5 {
        let block = testkit.create_block();
        assert!(block.is_empty());
        assert_eq!(service.counter() as u64, i);

        let blockchain = testkit.blockchain();
        let keypair = blockchain.service_keypair();
        let tx = keypair.after_commit(SERVICE_ID, i);
        assert!(!testkit.is_tx_in_pool(&tx.object_hash()));
    }

    service.switch_to_generic_broadcast();
    for i in 0..5 {
        let block = testkit.create_block();
        let expected_block_len = if i == 0 { 0 } else { 1 };
        assert_eq!(block.len(), expected_block_len);
    }
}

#[tokio::test]
async fn after_commit_not_called_after_service_stop() {
    let service = AfterCommitService::new();
    let mut testkit = TestKitBuilder::validator()
        .with(Spec::new(Supervisor).with_default_instance())
        .with(Spec::new(service.clone()).with_default_instance())
        .build();
    service.switch_to_generic_broadcast();

    let keys = testkit.us().service_keypair();
    let tx = keys.stop_service(SUPERVISOR_ID, SERVICE_ID);
    let block = testkit.create_block_with_transaction(tx);
    block[0].status().expect("Service should stop");

    // Check that `after_commit` hook is not called for the stopped service.
    for _ in 0..5 {
        let block = testkit.create_block();
        assert!(block.is_empty());
    }

    // Resume the service and check that `after_commit` starts being called again.
    let tx = keys.resume_service(SUPERVISOR_ID, SERVICE_ID);
    let block = testkit.create_block_with_transaction(tx);
    block[0].status().expect("Service should resume");
    for _ in 0..5 {
        let block = testkit.create_block();
        assert_eq!(block.len(), 1);
    }
}

#[tokio::test]
async fn after_commit_during_service_freeze() {
    let service = AfterCommitService::new();
    let mut testkit = TestKitBuilder::validator()
        .with(Spec::new(Supervisor).with_default_instance())
        .with(Spec::new(service.clone()).with_default_instance())
        .build();

    let keys = testkit.us().service_keypair();
    let tx = keys.freeze_service(SUPERVISOR_ID, SERVICE_ID);
    let block = testkit.create_block_with_transaction(tx);
    block[0].status().expect("Service should freeze");

    // `broadcaster` used in `after_commit` hook by default is not exposed when the service
    // is frozen.
    for _ in 0..5 {
        let block = testkit.create_block();
        assert!(block.is_empty());
    }

    // Generic broadcast is switched off, too, due to transaction filtering within testkit.
    service.switch_to_generic_broadcast();
    for _ in 0..5 {
        let block = testkit.create_block();
        assert!(block.is_empty());
    }
}

#[tokio::test]
async fn after_commit_during_migration() {
    let service = AfterCommitService::new();
    let mut testkit = TestKitBuilder::validator()
        .with(Spec::new(Supervisor).with_default_instance())
        .with(Spec::new(service.clone()).with_default_instance())
        .with(Spec::migrating(AfterCommitServiceV2))
        .build();

    let keys = testkit.us().service_keypair();
    let tx = keys.stop_service(SUPERVISOR_ID, SERVICE_ID);
    let block = testkit.create_block_with_transaction(tx);
    block[0].status().expect("Service should stop");

    let tx = keys.start_migration(
        SUPERVISOR_ID,
        StartMigration {
            instance_id: SERVICE_ID,
            new_artifact: AfterCommitServiceV2.artifact_id(),
            migration_len: 10,
        },
    );
    let block = testkit.create_block_with_transaction(tx);
    block[0].status().expect("Service should start migrating");

    // As with frozen service, the ordinary broadcast should be switched off.
    for _ in 0..5 {
        let block = testkit.create_block();
        assert!(block.is_empty());

        let snapshot = testkit.snapshot();
        let service_state = snapshot.for_dispatcher().get_instance(SERVICE_ID).unwrap();
        assert_matches!(service_state.status, Some(InstanceStatus::Migrating(_)));
    }
    // Generic broadcast is switched off, too, due to transaction filtering within testkit.
    service.switch_to_generic_broadcast();
    for _ in 0..5 {
        let block = testkit.create_block();
        assert!(block.is_empty());

        let snapshot = testkit.snapshot();
        let service_state = snapshot.for_dispatcher().get_instance(SERVICE_ID).unwrap();
        assert_matches!(service_state.status, Some(InstanceStatus::Migrating(_)));
    }

    testkit.create_block();
    testkit.create_block();
    let snapshot = testkit.snapshot();
    let service_state = snapshot.for_dispatcher().get_instance(SERVICE_ID).unwrap();
    assert_matches!(service_state.status, Some(InstanceStatus::Stopped));
}

#[tokio::test]
async fn incorrect_txs_are_not_included_into_blocks() {
    let service = AfterCommitService::new();
    let mut testkit = TestKitBuilder::validator()
        .with(Spec::new(Supervisor).with_default_instance())
        .with(Spec::new(service).with_default_instance())
        .build();
    let keys = testkit.us().service_keypair();

    // Generate some transactions using the service, but do not commit them.
    for _ in 0..5 {
        let block = testkit.create_block_with_tx_hashes(&[]);
        assert!(block.is_empty());
        let new_tx = keys.after_commit(SERVICE_ID, testkit.height().0);
        assert!(testkit.is_tx_in_pool(&new_tx.object_hash()));
    }

    let tx = keys.freeze_service(SUPERVISOR_ID, SERVICE_ID);
    let block = testkit.create_block_with_transaction(tx);
    block[0].status().expect("Service should freeze");

    // Check that transactions in the pool are not committed while the service is frozen.
    let block = testkit.create_block();
    assert!(block.is_empty());

    // Resume the service.
    let tx = keys.resume_service(SUPERVISOR_ID, SERVICE_ID);
    let block = testkit.create_block_with_transaction(tx);
    block[0].status().expect("Service should resume");

    // Check that all previously created transactions have been committed.
    let block = testkit.create_block();
    assert_eq!(block.len(), 6); // 5 old transactions + 1 generated after resume
}

#[tokio::test]
async fn restart_testkit() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(3)
        .with(Spec::new(AfterCommitService::new()).with_default_instance())
        .build();
    testkit.create_blocks_until(Height(5));

    let stopped = testkit.stop();
    assert_eq!(stopped.height(), Height(5));
    assert_eq!(stopped.network().validators().len(), 3);
    let service = AfterCommitService::new();
    let rust_runtime = RustRuntime::builder().with_factory(service.clone());
    let mut testkit = stopped.resume(rust_runtime);
    for _ in 0..3 {
        testkit.create_block();
    }

    // The counter is controlled by the service instance and thus is *not* persistent
    // between reloads.
    assert_eq!(service.counter(), 3);

    // OTOH, the database state is fully persisted between reloads.
    assert_eq!(testkit.height(), Height(8));
    assert_eq!(testkit.network().validators().len(), 3);
    let transactions_are_committed = (1..=8)
        .map(|i| {
            testkit
                .blockchain()
                .service_keypair()
                .after_commit(SERVICE_ID, i)
                .object_hash()
        })
        .all(|hash| {
            let snapshot = testkit.snapshot();
            BlockchainExplorer::new(snapshot.as_ref())
                .transaction_without_proof(&hash)
                .is_some()
        });
    assert!(transactions_are_committed);
}

#[test]
fn tx_pool_is_retained_on_restart() {
    let mut testkit = TestKitBuilder::validator()
        .with(Spec::new(AfterCommitService::new()).with_default_instance())
        .build();

    let tx_hashes: Vec<_> = (100..105)
        .map(|i| {
            let tx = testkit
                .blockchain()
                .service_keypair()
                .after_commit(SERVICE_ID, i);
            let tx_hash = tx.object_hash();
            testkit.add_tx(tx);
            assert!(testkit.is_tx_in_pool(&tx_hash));
            tx_hash
        })
        .collect();

    let stopped = testkit.stop();
    let rust_runtime = RustRuntime::builder().with_factory(AfterCommitService::new());
    let testkit = stopped.resume(rust_runtime);
    assert!(tx_hashes
        .iter()
        .all(|tx_hash| testkit.is_tx_in_pool(tx_hash)));
}
