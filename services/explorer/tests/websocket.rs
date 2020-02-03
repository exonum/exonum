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

//! WebSocket API tests.

use actix_web::ws::CloseCode;
use assert_matches::assert_matches;
use exonum::{
    crypto::KeyPair,
    helpers::Height,
    merkledb::ObjectHash,
    runtime::{CoreError, ExecutionError, SUPERVISOR_INSTANCE_ID as SUPERVISOR_ID},
};
use exonum_explorer::api::websocket::Notification;
use exonum_rust_runtime::{DefaultInstance, ServiceFactory};
use exonum_supervisor::{ConfigPropose, Supervisor, SupervisorInterface};
use exonum_testkit::{TestKit, TestKitApi, TestKitBuilder};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use websocket::{
    client::sync::Client, stream::sync::TcpStream, ClientBuilder, Message as WsMessage,
    OwnedMessage,
};

use std::{thread, time::Duration};

use exonum_explorer_service::ExplorerFactory;

mod counter;
use crate::counter::{CounterInterface, CounterService, SERVICE_ID};

fn create_ws_client(addr: &str) -> Client<TcpStream> {
    let addr = addr.replace("http://", "ws://");
    let client = ClientBuilder::new(&addr)
        .unwrap()
        .connect_insecure()
        .expect("Cannot launch WS client");
    client
        .stream_ref()
        .set_read_timeout(Some(Duration::from_secs(1)))
        .expect("Cannot set read timeout for WS client");
    client
}

fn send_message(client: &mut Client<TcpStream>, message: &serde_json::Value) {
    let message_str = serde_json::to_string(message).unwrap();
    client
        .send_message(&OwnedMessage::Text(message_str))
        .expect("Cannot send message");
}

fn receive_message<T: DeserializeOwned>(client: &mut Client<TcpStream>) -> Option<T> {
    if let Ok(response) = client.recv_message() {
        match response {
            OwnedMessage::Text(ref text) => return Some(serde_json::from_str(text).unwrap()),
            other => panic!("Unexpected WS response: {:?}", other),
        }
    }
    None
}

fn assert_no_message(client: &mut Client<TcpStream>) {
    if let Some(value) = receive_message::<Value>(client) {
        panic!("Received unexpected message: {:?}", value);
    }
}

fn assert_closure(mut client: Client<TcpStream>) {
    let msg = OwnedMessage::from(WsMessage::close_because(
        CloseCode::Away.into(),
        "Explorer service shut down",
    ));
    assert_eq!(client.recv_message().unwrap(), msg);
    client.shutdown().ok();
}

fn init_testkit() -> (TestKit, TestKitApi) {
    let mut testkit = TestKitBuilder::validator()
        .with_default_rust_service(CounterService)
        .with_default_rust_service(ExplorerFactory)
        .build();
    let api = testkit.api();
    (testkit, api)
}

/// Checks that the WS client accepts valid transactions and discards transactions with
/// an incorrect instance ID.
#[test]
fn test_send_transaction() {
    let (mut testkit, api) = init_testkit();
    let url = api.public_url("api/explorer/v1/ws");
    let mut client = create_ws_client(&url);

    // Check that the server sends no messages initially.
    assert_no_message(&mut client);

    // Send transaction.
    let keypair = KeyPair::random();
    let tx = keypair.increment(SERVICE_ID, 3);
    let tx_hash = tx.object_hash();
    let tx_body = json!({ "type": "transaction", "payload": { "tx_body": tx }});
    send_message(&mut client, &tx_body);

    // Check server response.
    let response: Value = receive_message(&mut client).unwrap();
    assert_eq!(
        response,
        json!({
            "result": "success",
            "response": { "tx_hash": tx_hash },
        })
    );

    // Check that the transaction is in the mempool.
    testkit.poll_events();
    assert!(testkit.is_tx_in_pool(&tx_hash));

    // Send invalid transaction.
    let keypair = KeyPair::random();
    let tx = keypair.increment(SERVICE_ID + 1, 5);
    let tx_body = json!({ "type": "transaction", "payload": { "tx_body": tx }});
    send_message(&mut client, &tx_body);

    // Check response on sent message.
    let response: Value = receive_message(&mut client).unwrap();
    assert_eq!(
        response,
        json!({
            "result": "error",
            "description": ExecutionError::from(CoreError::IncorrectInstanceId).to_string(),
        })
    );
}

