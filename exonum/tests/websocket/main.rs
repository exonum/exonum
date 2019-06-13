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

//! Tests for the blockchain explorer functionality.

#[macro_use]
extern crate exonum_derive;
#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
#[cfg(test)]
#[macro_use]
extern crate pretty_assertions;

use exonum::{
    api::websocket::*, crypto::gen_keypair, node::ExternalMessage, runtime::rust::Transaction,
};
use exonum_merkledb::ObjectHash;
use websocket::{
    client::sync::Client, stream::sync::TcpStream, ClientBuilder, OwnedMessage, WebSocketResult,
};

use std::{thread::sleep, time::Duration};

mod blockchain;

use blockchain::*;

fn create_ws_client(addr: &str) -> WebSocketResult<Client<TcpStream>> {
    let mut last_err = None;
    for _ in 0..5 {
        match ClientBuilder::new(addr).unwrap().connect_insecure() {
            Err(e) => {
                sleep(Duration::from_millis(100));
                last_err = Some(e);
                continue;
            }
            ok => return ok,
        }
    }
    Err(last_err.unwrap())?
}

fn recv_text_msg(client: &mut Client<TcpStream>) -> String {
    let response = client.recv_message().unwrap();
    match response {
        OwnedMessage::Text(text) => text,
        other => panic!("Incorrect response: {:?}", other),
    }
}

#[test]
fn test_send_transaction() {
    let node_handler = run_node(6330, 8079);

    let mut client =
        create_ws_client("ws://localhost:8079/api/explorer/v1/ws").expect("Cannot connect to node");
    client
        .stream_ref()
        .set_read_timeout(Some(Duration::from_secs(60)))
        .unwrap();

    // Check that no messages on start.
    assert!(client.recv_message().is_err());

    // Send transaction.
    let (pk, sk) = gen_keypair();
    let tx = CreateWallet::new(&pk, "Alice").sign(SERVICE_ID, pk, &sk);
    let tx_hash = tx.object_hash();
    let tx_json =
        serde_json::to_string(&json!({ "type": "transaction", "payload": { "tx_body": tx }}))
            .unwrap();
    client.send_message(&OwnedMessage::Text(tx_json)).unwrap();

    // Check response on set message.
    let resp_text = recv_text_msg(&mut client);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&resp_text).unwrap(),
        json!({
            "result": "success",
            "response": { "tx_hash": tx_hash }
        })
    );

    // Shutdown node.
    client.shutdown().unwrap();
    node_handler
        .api_tx
        .send_external_message(ExternalMessage::Shutdown)
        .unwrap();
    node_handler.node_thread.join().unwrap();
}

#[test]
fn test_blocks_subscribe() {
    let node_handler = run_node(6331, 8080);

    let mut client = create_ws_client("ws://localhost:8080/api/explorer/v1/blocks/subscribe")
        .expect("Cannot connect to node");
    client
        .stream_ref()
        .set_read_timeout(Some(Duration::from_secs(60)))
        .unwrap();

    // Get one message and check that it is text.
    let resp_text = recv_text_msg(&mut client);

    // Try to parse incoming message into Block.
    let notification = serde_json::from_str::<Notification>(&resp_text).unwrap();
    match notification {
        Notification::Block(_) => (),
        other => panic!("Incorrect notification type (expected Block): {:?}", other),
    }

    // Shutdown node.
    client.shutdown().unwrap();
    node_handler
        .api_tx
        .send_external_message(ExternalMessage::Shutdown)
        .unwrap();
    node_handler.node_thread.join().unwrap();
}

#[test]
fn test_transactions_subscribe() {
    let node_handler = run_node(6332, 8081);

    let mut client = create_ws_client("ws://localhost:8081/api/explorer/v1/transactions/subscribe")
        .expect("Cannot connect to node");
    client
        .stream_ref()
        .set_read_timeout(Some(Duration::from_secs(60)))
        .unwrap();

    // Send transaction.
    let (pk, sk) = gen_keypair();
    let tx = CreateWallet::new(&pk, "Alice").sign(SERVICE_ID, pk, &sk);
    let tx_json = json!({ "tx_body": tx });
    let http_client = reqwest::Client::new();
    let _res = http_client
        .post("http://localhost:8081/api/explorer/v1/transactions")
        .json(&tx_json)
        .send()
        .unwrap();

    // Get one message and check that it is text.
    let resp_text = recv_text_msg(&mut client);

    // Try to parse incoming message into Block.
    let notification = serde_json::from_str::<Notification>(&resp_text).unwrap();
    match notification {
        Notification::Transaction(_) => (),
        other => panic!(
            "Incorrect notification type (expected Transaction): {:?}",
            other
        ),
    };

    // Shutdown node.
    client.shutdown().unwrap();
    node_handler
        .api_tx
        .send_external_message(ExternalMessage::Shutdown)
        .unwrap();
    node_handler.node_thread.join().unwrap();
}

#[test]
fn test_subscribe() {
    let node_handler = run_node(6333, 8082);

    let mut client =
        create_ws_client("ws://localhost:8082/api/explorer/v1/ws").expect("Cannot connect to node");
    client
        .stream_ref()
        .set_read_timeout(Some(Duration::from_secs(60)))
        .unwrap();

    // Check that no messages on start.
    assert!(client.recv_message().is_err());

    // Set blocks filter.
    let filters = serde_json::to_string(
        &json!({"type": "set-subscriptions", "payload": [{ "type": "blocks" }]}),
    )
    .unwrap();
    client.send_message(&OwnedMessage::Text(filters)).unwrap();

    // Check response on set message.
    let resp_text = recv_text_msg(&mut client);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&resp_text).unwrap(),
        json!({"result": "success"})
    );

    // Get one message and check that it is text.
    let resp_text = recv_text_msg(&mut client);

    // Try to parse incoming message into Block.
    let notification = serde_json::from_str::<Notification>(&resp_text).unwrap();
    match notification {
        Notification::Block(_) => (),
        other => panic!("Incorrect notification type (expected Block): {:?}", other),
    }

    // Shutdown node.
    client.shutdown().unwrap();
    node_handler
        .api_tx
        .send_external_message(ExternalMessage::Shutdown)
        .unwrap();
    node_handler.node_thread.join().unwrap();
}
