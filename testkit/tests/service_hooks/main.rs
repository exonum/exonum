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

use exonum::{helpers::Height, runtime::rust::Transaction};
use exonum_merkledb::{BinaryValue, ObjectHash};
use exonum_testkit::{ServiceInstances, TestKitBuilder};

mod hooks;
mod proto;

fn after_commit_service_instances(factory: AfterCommitService) -> ServiceInstances {
    ServiceInstances::new(factory).with_instance(SERVICE_NAME, SERVICE_ID, ())
}

#[test]
fn test_after_commit() {
    let service = AfterCommitService::new();
    let mut testkit = TestKitBuilder::validator()
        .with_service(after_commit_service_instances(service.clone()))
        .create();

    // Check that `after_commit` invoked on the correct height.
    for i in 1..5 {
        let block = testkit.create_block();
        if i > 1 {
            let message = &block[0].content().message().payload().payload;
            let message = TxAfterCommit::from_bytes(message.into()).unwrap();
            assert_eq!(message, TxAfterCommit::new(Height(i - 1)));
        }

        assert_eq!(service.counter() as u64, i);

        let keypair = &testkit.blockchain().service_keypair;
        let tx = TxAfterCommit::new(Height(i)).sign(SERVICE_ID, keypair.0, &keypair.1);
        assert!(testkit.is_tx_in_pool(&tx.object_hash()));
    }

    let expected_block_sizes = testkit
        .explorer()
        .blocks(Height(1)..)
        .all(|block| block.len() == if block.height() == Height(1) { 0 } else { 1 });
    assert!(expected_block_sizes);
}

#[test]
#[ignore = "TODO: Rewrite restart services business logic [ECR-3260]"]
fn restart_testkit() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(3)
        .with_service(after_commit_service_instances(AfterCommitService::new()))
        .create();
    testkit.create_blocks_until(Height(5));

    let stopped = testkit.stop();
    assert_eq!(stopped.height(), Height(5));
    assert_eq!(stopped.network().validators().len(), 3);
    let service = AfterCommitService::new();
    let mut testkit = stopped.resume(vec![
    // service.clone().into()
    ]);
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
            let keypair = &testkit.blockchain().service_keypair;
            TxAfterCommit::new(Height(i))
                .sign(SERVICE_ID, keypair.0, &keypair.1)
                .object_hash()
        })
        .all(|hash| {
            testkit
                .explorer()
                .transaction_without_proof(&hash)
                .is_some()
        });
    assert!(transactions_are_committed);
}

#[test]
#[ignore = "TODO: Rewrite restart services business logic [ECR-3260]"]
fn tx_pool_is_retained_on_restart() {
    let mut testkit = TestKitBuilder::validator()
        .with_service(after_commit_service_instances(AfterCommitService::new()))
        .create();

    let tx_hashes: Vec<_> = (100..105)
        .map(|i| {
            let keypair = &testkit.blockchain().service_keypair;
            let message = TxAfterCommit::new(Height(i))
                .sign(SERVICE_ID, keypair.0, &keypair.1);
            let tx_hash = message.object_hash();
            testkit.add_tx(message);
            assert!(testkit.is_tx_in_pool(&tx_hash));
            tx_hash
        })
        .collect();

    let stopped = testkit.stop();
    let testkit = stopped.resume(vec![
        // AfterCommitService::new().into()
    ]);
    assert!(tx_hashes
        .iter()
        .all(|tx_hash| testkit.is_tx_in_pool(tx_hash)));
}
