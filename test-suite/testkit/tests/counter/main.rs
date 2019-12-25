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

use assert_matches::assert_matches;
use exonum::{
    api::{
        node::public::explorer::{
            CallStatusQuery, CallStatusResponse, TransactionQuery, TransactionResponse,
        },
        Error as ApiError,
    },
    blockchain::{CallInBlock, ValidatorKeys},
    crypto::{self, gen_keypair, Hash},
    explorer::BlockchainExplorer,
    helpers::Height,
    merkledb::BinaryValue,
    messages::{AnyTx, Verified},
    runtime::{ErrorKind, ExecutionError, SnapshotExt},
};
use exonum_merkledb::{access::Access, HashTag, ObjectHash, Snapshot};
use exonum_testkit::{ApiKind, TestKit, TestKitApi, TestKitBuilder, TestNode};
use pretty_assertions::assert_eq;
use serde_json::{json, Value};

use crate::counter::{
    CounterSchema, CounterService, CounterServiceInterface, CounterWithProof, SERVICE_ID,
    SERVICE_NAME,
};

use exonum::blockchain::{AdditionalHeaders, ProposerId};
use exonum::helpers::ValidatorId;

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
    gen_keypair().increment(SERVICE_ID, by)
}

fn gen_inc_incorrect_tx(by: u64) -> Verified<AnyTx> {
    gen_keypair().increment(SERVICE_ID + 1, by)
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
    let keypair = gen_keypair();

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
    let keypair = gen_keypair();
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
fn test_explorer_blocks_basic() {
    use exonum::api::node::public::explorer::BlocksRange;
    use exonum::helpers::Height;

    let (mut testkit, api) = init_testkit();

    let BlocksRange { blocks, range } = api
        .public(ApiKind::Explorer)
        .get("v1/blocks?count=10")
        .unwrap();
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].block.height, Height(0));
    assert_eq!(blocks[0].block.prev_hash, crypto::Hash::zero());
    assert_eq!(range.start, Height(0));
    assert_eq!(range.end, Height(1));

    // Check JSON presentation of the block
    let response: serde_json::Value = api
        .public(ApiKind::Explorer)
        .get("v1/blocks?count=10")
        .unwrap();
    assert_eq!(
        response,
        json!({
            "range": { "start": 0, "end": 1 },
            "blocks": [{
                "height": 0,
                "tx_count": 0,
                "prev_hash": crypto::Hash::zero(),
                "tx_hash": HashTag::empty_list_hash(),
                "state_hash": blocks[0].block.state_hash,
                "error_hash": blocks[0].block.error_hash,
                "additional_headers": blocks[0].block.additional_headers,
            }],
        })
    );

    // Check empty block creation
    testkit.create_block();

    let BlocksRange { blocks, range } = api
        .public(ApiKind::Explorer)
        .get("v1/blocks?count=10")
        .unwrap();

    let mut headers = AdditionalHeaders::new();
    headers.insert::<ProposerId>(ValidatorId(0).into());

    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].block.height, Height(1));
    assert_eq!(blocks[0].block.prev_hash, blocks[1].block.object_hash());
    assert_eq!(blocks[0].block.tx_count, 0);
    assert_eq!(blocks[0].block.additional_headers, headers);
    assert_eq!(blocks[1].block.height, Height(0));
    assert_eq!(blocks[1].block.prev_hash, crypto::Hash::default());
    assert_eq!(range.start, Height(0));
    assert_eq!(range.end, Height(2));

    // Check positioning of `precommits` and `block_time` within response.
    let response: serde_json::Value = api
        .public(ApiKind::Explorer)
        .get("v1/blocks?count=10&earliest=1&add_precommits=true")
        .unwrap();

    let snapshot = testkit.snapshot();
    let precommit = BlockchainExplorer::new(snapshot.as_ref())
        .block(Height(1))
        .unwrap()
        .precommits()[0]
        .clone();
    assert_eq!(
        response,
        json!({
            "range": { "start": 1, "end": 2 },
            "blocks": [{
                "height": 1,
                "tx_count": 0,
                "prev_hash": blocks[1].block.object_hash(),
                "tx_hash": HashTag::empty_list_hash(),
                "state_hash": blocks[0].block.state_hash,
                "error_hash": blocks[0].block.error_hash,
                "precommits": [precommit],
                "additional_headers": blocks[0].block.additional_headers,
            }],
        })
    );

    let response: serde_json::Value = api
        .public(ApiKind::Explorer)
        .get("v1/blocks?count=10&earliest=1&add_blocks_time=true")
        .unwrap();
    assert_eq!(
        response,
        json!({
            "range": { "start": 1, "end": 2 },
            "blocks": [{
                "height": 1,
                "tx_count": 0,
                "prev_hash": blocks[1].block.object_hash(),
                "tx_hash": HashTag::empty_list_hash(),
                "state_hash": blocks[0].block.state_hash,
                "error_hash": blocks[0].block.error_hash,
                "time": precommit.payload().time(),
                "additional_headers": blocks[0].block.additional_headers,
            }],
        })
    );
}

