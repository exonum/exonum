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
extern crate serde_derive;

// HACK: Silent "dead_code" warning.
pub use crate::hooks::{AfterCommitService, TxAfterCommit, SERVICE_ID, SERVICE_NAME};

use exonum::{explorer::BlockchainExplorer, helpers::Height, runtime::rust::Transaction};
use exonum_merkledb::{BinaryValue, ObjectHash};
use exonum_testkit::TestKitBuilder;

mod hooks;
mod proto;

#[test]
fn test_after_commit() {
    let service = AfterCommitService::new();
    let mut testkit = TestKitBuilder::validator()
        .with_default_rust_service(service.clone())
        .create();

    // Check that `after_commit` invoked on the correct height.
    for i in 1..5 {
        let block = testkit.create_block();
        if i > 1 {
            let arguments = &block[0].content().payload().arguments;
            let message = TxAfterCommit::from_bytes(arguments.into()).unwrap();
            assert_eq!(message, TxAfterCommit::new(Height(i - 1)));
        }

        assert_eq!(service.counter() as u64, i);

        let blockchain = testkit.blockchain();
        let keypair = blockchain.service_keypair();
        let tx = TxAfterCommit::new(Height(i)).sign(SERVICE_ID, keypair.0, &keypair.1);
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
        .create();

    for i in 1..5 {
        let block = testkit.create_block();
        assert!(block.is_empty());
        assert_eq!(service.counter() as u64, i);

        let blockchain = testkit.blockchain();
        let keypair = blockchain.service_keypair();
        let tx = TxAfterCommit::new(Height(i)).sign(SERVICE_ID, keypair.0, &keypair.1);
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
        .create();
    testkit.create_blocks_until(Height(5));

    let stopped = testkit.stop();
    assert_eq!(stopped.height(), Height(5));
    assert_eq!(stopped.network().validators().len(), 3);
    let service = AfterCommitService::new();
    let runtime = stopped.rust_runtime().with_factory(service.clone());
    let mut testkit = stopped.resume(vec![runtime]);
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
            let blockchain = testkit.blockchain();
            let keypair = blockchain.service_keypair();
            TxAfterCommit::new(Height(i))
                .sign(SERVICE_ID, keypair.0, &keypair.1)
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
        .create();

    let tx_hashes: Vec<_> = (100..105)
        .map(|i| {
            let blockchain = testkit.blockchain();
            let keypair = blockchain.service_keypair();
            let message = TxAfterCommit::new(Height(i)).sign(SERVICE_ID, keypair.0, &keypair.1);
            let tx_hash = message.object_hash();
            testkit.add_tx(message);
            assert!(testkit.is_tx_in_pool(&tx_hash));
            tx_hash
        })
        .collect();

    let stopped = testkit.stop();
    let runtime = stopped
        .rust_runtime()
        .with_factory(AfterCommitService::new());
    let testkit = stopped.resume(vec![runtime]);
    assert!(tx_hashes
        .iter()
        .all(|tx_hash| testkit.is_tx_in_pool(tx_hash)));
}
