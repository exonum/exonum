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

use exonum::{
    blockchain::{CallInBlock, ValidatorKeys},
    crypto::{Hash, KeyPair},
    helpers::Height,
    messages::{AnyTx, Verified},
    runtime::SnapshotExt,
};
use exonum_explorer::{api::TransactionResponse, BlockchainExplorer};
use exonum_merkledb::{access::Access, HashTag, ObjectHash, Snapshot};
use exonum_testkit::{ApiKind, TestKit, TestKitApi, TestKitBuilder, TestNode};
use pretty_assertions::assert_eq;

use std::collections::HashSet;

use crate::counter::{
    CounterSchema, CounterService, CounterServiceInterface, CounterWithProof, SERVICE_ID,
    SERVICE_NAME,
};

mod counter;

fn init_testkit() -> (TestKit, TestKitApi) {
    let mut testkit = TestKit::for_rust_service(CounterService, SERVICE_NAME, SERVICE_ID, ());
    let api = testkit.api();
    (testkit, api)
}

fn get_validator_keys(testkit: &TestKit) -> Vec<ValidatorKeys> {
    testkit
        .network()
        .validators()
        .iter()
        .map(TestNode::public_keys)
        .collect()
}

fn inc_count(api: &TestKitApi, by: u64) -> Hash {
    let tx_info: TransactionResponse = api
        .public(ApiKind::Service("counter"))
        .query(&by)
        .post("count")
        .unwrap();
    tx_info.tx_hash
}

fn get_schema<'a>(snapshot: &'a dyn Snapshot) -> CounterSchema<impl Access + 'a> {
    snapshot.service_schema(SERVICE_NAME).unwrap()
}

fn gen_inc_tx(by: u64) -> Verified<AnyTx> {
    KeyPair::random().increment(SERVICE_ID, by)
}

fn gen_inc_incorrect_tx(by: u64) -> Verified<AnyTx> {
    KeyPair::random().increment(SERVICE_ID + 1, by)
}

#[test]
fn test_inc_add_tx() {
    let (mut testkit, _) = init_testkit();
    let tx = gen_inc_tx(5);
    testkit.add_tx(tx.clone());
    assert!(testkit.is_tx_in_pool(&tx.object_hash()));
}

#[test]
#[should_panic(expected = "Attempt to add invalid tx in the pool")]
fn test_inc_add_tx_incorrect_transaction() {
    let (mut testkit, _) = init_testkit();
    let incorrect_tx = gen_inc_incorrect_tx(5);
    testkit.add_tx(incorrect_tx);
}

#[test]
fn test_inc_count_create_block() {
    let (mut testkit, api) = init_testkit();
    let keypair = KeyPair::random();

    // Create a pre-signed transaction
    testkit.create_block_with_transaction(keypair.increment(SERVICE_ID, 5));

    // Check that the user indeed is persisted by the service
    let counter: u64 = api
        .public(ApiKind::Service("counter"))
        .get("count")
        .unwrap();
    assert_eq!(counter, 5);

    let counter_with_proof: CounterWithProof = api
        .public(ApiKind::Service("counter"))
        .get("count-with-proof")
        .unwrap();
    let validator_keys = get_validator_keys(&testkit);
    assert_eq!(counter_with_proof.verify(&validator_keys), Some(5));

    testkit.create_block_with_transactions(vec![
        keypair.increment(SERVICE_ID, 4),
        keypair.increment(SERVICE_ID, 1),
    ]);

    let counter: u64 = api
        .public(ApiKind::Service("counter"))
        .get("count")
        .unwrap();
    assert_eq!(counter, 10);
    let counter_with_proof: CounterWithProof = api
        .public(ApiKind::Service("counter"))
        .get("count-with-proof")
        .unwrap();
    assert_eq!(counter_with_proof.verify(&validator_keys), Some(10));
}

#[should_panic(expected = "Transaction is already committed")]
#[test]
fn test_inc_count_create_block_with_committed_transaction() {
    let (mut testkit, _) = init_testkit();
    let keypair = KeyPair::random();
    // Create a pre-signed transaction
    testkit.create_block_with_transaction(keypair.increment(SERVICE_ID, 5));
    // Create another block with the same transaction
    testkit.create_block_with_transaction(keypair.increment(SERVICE_ID, 5));
}