#[test]
fn test_explorer_api_block_request() {
    let (mut testkit, api) = init_testkit();
    testkit.create_block();

    let response: Value = api
        .public(ApiKind::Explorer)
        .get("v1/block?height=1")
        .unwrap();
    assert_eq!(response["height"], 1);

    let response = api
        .public(ApiKind::Explorer)
        .get::<Value>("v1/block?height=10")
        .unwrap_err();

    assert_matches!(
        response,
        ApiError::NotFound(ref body) if body == "Requested block height (10) exceeds the blockchain height (1)"
    );
}

fn create_sample_block(testkit: &mut TestKit) {
    let height = testkit.height().next().0;
    if height == 2 || height == 5 {
        let tx = gen_keypair().increment(SERVICE_ID, height);
        testkit.api().send(tx.clone());
    }
    testkit.create_block();
}

#[test]
fn test_explorer_blocks_skip_empty_small() {
    use exonum::api::node::public::explorer::BlocksRange;
    use exonum::helpers::Height;

    let (mut testkit, api) = init_testkit();
    create_sample_block(&mut testkit);

    let BlocksRange { blocks, range } = api
        .public(ApiKind::Explorer)
        .get("v1/blocks?count=10&skip_empty_blocks=true")
        .unwrap();
    assert!(blocks.is_empty());
    assert_eq!(range.start, Height(0));
    assert_eq!(range.end, Height(2));

    create_sample_block(&mut testkit);

    let BlocksRange { blocks, range } = api
        .public(ApiKind::Explorer)
        .get("v1/blocks?count=10")
        .unwrap();
    assert_eq!(blocks.len(), 3);
    assert_eq!(blocks[0].block.height, Height(2));
    assert_eq!(blocks[0].block.prev_hash, blocks[1].block.object_hash());
    assert_eq!(blocks[0].block.tx_count, 1);
    assert_eq!(range.start, Height(0));
    assert_eq!(range.end, Height(3));

    let BlocksRange { blocks, range } = api
        .public(ApiKind::Explorer)
        .get("v1/blocks?count=10&skip_empty_blocks=true")
        .unwrap();
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].block.height, Height(2));
    assert_eq!(range.start, Height(0));
    assert_eq!(range.end, Height(3));

    create_sample_block(&mut testkit);
    create_sample_block(&mut testkit);

    let BlocksRange { blocks, range } = api
        .public(ApiKind::Explorer)
        .get("v1/blocks?count=10&skip_empty_blocks=true")
        .unwrap();
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].block.height, Height(2));
    assert_eq!(range.start, Height(0));
    assert_eq!(range.end, Height(5));
}

#[test]
fn test_explorer_blocks_skip_empty() {
    use exonum::api::node::public::explorer::BlocksRange;
    use exonum::helpers::Height;

    let (mut testkit, api) = init_testkit();
    for _ in 0..5 {
        create_sample_block(&mut testkit);
    }

    let BlocksRange { blocks, range } = api
        .public(ApiKind::Explorer)
        .get("v1/blocks?count=1&skip_empty_blocks=true")
        .unwrap();
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].block.height, Height(5));
    assert_eq!(range.start, Height(5));
    assert_eq!(range.end, Height(6));

    let BlocksRange { blocks, range } = api
        .public(ApiKind::Explorer)
        .get("v1/blocks?count=3&skip_empty_blocks=true")
        .unwrap();
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].block.height, Height(5));
    assert_eq!(blocks[1].block.height, Height(2));
    assert_eq!(range.start, Height(0));
    assert_eq!(range.end, Height(6));
}