#[test]
fn test_blocks_subscription() {
    let (mut testkit, api) = init_testkit();
    let url = api.public_url("api/explorer/v1/blocks/subscribe");
    let mut client = create_ws_client(&url);

    testkit.create_block();
    // Get the block notification.
    let notification: Notification = receive_message(&mut client).unwrap();
    assert_matches!(notification, Notification::Block(ref block) if block.height == Height(1));

    // Create one more block.
    testkit.create_block();
    let notification: Notification = receive_message(&mut client).unwrap();
    assert_matches!(notification, Notification::Block(ref block) if block.height == Height(2));
}

#[test]
fn test_transactions_subscription() {
    let (mut testkit, api) = init_testkit();
    let url = api.public_url("api/explorer/v1/transactions/subscribe");
    let mut client = create_ws_client(&url);

    // Create a block with a single transaction.
    let keypair = KeyPair::random();
    let tx = keypair.increment(SERVICE_ID, 3);
    testkit.create_block_with_transaction(tx.clone());

    let notification: Notification = receive_message(&mut client).unwrap();
    let tx_summary = match notification {
        Notification::Transaction(summary) => summary,
        notification => panic!("Unexpected notification: {:?}", notification),
    };
    assert_eq!(tx_summary.tx_hash, tx.object_hash());
    assert_eq!(tx_summary.instance_id, SERVICE_ID);
    tx_summary.status.0.unwrap();
}

#[test]
fn test_transactions_subscription_with_filter() {
    let (mut testkit, api) = init_testkit();
    let url = format!(
        "api/explorer/v1/transactions/subscribe?instance_id={}&method_id=0",
        SERVICE_ID
    );
    let url = api.public_url(&url);
    let mut client = create_ws_client(&url);

    let alice = KeyPair::random();
    let reset_tx = alice.reset(SERVICE_ID, ());
    let inc_tx = alice.increment(SERVICE_ID, 3);
    testkit.create_block_with_transactions(vec![reset_tx, inc_tx.clone()]);

    let notification: Notification = receive_message(&mut client).unwrap();
    let tx_summary = match notification {
        Notification::Transaction(summary) => summary,
        notification => panic!("Unexpected notification: {:?}", notification),
    };
    assert_eq!(tx_summary.tx_hash, inc_tx.object_hash());
    assert_no_message(&mut client);

    // Create some more transfer transactions and check that they are received.
    let other_tx = alice.increment(SERVICE_ID, 1);
    testkit.create_block_with_transaction(other_tx.clone());

    let notification: Notification = receive_message(&mut client).unwrap();
    let tx_summary = match notification {
        Notification::Transaction(summary) => summary,
        notification => panic!("Unexpected notification: {:?}", notification),
    };
    assert_eq!(tx_summary.tx_hash, other_tx.object_hash());
    assert_no_message(&mut client);
}

#[test]
fn test_transactions_subscribe_with_partial_filter() {
    let (mut testkit, api) = init_testkit();
    let url = format!(
        "api/explorer/v1/transactions/subscribe?instance_id={}",
        SERVICE_ID
    );
    let url = api.public_url(&url);
    let mut client = create_ws_client(&url);

    let alice = KeyPair::random();
    let reset_tx = alice.reset(SERVICE_ID, ());
    let inc_tx = alice.increment(SERVICE_ID, 3);
    testkit.create_block_with_transactions(vec![reset_tx.clone(), inc_tx.clone()]);
    let other_tx = alice.increment(SERVICE_ID, 5);
    testkit.create_block_with_transaction(other_tx.clone());

    let summaries = (0..3).map(|_| {
        let notification: Notification = receive_message(&mut client).unwrap();
        match notification {
            Notification::Transaction(summary) => summary,
            notification => panic!("Unexpected notification: {:?}", notification),
        }
    });

    let summaries: Vec<_> = summaries
        .map(|summary| (summary.tx_hash, summary.location.block_height()))
        .collect();
    assert_eq!(
        summaries,
        vec![
            (reset_tx.object_hash(), Height(1)),
            (inc_tx.object_hash(), Height(1)),
            (other_tx.object_hash(), Height(2)),
        ]
    );

    assert_no_message(&mut client);
}

#[test]
fn test_transactions_subscribe_with_bad_filter() {
    let (mut testkit, api) = init_testkit();
    // `instance_id` is missing from the filter.
    let url = api.public_url("api/explorer/v1/transactions/subscribe?method_id=0");
    let mut client = create_ws_client(&url);

    let alice = KeyPair::random();
    let reset_tx = alice.reset(SERVICE_ID, ());
    let inc_tx = alice.increment(SERVICE_ID, 3);
    testkit.create_block_with_transactions(vec![reset_tx.clone(), inc_tx.clone()]);

    assert_no_message(&mut client);
}

