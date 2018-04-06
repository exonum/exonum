// Copyright 2018 The Exonum Team
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

use exonum::crypto::{gen_keypair, hash, Hash};
use exonum::helpers::{init_logger, Height};
use exonum::crypto::CryptoHash;
use exonum::blockchain::Transaction;
use exonum_testkit::{TestKitBuilder, TestKitApi, TestKit, ApiKind};
use exonum_time::{MockTimeProvider, TimeService};

use serde_json;
use std::time::SystemTime;

use TimestampingService;
use transactions::TxTimestamp;
use schema::{Timestamp, TimestampEntry};

fn init_testkit() -> (TestKit, MockTimeProvider) {
    let _ = init_logger();
    let mock_provider = MockTimeProvider::new(SystemTime::now());
    let mut testkit = TestKitBuilder::validator()
        .with_service(TimestampingService::new())
        .with_service(TimeService::with_provider(mock_provider.clone()))
        .create();
    testkit.create_blocks_until(Height(2)); // TimeService is None if no blocks were forged
    (testkit, mock_provider)
}

/// Assert transaction status
fn assert_status(api: &TestKitApi, tx: &Transaction, expected_status: &serde_json::Value) {
    let info: serde_json::Value = api.get(
        ApiKind::Explorer,
        &format!("v1/transactions/{}", &tx.hash().to_string()),
    );
    if let serde_json::Value::Object(mut info) = info {
        let tx_status = info.remove("status").unwrap();
        assert_eq!(tx_status, *expected_status);
    } else {
        panic!("Invalid transaction info format, object expected");
    }
}

#[test]
fn test_api_get_timestamp_nothing() {
    let (testkit, _) = init_testkit();
    let api = testkit.api();
    let entry: Option<TimestampEntry> =
        api.get(
            ApiKind::Service("timestamping"),
            &format!("/v1/timestamps/value/{}", Hash::zero().to_hex()),
        );
    assert!(entry.is_none());
}

#[test]
fn test_api_post_timestamp() {
    let (testkit, _) = init_testkit();

    let info = Timestamp::new(&Hash::zero(), "metadata");
    let keypair = gen_keypair();
    let tx = TxTimestamp::new(&keypair.0, info, &keypair.1);

    let api = testkit.api();
    let tx_hash: Hash = api.post(ApiKind::Service("timestamping"), "/v1/timestamps", &tx);
    let tx2 = tx.clone();

    assert_eq!(tx2, tx);
    assert_eq!(tx2.hash(), tx_hash);
}

#[test]
fn test_api_get_timestamp_proof() {
    let (mut testkit, _) = init_testkit();

    let keypair = gen_keypair();

    // Create timestamp
    let info = Timestamp::new(&Hash::zero(), "metadata");
    let tx = TxTimestamp::new(&keypair.0, info, &keypair.1);
    testkit.create_block_with_transactions(txvec![tx]);

    // get proof
    let api = testkit.api();
    let _: serde_json::Value = api.get(
        ApiKind::Service("timestamping"),
        &format!("/v1/timestamps/proof/{}", Hash::zero().to_hex()),
    );

    // TODO implement proof validation
}

#[test]
fn test_api_get_timestamp_entry() {
    let (mut testkit, _) = init_testkit();

    let keypair = gen_keypair();

    // Create timestamp
    let info = Timestamp::new(&Hash::zero(), "metadata");
    let tx = TxTimestamp::new(&keypair.0, info.clone(), &keypair.1);
    testkit.create_block_with_transactions(txvec![tx.clone()]);

    let api = testkit.api();
    let entry: Option<TimestampEntry> =
        api.get(
            ApiKind::Service("timestamping"),
            &format!("/v1/timestamps/value/{}", Hash::zero().to_hex()),
        );

    let entry = entry.unwrap();
    assert_eq!(entry.timestamp(), info);
    assert_eq!(entry.tx_hash(), &tx.hash());
}

#[test]
fn test_api_can_not_add_same_content_hash() {
    let (mut testkit, _) = init_testkit();
    let api = testkit.api();

    let keypair = gen_keypair();
    let content_hash = hash(&[1]);
    let timestamp1 = Timestamp::new(&content_hash, "metadata");
    let timestamp2 = Timestamp::new(&content_hash, "other metadata");
    let tx_ok = TxTimestamp::new(&keypair.0, timestamp1.clone(), &keypair.1);
    let tx_err = TxTimestamp::new(&keypair.0, timestamp2.clone(), &keypair.1);

    testkit.create_block_with_transaction(tx_ok.clone());
    assert_status(&api, &tx_ok, &json!({ "type": "success" }));

    testkit.create_block_with_transaction(tx_err.clone());
    assert_status(
        &api,
        &tx_err,
        &json!({ "type": "error", "code": 0, "description": "Content hash already exists" }),
    );
}