#[test]
fn test_explorer_blocks_bounds() {
    use exonum::api::node::public::explorer::BlocksRange;
    use exonum::helpers::Height;

    let (mut testkit, api) = init_testkit();
    for _ in 0..5 {
        create_sample_block(&mut testkit);
    }

    // Check `latest` param
    let BlocksRange { blocks, range } = api
        .public(ApiKind::Explorer)
        .get("v1/blocks?count=10&skip_empty_blocks=true&latest=4")
        .unwrap();
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].block.height, Height(2));
    assert_eq!(range.start, Height(0));
    assert_eq!(range.end, Height(5));

    // Check `earliest` param
    let BlocksRange { blocks, range } = api
        .public(ApiKind::Explorer)
        .get("v1/blocks?count=10&earliest=3")
        .unwrap();
    assert_eq!(blocks.len(), 3);
    assert_eq!(blocks[0].block.height, Height(5));
    assert_eq!(range.start, Height(3));
    assert_eq!(range.end, Height(6));

    // Check `earliest` & `latest`
    let BlocksRange { blocks, range } = api
        .public(ApiKind::Explorer)
        .get("v1/blocks?count=10&latest=4&earliest=3")
        .unwrap();
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].block.height, Height(4));
    assert_eq!(range.start, Height(3));
    assert_eq!(range.end, Height(5));

    // Check that `count` takes precedence over `earliest`.
    let BlocksRange { blocks, range } = api
        .public(ApiKind::Explorer)
        .get("v1/blocks?count=2&latest=4&earliest=1")
        .unwrap();
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].block.height, Height(4));
    assert_eq!(range.start, Height(3));
    assert_eq!(range.end, Height(5));

    // Check `latest` param isn't exceed the height.
    let BlocksRange { blocks, range } = api
        .public(ApiKind::Explorer)
        .get("v1/blocks?count=2&latest=5")
        .unwrap();
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].block.height, Height(5));
    assert_eq!(range.start, Height(4));
    assert_eq!(range.end, Height(6));

    // Check `latest` param is exceed the height.
    let result: Result<BlocksRange, ApiError> = api
        .public(ApiKind::Explorer)
        .get("v1/blocks?count=2&latest=6");
    assert!(result.is_err());
}

#[test]
fn test_explorer_blocks_loaded_info() {
    use exonum::api::node::public::explorer::BlocksRange;
    use exonum::helpers::Height;

    let (mut testkit, api) = init_testkit();
    testkit.create_blocks_until(Height(6));

    let BlocksRange { blocks, .. } = api
        .public(ApiKind::Explorer)
        .get("v1/blocks?count=4")
        .unwrap();
    assert!(blocks
        .iter()
        .all(|info| info.time.is_none() && info.precommits.is_none()));

    let BlocksRange { blocks, .. } = api
        .public(ApiKind::Explorer)
        .get("v1/blocks?count=4&add_blocks_time=true")
        .unwrap();
    assert!(blocks
        .iter()
        .all(|info| info.time.is_some() && info.precommits.is_none()));

    let BlocksRange { blocks, .. } = api
        .public(ApiKind::Explorer)
        .get("v1/blocks?count=4&add_precommits=true")
        .unwrap();
    assert!(blocks
        .iter()
        .all(|info| info.time.is_none() && info.precommits.is_some()));
}