#[test]
fn test_dynamic_subscriptions() {
    let (mut testkit, api) = init_testkit();
    let url = api.public_url("api/explorer/v1/ws");
    let mut client = create_ws_client(&url);

    testkit.create_block();
    assert_no_message(&mut client);
    let alice = KeyPair::random();
    testkit.create_block_with_transaction(alice.increment(SERVICE_ID, 1));
    assert_no_message(&mut client);

    let filters = json!({ "type": "set_subscriptions", "payload": [{ "type": "blocks" }]});
    send_message(&mut client, &filters);
    // First response is subscription result.
    let response: Value = receive_message(&mut client).unwrap();
    assert_eq!(response, json!({ "result": "success", "response": null }));

    let tx = alice.increment(SERVICE_ID, 2);
    let block = testkit.create_block_with_transaction(tx);
    let notification: Notification = receive_message(&mut client).unwrap();
    assert_matches!(notification, Notification::Block(ref b) if b.height == block.height());
    // Since the client is not subscribed to transactions, it should receive no corresponding
    // notification.
    assert_no_message(&mut client);
}

#[test]
fn test_node_shutdown_with_active_ws_client_should_not_wait_for_timeout() {
    let (testkit, api) = init_testkit();
    let url = api.public_url("api/explorer/v1/ws");
    let clients: Vec<_> = (0..5).map(|_| create_ws_client(&url)).collect();

    // Simulate shutting down the node.
    drop(testkit);

    // Each client should receive a `Close` message.
    for client in clients {
        assert_closure(client);
    }
}

#[test]
fn test_blocks_and_tx_subscriptions() {
    let (mut testkit, api) = init_testkit();

    // Create block WS client first.
    let block_url = api.public_url("api/explorer/v1/blocks/subscribe");
    let mut block_client = create_ws_client(&block_url);

    testkit.create_block();
    let notification: Notification = receive_message(&mut block_client).unwrap();
    match notification {
        Notification::Block(block) => assert_eq!(block.height, Height(1)),
        other => panic!("Incorrect notification type: {:?}", other),
    }
    block_client.shutdown().ok();

    // Open transaction WS client and test it.
    let tx_url = api.public_url("api/explorer/v1/transactions/subscribe");
    let mut tx_client = create_ws_client(&tx_url);
    let alice = KeyPair::random();
    let tx = alice.increment(SERVICE_ID, 3);
    testkit.create_block_with_transaction(tx.clone());
    let notification: Notification = receive_message(&mut tx_client).unwrap();
    match notification {
        Notification::Transaction(summary) => assert_eq!(summary.tx_hash, tx.object_hash()),
        other => panic!("Incorrect notification type: {:?}", other),
    }
    tx_client.shutdown().ok();

    // Open block WS client again.
    let mut block_client = create_ws_client(&block_url);
    testkit.create_block();
    let notification: Notification = receive_message(&mut block_client).unwrap();
    match notification {
        Notification::Block(block) => assert_eq!(block.height, Height(3)),
        other => panic!("Incorrect notification type: {:?}", other),
    }
    block_client.shutdown().ok();
}

#[test]
fn connections_shut_down_on_service_stop() {
    let mut testkit = TestKitBuilder::validator()
        .with_default_rust_service(ExplorerFactory)
        .with_rust_service(Supervisor)
        .with_artifact(Supervisor.artifact_id())
        .with_instance(Supervisor::simple())
        .build();

    let api = testkit.api();
    let url = api.public_url("api/explorer/v1/blocks/subscribe");
    let mut client = create_ws_client(&url);

    let deadline = Height(5);
    let config = ConfigPropose::new(0, deadline).stop_service(ExplorerFactory::INSTANCE_ID);
    let config_tx = testkit
        .us()
        .service_keypair()
        .propose_config_change(SUPERVISOR_ID, config);
    let block = testkit.create_block_with_transaction(config_tx);
    block[0].status().unwrap();

    // Retrieve blocks from the client.
    for height in block.height().0..deadline.0 {
        let notification: Notification = receive_message(&mut client).unwrap();
        match notification {
            Notification::Block(block) => assert_eq!(block.height, Height(height)),
            other => panic!("Incorrect notification type: {:?}", other),
        }

        thread::sleep(Duration::from_millis(50));
        testkit.create_block();
    }

    // Service should shut down and send the corresponding message to the client.
    assert_closure(client);
}