#[test]
#[should_panic(expected = "Attempt to add invalid tx in the pool")]
fn test_inc_count_create_block_with_transaction_incorrect_transaction() {
    let (mut testkit, _) = init_testkit();
    let incorrect_tx = gen_inc_incorrect_tx(5);
    testkit.create_block_with_transaction(incorrect_tx);
}

#[test]
fn test_inc_count_api() {
    let (mut testkit, api) = init_testkit();
    inc_count(&api, 5);
    testkit.create_block();

    // Check that the user indeed is persisted by the service
    let counter: u64 = api
        .public(ApiKind::Service("counter"))
        .get("count")
        .unwrap();
    assert_eq!(counter, 5);
}

#[test]
fn test_inc_count_with_multiple_transactions() {
    let (mut testkit, api) = init_testkit();
    let validator_keys = get_validator_keys(&testkit);

    for _ in 0..100 {
        inc_count(&api, 1);
        inc_count(&api, 2);
        inc_count(&api, 3);
        inc_count(&api, 4);

        testkit.create_block();
        let counter_with_proof: CounterWithProof = api
            .public(ApiKind::Service("counter"))
            .get("count-with-proof")
            .unwrap();
        counter_with_proof.verify(&validator_keys);
    }

    assert_eq!(testkit.height(), Height(100));
    let counter: u64 = api
        .public(ApiKind::Service("counter"))
        .get("count")
        .unwrap();
    assert_eq!(counter, 10);
}

#[test]
fn test_inc_count_with_manual_tx_control() {
    let (mut testkit, api) = init_testkit();
    let tx_a = inc_count(&api, 5);
    let tx_b = inc_count(&api, 3);

    // Empty block
    testkit.create_block_with_tx_hashes(&[]);
    let counter: u64 = api
        .public(ApiKind::Service("counter"))
        .get("count")
        .unwrap();
    assert_eq!(counter, 0);

    // The counter is touched by the `before_transactions` handler.
    let counter: CounterWithProof = api
        .public(ApiKind::Service("counter"))
        .get("count-with-proof")
        .unwrap();
    assert_eq!(counter.verify(&get_validator_keys(&testkit)), None);

    testkit.create_block_with_tx_hashes(&[tx_b.object_hash()]);
    let counter: CounterWithProof = api
        .public(ApiKind::Service("counter"))
        .get("count-with-proof")
        .unwrap();
    assert_eq!(counter.verify(&get_validator_keys(&testkit)), Some(3));

    testkit.create_block_with_tx_hashes(&[tx_a.object_hash()]);
    let counter: u64 = api
        .public(ApiKind::Service("counter"))
        .get("count")
        .unwrap();
    assert_eq!(counter, 8);
}

#[test]
fn test_private_api() {
    let (mut testkit, api) = init_testkit();
    inc_count(&api, 5);
    inc_count(&api, 3);

    testkit.create_block();
    let counter: u64 = api
        .private(ApiKind::Service("counter"))
        .get("count")
        .unwrap();
    assert_eq!(counter, 8);

    let tx = testkit.us().service_keypair().reset(SERVICE_ID, ());
    let tx_info: TransactionResponse = api
        .private(ApiKind::Service("counter"))
        .query(&())
        .post("reset")
        .unwrap();
    assert_eq!(tx_info.tx_hash, tx.object_hash());

    testkit.create_block();
    let counter: CounterWithProof = api
        .public(ApiKind::Service("counter"))
        .get("count-with-proof")
        .unwrap();
    assert_eq!(counter.verify(&get_validator_keys(&testkit)), Some(0));
}

#[test]
#[should_panic(expected = "Insufficient number of precommits")]
fn counter_proof_without_precommits() {
    let (mut testkit, api) = init_testkit();
    inc_count(&api, 5);
    testkit.create_block();

    let mut counter: CounterWithProof = api
        .public(ApiKind::Service("counter"))
        .get("count-with-proof")
        .unwrap();
    counter.remove_precommits();
    counter.verify(&get_validator_keys(&testkit));
}