#[test]
fn test_explorer_single_block() {
    use exonum::helpers::Height;
    use std::collections::HashSet;

    let mut testkit = TestKitBuilder::validator()
        .with_validators(4)
        .with_default_rust_service(CounterService)
        .create();

    assert_eq!(testkit.majority_count(), 3);

    {
        let snapshot = testkit.snapshot();
        let explorer = BlockchainExplorer::new(snapshot.as_ref());
        let block = explorer.block(Height(0)).unwrap();
        assert_eq!(block.height(), Height(0));
        assert_eq!(block.header().prev_hash, crypto::Hash::default());
        assert_eq!(&*block.transaction_hashes(), &[]);
    }

    let tx = gen_keypair().increment(SERVICE_ID, 5);
    testkit.api().send(tx.clone());
    testkit.create_block(); // height == 1

    {
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
}

#[test]
fn test_explorer_transaction_info() {
    use exonum::explorer::{BlockchainExplorer, TransactionInfo};
    use exonum::helpers::Height;

    let (mut testkit, api) = init_testkit();
    let tx = gen_keypair().increment(SERVICE_ID, 5);

    let info = api
        .public(ApiKind::Explorer)
        .get::<Value>(&format!(
            "v1/transactions?hash={}",
            &tx.object_hash().to_hex()
        ))
        .unwrap_err();
    let error_body = json!({ "type": "unknown" });
    assert_matches!(
        info,
        ApiError::NotFound(ref body) if serde_json::from_str::<Value>(body).unwrap() == error_body
    );

    api.send(tx.clone());
    testkit.poll_events();

    let info: Value = api
        .public(ApiKind::Explorer)
        .get(&format!(
            "v1/transactions?hash={}",
            &tx.object_hash().to_hex()
        ))
        .unwrap();
    assert_eq!(
        info,
        json!({
            "type": "in-pool",
            "content": tx,
        })
    );

    testkit.create_block();
    let info: TransactionInfo = api
        .public(ApiKind::Explorer)
        .get(&format!(
            "v1/transactions?hash={}",
            &tx.object_hash().to_hex()
        ))
        .unwrap();
    assert!(info.is_committed());
    let committed = info.as_committed().unwrap();
    assert_eq!(committed.location().block_height(), Height(1));
    assert!(committed.status().is_ok());

    let snapshot = testkit.snapshot();
    let explorer = BlockchainExplorer::new(snapshot.as_ref());
    let block = explorer.block(Height(1)).unwrap();
    assert!(committed
        .location_proof()
        .check_against_hash(block.header().tx_hash)
        .is_ok());

    let proof = block.error_proof(CallInBlock::transaction(0));
    let proof = proof.check_against_hash(block.header().error_hash).unwrap();
    let (&call_location, status) = proof.all_entries().next().unwrap();
    assert_eq!(call_location, CallInBlock::transaction(0));
    assert!(status.is_none());
}

#[test]
fn test_explorer_transaction_statuses() {
    use exonum::explorer::TransactionInfo;

    let (mut testkit, api) = init_testkit();
    let tx = gen_keypair().increment(SERVICE_ID, 5);
    let error_tx = gen_keypair().increment(SERVICE_ID, 0);
    let panicking_tx = gen_keypair().increment(SERVICE_ID, u64::max_value() - 3);

    let block = testkit.create_block_with_transactions(vec![
        tx.clone(),
        error_tx.clone(),
        panicking_tx.clone(),
    ]);

    fn check_statuses(statuses: &[Result<(), ExecutionError>]) {
        assert!(statuses[0].is_ok());
        assert_eq!(
            *statuses[1].as_ref().unwrap_err(),
            ExecutionError::service(0, "Adding zero does nothing!").to_match()
        );
        assert_matches!(
            statuses[2],
            Err(ref err) if err.kind() == ErrorKind::Unexpected
                && err.description() == "attempt to add with overflow"
        );
    }

    // Check statuses retrieved from a block.
    let statuses: Vec<_> = block
        .transactions
        .iter()
        .map(|tx| tx.status().map_err(Clone::clone))
        .collect();
    check_statuses(&statuses);

    // Check errors in the `BlockWithTransactions`.
    let errors = block.error_map();
    assert_eq!(errors.len(), 2);
    assert_eq!(
        errors[&CallInBlock::transaction(1)].description(),
        "Adding zero does nothing!"
    );
    assert_eq!(
        errors[&CallInBlock::transaction(2)].kind(),
        ErrorKind::Unexpected
    );

    // Check status proofs for transactions.
    let snapshot = testkit.snapshot();
    let explorer = BlockchainExplorer::new(&snapshot);
    let block_info = explorer.block(testkit.height()).unwrap();
    let proof = block_info.error_proof(CallInBlock::transaction(0));
    let proof = proof.check_against_hash(block.header.error_hash).unwrap();
    assert_eq!(proof.entries().count(), 0);
    let proof = block_info.error_proof(CallInBlock::transaction(1));
    let proof = proof.check_against_hash(block.header.error_hash).unwrap();
    assert_eq!(proof.entries().count(), 1);
    assert_eq!(
        proof.entries().next().unwrap().1.description(),
        "Adding zero does nothing!"
    );
    let proof = block_info.error_proof(CallInBlock::transaction(2));
    let proof = proof.check_against_hash(block.header.error_hash).unwrap();
    assert_eq!(proof.entries().count(), 1);
    assert_eq!(
        proof.entries().next().unwrap().1.kind(),
        ErrorKind::Unexpected
    );

    // Now, the same statuses retrieved via explorer web API.
    let statuses: Vec<_> = [
        tx.object_hash(),
        error_tx.object_hash(),
        panicking_tx.object_hash(),
    ]
    .iter()
    .map(|hash| {
        let info: TransactionInfo = api
            .public(ApiKind::Explorer)
            .query(&TransactionQuery::new(*hash))
            .get("v1/transactions")
            .unwrap();
        info.as_committed().unwrap().status().map_err(Clone::clone)
    })
    .collect();
    check_statuses(&statuses);
}

#[test]
fn test_explorer_with_after_transactions_error() {
    let (mut testkit, _) = init_testkit();
    let tx1 = gen_keypair().increment(SERVICE_ID, 21);
    let keypair = gen_keypair();
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
    let tx = gen_keypair().increment(SERVICE_ID, 13);

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

/// Checks that `ExplorerApi` accepts valid transactions and discards transactions with incorrect instance ID.
#[test]
fn test_explorer_add_invalid_transaction() {
    let (_testkit, api) = init_testkit();

    // Send valid transaction.
    let keypair = gen_keypair();
    let tx = keypair.reset(SERVICE_ID, ());
    let data = hex::encode(tx.to_bytes());
    let response = api
        .public(ApiKind::Explorer)
        .query(&json!({ "tx_body": data }))
        .post::<TransactionResponse>("v1/transactions")
        .expect("Failed to send valid transaction.");
    assert_eq!(response.tx_hash, tx.object_hash());

    // Send invalid transaction.
    let tx = keypair.reset(SERVICE_ID + 1, ());
    let data = hex::encode(tx.to_bytes());
    let response = api
        .public(ApiKind::Explorer)
        .query(&json!({ "tx_body": data }))
        .post::<TransactionResponse>("v1/transactions")
        .expect_err("Expected transaction send to finish with error.");
    let error_body = "Execution error with code `dispatcher:7` occurred: Suitable runtime \
                      for the given service instance ID is not found.";
    assert_matches!(
        response,
        ApiError::BadRequest(ref body) if body == error_body
    );
}

#[test]
fn test_explorer_api_with_before_transactions_error() {
    let (mut testkit, api) = init_testkit();
    let key_pair = gen_keypair();
    let tx = key_pair.increment(SERVICE_ID, 13);

    // This tx lead to error in before_transaction on the next transaction
    testkit.create_block_with_transaction(tx.clone());
    let response = api
        .public(ApiKind::Explorer)
        .query(&CallStatusQuery {
            height: Height(1),
            service_id: SERVICE_ID,
        })
        .get::<CallStatusResponse>("v1/call_status/before_transactions")
        .expect("Explorer Api unexpectedly failed");
    assert!(response.status.0.is_ok());

    let tx = key_pair.increment(SERVICE_ID, 1);
    // So perform one more tx to check the error
    testkit.create_block_with_transaction(tx.clone());

    let response = api
        .public(ApiKind::Explorer)
        .query(&CallStatusQuery {
            height: Height(2),
            service_id: SERVICE_ID,
        })
        .get::<CallStatusResponse>("v1/call_status/before_transactions")
        .expect("Explorer Api unexpectedly failed");
    let execution_error = response.status.0.unwrap_err();
    assert!(execution_error.description().contains("Number 13"));

    let response = api
        .public(ApiKind::Explorer)
        .query(&TransactionQuery {
            hash: tx.object_hash(),
        })
        .get::<CallStatusResponse>("v1/call_status/transaction")
        .expect("Explorer Api unexpectedly failed");
    assert!(response.status.0.is_ok());

    let response = api
        .public(ApiKind::Explorer)
        .query(&CallStatusQuery {
            height: Height(1),
            service_id: SERVICE_ID,
        })
        .get::<CallStatusResponse>("v1/call_status/after_transactions")
        .expect("Explorer Api unexpectedly failed");
    assert!(response.status.0.is_ok());
}

#[test]
fn test_explorer_api_with_transaction_error() {
    let (mut testkit, api) = init_testkit();
    let tx = gen_keypair().increment(SERVICE_ID, 0);

    testkit.create_block_with_transaction(tx.clone());

    let response = api
        .public(ApiKind::Explorer)
        .query(&TransactionQuery {
            hash: tx.object_hash(),
        })
        .get::<CallStatusResponse>("v1/call_status/transaction")
        .expect("Explorer Api unexpectedly failed");
    let execution_error = response.status.0.unwrap_err();
    assert!(execution_error
        .description()
        .contains("Adding zero does nothing!"));

    let response = api
        .public(ApiKind::Explorer)
        .query(&CallStatusQuery {
            height: Height(1),
            service_id: SERVICE_ID,
        })
        .get::<CallStatusResponse>("v1/call_status/before_transactions")
        .expect("Explorer Api unexpectedly failed");
    assert!(response.status.0.is_ok());

    let response = api
        .public(ApiKind::Explorer)
        .query(&CallStatusQuery {
            height: Height(1),
            service_id: SERVICE_ID,
        })
        .get::<CallStatusResponse>("v1/call_status/after_transactions")
        .expect("Explorer Api unexpectedly failed");
    assert!(response.status.0.is_ok());
}

#[test]
fn test_explorer_api_with_after_transactions_error() {
    let (mut testkit, api) = init_testkit();
    let tx = gen_keypair().increment(SERVICE_ID, 42);

    testkit.create_block_with_transaction(tx.clone());

    let response = api
        .public(ApiKind::Explorer)
        .query(&CallStatusQuery {
            height: Height(1),
            service_id: SERVICE_ID,
        })
        .get::<CallStatusResponse>("v1/call_status/after_transactions")
        .expect("Explorer Api unexpectedly failed");
    let execution_error = response.status.0.unwrap_err();
    assert!(execution_error
        .description()
        .contains("What's the question?"));

    let response = api
        .public(ApiKind::Explorer)
        .query(&CallStatusQuery {
            height: Height(1),
            service_id: SERVICE_ID,
        })
        .get::<CallStatusResponse>("v1/call_status/before_transactions")
        .expect("Explorer Api unexpectedly failed");
    assert!(response.status.0.is_ok());

    let response = api
        .public(ApiKind::Explorer)
        .query(&TransactionQuery {
            hash: tx.object_hash(),
        })
        .get::<CallStatusResponse>("v1/call_status/transaction")
        .expect("Explorer Api unexpectedly failed");
    assert!(response.status.0.is_ok());
}

#[test]
fn test_explorer_api_without_error() {
    let (mut testkit, api) = init_testkit();
    let tx = gen_keypair().increment(SERVICE_ID, 1);

    testkit.create_block_with_transaction(tx.clone());

    let response = api
        .public(ApiKind::Explorer)
        .query(&CallStatusQuery {
            height: Height(1),
            service_id: SERVICE_ID,
        })
        .get::<CallStatusResponse>("v1/call_status/before_transactions")
        .expect("Explorer Api unexpectedly failed");
    assert!(response.status.0.is_ok());

    let response = api
        .public(ApiKind::Explorer)
        .query(&TransactionQuery {
            hash: tx.object_hash(),
        })
        .get::<CallStatusResponse>("v1/call_status/transaction")
        .expect("Explorer Api unexpectedly failed");
    assert!(response.status.0.is_ok());

    let response = api
        .public(ApiKind::Explorer)
        .query(&CallStatusQuery {
            height: Height(1),
            service_id: SERVICE_ID,
        })
        .get::<CallStatusResponse>("v1/call_status/after_transactions")
        .expect("Explorer Api unexpectedly failed");
    assert!(response.status.0.is_ok());
}
