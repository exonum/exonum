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

// HACK: Silent "dead_code" warning.
pub use crate::hooks::{AfterCommitInterface, AfterCommitService, SERVICE_ID, SERVICE_NAME};

use exonum::helpers::Height;
use exonum_explorer::BlockchainExplorer;
use exonum_merkledb::{BinaryValue, ObjectHash};
use exonum_rust_runtime::RustRuntime;
use exonum_testkit::TestKitBuilder;
use pretty_assertions::assert_eq;

mod hooks;

#[test]
fn test_after_commit() {
    let service = AfterCommitService::new();
    let mut testkit = TestKitBuilder::validator()
        .with_default_rust_service(service.clone())
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
#[test]
fn test_after_commit_with_auditor() {
    let service = AfterCommitService::new();
    let mut testkit = TestKitBuilder::auditor()
        .with_validators(2)
        .with_default_rust_service(service.clone())
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

#[test]
fn restart_testkit() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(3)
        .with_default_rust_service(AfterCommitService::new())
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
        .with_default_rust_service(AfterCommitService::new())
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
