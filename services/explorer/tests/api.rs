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
    blockchain::{AdditionalHeaders, CallInBlock, ProposerId},
    crypto::{Hash, KeyPair},
    helpers::{Height, ValidatorId},
    merkledb::{BinaryValue, HashTag, ObjectHash},
    runtime::{ErrorKind, ExecutionError},
};
use exonum_api as api;
use exonum_explorer::{api::*, BlockchainExplorer, TransactionInfo};
use exonum_testkit::{ApiKind, TestKit, TestKitApi, TestKitBuilder};
use serde_json::{json, Value};

use crate::counter::{CounterInterface, CounterService, SERVICE_ID};
use exonum_explorer_service::ExplorerFactory;

mod counter;

fn init_testkit() -> (TestKit, TestKitApi) {
    let mut testkit = TestKitBuilder::validator()
        .with_default_rust_service(CounterService)
        .with_default_rust_service(ExplorerFactory)
        .build();
    let api = testkit.api();
    (testkit, api)
}

#[test]
fn test_explorer_blocks_basic() {
    let (mut testkit, api) = init_testkit();

    let BlocksRange { blocks, range } = api
        .public(ApiKind::Explorer)
        .get("v1/blocks?count=10")
        .unwrap();
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].block.height, Height(0));
    assert_eq!(blocks[0].block.prev_hash, Hash::zero());
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
                "prev_hash": Hash::zero(),
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
    headers.insert::<ProposerId>(ValidatorId(0));

    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].block.height, Height(1));
    assert_eq!(blocks[0].block.prev_hash, blocks[1].block.object_hash());
    assert_eq!(blocks[0].block.tx_count, 0);
    assert_eq!(blocks[0].block.additional_headers, headers);
    assert_eq!(blocks[1].block.height, Height(0));
    assert_eq!(blocks[1].block.prev_hash, Hash::default());
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

    assert_eq!(response.http_code, api::HttpStatusCode::NOT_FOUND);
    assert_eq!(response.body.title, "Failed to get block info");
    assert_eq!(
        response.body.detail,
        "Requested block height (10) exceeds the blockchain height (1)"
    );
    assert_eq!(response.body.source, "2:explorer");
}

fn create_sample_block(testkit: &mut TestKit) {
    let height = testkit.height().next().0;
    if height == 2 || height == 5 {
        let tx = KeyPair::random().increment(SERVICE_ID, height);
        testkit.api().send(tx.clone());
    }
    testkit.create_block();
}

#[test]
fn test_explorer_blocks_skip_empty_small() {
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
    let result: Result<BlocksRange, api::Error> = api
        .public(ApiKind::Explorer)
        .get("v1/blocks?count=2&latest=6");
    assert!(result.is_err());
}

#[test]
fn test_explorer_blocks_loaded_info() {
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
fn test_explorer_transaction_info() {
    let (mut testkit, api) = init_testkit();
    let tx = KeyPair::random().increment(SERVICE_ID, 5);

    let info = api
        .public(ApiKind::Explorer)
        .get::<Value>(&format!(
            "v1/transactions?hash={}",
            &tx.object_hash().to_hex()
        ))
        .unwrap_err();

    assert_eq!(info.http_code, api::HttpStatusCode::NOT_FOUND);
    assert_eq!(info.body.title, "Failed to get transaction info");
    assert_eq!(
        info.body.detail,
        serde_json::to_string(&json!({"type": "unknown"})).unwrap()
    );
    assert_eq!(info.body.source, "2:explorer");

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
            "type": "in_pool",
            "message": tx,
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
    let (mut testkit, api) = init_testkit();
    let tx = KeyPair::random().increment(SERVICE_ID, 5);
    let error_tx = KeyPair::random().increment(SERVICE_ID, 0);
    let panicking_tx = KeyPair::random().increment(SERVICE_ID, u64::max_value() - 3);

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

/// Checks that `ExplorerApi` accepts valid transactions and discards transactions with
/// the incorrect instance ID.
#[test]
fn test_explorer_add_invalid_transaction() {
    let (_testkit, api) = init_testkit();

    // Send valid transaction.
    let keypair = KeyPair::random();
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

    assert_eq!(response.http_code, api::HttpStatusCode::BAD_REQUEST);
    assert_eq!(
        response.body.title,
        "Failed to add transaction to memory pool"
    );
    assert!(response
        .body
        .detail
        .starts_with("Execution error with code"));
    assert_eq!(response.body.source, "2:explorer");
}

#[test]
fn test_explorer_api_with_before_transactions_error() {
    let (mut testkit, api) = init_testkit();
    let key_pair = KeyPair::random();
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
    let tx = KeyPair::random().increment(SERVICE_ID, 0);

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
    let tx = KeyPair::random().increment(SERVICE_ID, 42);

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
    let tx = KeyPair::random().increment(SERVICE_ID, 1);

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
