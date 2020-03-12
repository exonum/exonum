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
    crypto::{hash, Hash, KeyPair},
    helpers::Height,
    merkledb::ObjectHash,
    messages::Verified,
    runtime::{AnyTx, InstanceId},
};
use exonum_explorer_service::ExplorerFactory;
use exonum_testkit::{
    explorer::api::{TransactionQuery, TransactionResponse},
    ApiKind, Spec, TestKit, TestKitApi, TestKitBuilder,
};
use exonum_time::{MockTimeProvider, TimeServiceFactory};
use serde_json::json;

use std::time::SystemTime;

use exonum_timestamping::{
    Config, Timestamp, TimestampEntry, TimestampQuery, TimestampingInterface, TimestampingService,
};

const TIME_SERVICE_ID: InstanceId = 102;
const TIME_SERVICE_NAME: &str = "time";
const SERVICE_ID: InstanceId = 103;
const SERVICE_NAME: &str = "timestamping";

fn init_testkit() -> (TestKit, MockTimeProvider) {
    let mock_provider = MockTimeProvider::new(SystemTime::now().into());
    let time_service = TimeServiceFactory::with_provider(mock_provider.clone());
    let time_service =
        Spec::new(time_service).with_instance(TIME_SERVICE_ID, TIME_SERVICE_NAME, ());

    let config = Config {
        time_service_name: TIME_SERVICE_NAME.to_owned(),
    };
    let timestamping =
        Spec::new(TimestampingService).with_instance(SERVICE_ID, SERVICE_NAME, config);

    let mut testkit = TestKitBuilder::validator()
        .with(Spec::new(ExplorerFactory).with_default_instance())
        .with(time_service)
        .with(timestamping)
        .build();
    testkit.create_blocks_until(Height(2)); // Ensure that time is set
    (testkit, mock_provider)
}

/// Assert transaction status
async fn assert_status(
    api: &TestKitApi,
    tx: &Verified<AnyTx>,
    expected_status: &serde_json::Value,
) {
    let content: serde_json::Value = api
        .public(ApiKind::Explorer)
        .query(&TransactionQuery::new(tx.object_hash()))
        .get("v1/transactions")
        .await
        .unwrap();

    if let serde_json::Value::Object(mut info) = content {
        let tx_status = info.remove("status").unwrap();
        assert_eq!(tx_status, *expected_status);
    } else {
        panic!("Invalid transaction info format, object expected");
    }
}

#[tokio::test]
async fn test_api_get_timestamp_nothing() {
    let (mut testkit, _) = init_testkit();
    let api = testkit.api();
    let entry: Option<TimestampEntry> = api
        .public(ApiKind::Service(SERVICE_NAME))
        .query(&TimestampQuery::new(Hash::zero()))
        .get("v1/timestamps/value")
        .await
        .unwrap();

    assert!(entry.is_none());
}

#[tokio::test]
async fn test_api_post_timestamp() {
    let (mut testkit, _) = init_testkit();
    let content = Timestamp::new(&Hash::zero(), "metadata");
    let tx = KeyPair::random().timestamp(SERVICE_ID, content);

    let api = testkit.api();
    let tx_info: TransactionResponse = api
        .public(ApiKind::Explorer)
        .query(&json!({ "tx_body": tx }))
        .post("v1/transactions")
        .await
        .unwrap();

    assert_eq!(tx.object_hash(), tx_info.tx_hash);
}

#[tokio::test]
async fn test_api_get_timestamp_proof() {
    let (mut testkit, _) = init_testkit();
    let keypair = KeyPair::random();

    // Create timestamp
    let content = Timestamp::new(&Hash::zero(), "metadata");
    let tx = keypair.timestamp(SERVICE_ID, content);
    testkit.create_block_with_transaction(tx);

    // Get proof.
    let api = testkit.api();
    let _: serde_json::Value = api
        .public(ApiKind::Service(SERVICE_NAME))
        .query(&TimestampQuery::new(Hash::zero()))
        .get("v1/timestamps/proof")
        .await
        .unwrap();

    // TODO: Implement proof validation. (ECR-1639)
}

#[tokio::test]
async fn test_api_get_timestamp_entry() {
    let (mut testkit, _) = init_testkit();

    // Create timestamp
    let content = Timestamp::new(&Hash::zero(), "metadata");
    let tx = KeyPair::random().timestamp(SERVICE_ID, content.clone());
    testkit.create_block_with_transaction(tx.clone());

    let api = testkit.api();
    let entry: Option<TimestampEntry> = api
        .public(ApiKind::Service(SERVICE_NAME))
        .query(&TimestampQuery::new(Hash::zero()))
        .get("v1/timestamps/value")
        .await
        .unwrap();

    let entry = entry.unwrap();
    assert_eq!(entry.timestamp, content);
    assert_eq!(entry.tx_hash, tx.object_hash());
}

#[tokio::test]
async fn test_api_cannot_add_same_content_hash() {
    let (mut testkit, _) = init_testkit();
    let api = testkit.api();
    let keypair = KeyPair::random();
    let content_hash = hash(&[1]);
    let timestamp1 = Timestamp::new(&content_hash, "metadata");
    let timestamp2 = Timestamp::new(&content_hash, "other metadata");
    let tx_ok = keypair.timestamp(SERVICE_ID, timestamp1);
    let tx_err = keypair.timestamp(SERVICE_ID, timestamp2);

    testkit.create_block_with_transaction(tx_ok.clone());
    assert_status(&api, &tx_ok, &json!({ "type": "success" })).await;

    testkit.create_block_with_transaction(tx_err.clone());
    assert_status(
        &api,
        &tx_err,
        &json!({
            "type": "service_error",
            "call_site": {
                "call_type": "method",
                "instance_id": SERVICE_ID,
                "method_id": 0,
            },
            "code": 0,
            "description": "Content hash already exists.",
            "runtime_id": 0,
            "type": "service_error",
        }),
    )
    .await;
}

#[tokio::test]
async fn test_api_get_configuration() {
    let (mut testkit, _) = init_testkit();
    let api = testkit.api();

    let config: Config = api
        .public(ApiKind::Service(SERVICE_NAME))
        .get("v1/timestamps/config")
        .await
        .expect("Failed to get service configuration.");

    assert_eq!(config.time_service_name, "time");
}
