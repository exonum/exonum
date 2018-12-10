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

#[macro_use]
extern crate serde_json;

#[macro_use]
extern crate exonum_testkit;

extern crate exonum;
extern crate exonum_time;
extern crate exonum_timestamping;

use exonum::{
    api::node::public::explorer::{TransactionQuery, TransactionResponse},
    crypto::{gen_keypair, hash, Hash},
    helpers::Height,
    messages::{to_hex_string, RawTransaction, Signed},
};
use exonum_testkit::{ApiKind, TestKit, TestKitApi, TestKitBuilder};
use exonum_time::{time_provider::MockTimeProvider, TimeService};

use std::time::SystemTime;

use exonum_timestamping::{
    api::TimestampQuery,
    schema::{Timestamp, TimestampEntry},
    transactions::TxTimestamp,
    Service,
};

fn init_testkit() -> (TestKit, MockTimeProvider) {
    let mock_provider = MockTimeProvider::new(SystemTime::now().into());
    let mut testkit = TestKitBuilder::validator()
        .with_service(Service)
        .with_service(TimeService::with_provider(mock_provider.clone()))
        .create();
    testkit.create_blocks_until(Height(2)); // TimeService is None if no blocks were forged
    (testkit, mock_provider)
}

/// Assert transaction status
fn assert_status(
    api: &TestKitApi,
    tx: &Signed<RawTransaction>,
    expected_status: &serde_json::Value,
) {
    let info: serde_json::Value = api
        .public(ApiKind::Explorer)
        .query(&TransactionQuery::new(tx.hash()))
        .get("v1/transactions")
        .unwrap();

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
    let entry: Option<TimestampEntry> = api
        .public(ApiKind::Service("timestamping"))
        .query(&TimestampQuery::new(Hash::zero()))
        .get("v1/timestamps/value")
        .unwrap();

    assert!(entry.is_none());
}

#[test]
fn test_api_post_timestamp() {
    let (testkit, _) = init_testkit();

    let info = Timestamp::new(&Hash::zero(), "metadata");
    let keypair = gen_keypair();
    let tx = TxTimestamp::sign(&keypair.0, info, &keypair.1);

    let api = testkit.api();
    let data = to_hex_string(&tx);

    let tx_info: TransactionResponse = api
        .public(ApiKind::Explorer)
        .query(&json!({ "tx_body": data }))
        .post("v1/transactions")
        .unwrap();

    assert_eq!(tx.hash(), tx_info.tx_hash);
}

#[test]
fn test_api_get_timestamp_proof() {
    let (mut testkit, _) = init_testkit();

    let keypair = gen_keypair();

    // Create timestamp
    let info = Timestamp::new(&Hash::zero(), "metadata");
    let tx = TxTimestamp::sign(&keypair.0, info, &keypair.1);
    testkit.create_block_with_transactions(txvec![tx.clone()]);

    // get proof
    let api = testkit.api();
    let _: serde_json::Value = api
        .public(ApiKind::Service("timestamping"))
        .query(&TimestampQuery::new(Hash::zero()))
        .get("v1/timestamps/proof")
        .unwrap();

    // TODO: Implement proof validation. (ECR-1639)
}

#[test]
fn test_api_get_timestamp_entry() {
    let (mut testkit, _) = init_testkit();

    let keypair = gen_keypair();

    // Create timestamp
    let info = Timestamp::new(&Hash::zero(), "metadata");
    let tx = TxTimestamp::sign(&keypair.0, info.clone(), &keypair.1);
    testkit.create_block_with_transactions(txvec![tx.clone()]);

    let api = testkit.api();
    let entry: Option<TimestampEntry> = api
        .public(ApiKind::Service("timestamping"))
        .query(&TimestampQuery::new(Hash::zero()))
        .get("v1/timestamps/value")
        .unwrap();

    let entry = entry.unwrap();
    assert_eq!(entry.timestamp, info);
    assert_eq!(entry.tx_hash, tx.hash());
}

#[test]
fn test_api_can_not_add_same_content_hash() {
    let (mut testkit, _) = init_testkit();
    let api = testkit.api();

    let keypair = gen_keypair();
    let content_hash = hash(&[1]);
    let timestamp1 = Timestamp::new(&content_hash, "metadata");
    let timestamp2 = Timestamp::new(&content_hash, "other metadata");
    let tx_ok = TxTimestamp::sign(&keypair.0, timestamp1.clone(), &keypair.1);
    let tx_err = TxTimestamp::sign(&keypair.0, timestamp2.clone(), &keypair.1);

    testkit.create_block_with_transaction(tx_ok.clone());
    assert_status(&api, &tx_ok, &json!({ "type": "success" }));

    testkit.create_block_with_transaction(tx_err.clone());
    assert_status(
        &api,
        &tx_err,
        &json!({ "type": "error", "code": 0, "description": "Content hash already exists" }),
    );
}