#[test]
#[should_panic(expected = "Invalid counter value in proof")]
fn counter_proof_with_mauled_value() {
    let (mut testkit, api) = init_testkit();
    inc_count(&api, 5);
    testkit.create_block();

    let mut counter: CounterWithProof = api
        .public(ApiKind::Service("counter"))
        .get("count-with-proof")
        .unwrap();
    counter.maul_value();
    counter.verify(&get_validator_keys(&testkit));
}

#[test]
fn test_duplicate_tx() {
    let (mut testkit, api) = init_testkit();

    inc_count(&api, 5);
    testkit.create_block();
    inc_count(&api, 5);
    inc_count(&api, 5);
    testkit.create_block();
    let counter: u64 = api
        .public(ApiKind::Service("counter"))
        .get("count")
        .unwrap();
    assert_eq!(counter, 5);
}

#[test]
fn test_explorer_with_after_transactions_error() {
    let (mut testkit, _) = init_testkit();
    let tx1 = KeyPair::random().increment(SERVICE_ID, 21);
    let keypair = KeyPair::random();
    let tx2 = keypair.increment(SERVICE_ID, 21);

    let block = testkit.create_block_with_transactions(vec![tx1, tx2]);
    let errors = block.error_map();
    assert_eq!(errors.len(), 1);
    assert!(errors[&CallInBlock::after_transactions(SERVICE_ID)]
        .description()
        .contains("What's the question?"));
    assert_ne!(block.header.error_hash, HashTag::empty_map_hash());

    let tx3 = keypair.increment(SERVICE_ID, 1);
    let block = testkit.create_block_with_transaction(tx3);
    assert!(block.errors.is_empty());
    assert_eq!(block.header.error_hash, HashTag::empty_map_hash());
}

#[test]
fn test_explorer_with_before_transactions_error() {
    let (mut testkit, _) = init_testkit();
    let tx = KeyPair::random().increment(SERVICE_ID, 13);

    let block = testkit.create_block_with_transaction(tx);
    let errors = block.error_map();
    assert!(errors.is_empty(), "{:?}", errors);
    let block = testkit.create_block();
    let errors = block.error_map();
    assert_eq!(errors.len(), 1);
    assert!(errors[&CallInBlock::before_transactions(SERVICE_ID)]
        .description()
        .contains("Number 13"));

    let snapshot = testkit.snapshot();
    let schema = get_schema(&snapshot);
    assert_eq!(schema.counter.get(), Some(13));
    // ^-- The changes in `before_transactions` should be reverted.
}

#[test]
fn test_explorer_single_block() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(4)
        .with_default_rust_service(CounterService)
        .build();

    assert_eq!(testkit.majority_count(), 3);

    let snapshot = testkit.snapshot();
    let explorer = BlockchainExplorer::new(&snapshot);
    let block = explorer.block(Height(0)).unwrap();
    assert_eq!(block.height(), Height(0));
    assert_eq!(block.header().prev_hash, Hash::default());
    assert_eq!(&*block.transaction_hashes(), &[]);

    let tx = KeyPair::random().increment(SERVICE_ID, 5);
    testkit.api().send(tx.clone());
    testkit.create_block(); // height == 1

    let snapshot = testkit.snapshot();
    let explorer = BlockchainExplorer::new(snapshot.as_ref());
    let block = explorer.block(Height(1)).unwrap();
    assert_eq!(block.height(), Height(1));
    assert_eq!(block.len(), 1);
    assert_eq!(
        block.header().tx_hash,
        HashTag::hash_list(&[tx.object_hash()])
    );
    assert_eq!(&*block.transaction_hashes(), &[tx.object_hash()]);

    let mut validators = HashSet::new();
    for precommit in block.precommits().iter() {
        assert_eq!(precommit.payload().height, Height(1));
        assert_eq!(precommit.payload().block_hash, block.header().object_hash());
        let pk = testkit
            .network()
            .consensus_public_key_of(precommit.payload().validator)
            .expect("Cannot find validator id");
        validators.insert(precommit.payload().validator);
        assert_eq!(pk, precommit.author())
    }

    assert!(validators.len() >= testkit.majority_count());
}
