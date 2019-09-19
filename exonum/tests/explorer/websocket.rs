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

//! Tests for the websocket functionality.

use websocket::{
    client::sync::Client, stream::sync::TcpStream, ClientBuilder, Message as WsMessage,
    OwnedMessage, WebSocketResult,
};

use exonum::{api::websocket::*, crypto::gen_keypair, messages::Message, node::ExternalMessage};
use std::{
    thread::sleep,
    time::{Duration, Instant},
};

use crate::blockchain::{CreateWallet, Transfer, SERVICE_ID};
use crate::node::{run_node, run_node_with_message_len};

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

fn recv_text_msg(client: &mut Client<TcpStream>) -> Option<String> {
    if let Ok(response) = client.recv_message() {
        match response {
            OwnedMessage::Text(text) => return Some(text),
            other => panic!("Incorrect response: {:?}", other),
        }
    }
    None
}

#[test]
fn test_send_transaction() {
    let node_handler = run_node(6330, 8079);

    let mut client =
        create_ws_client("ws://localhost:8079/api/explorer/v1/ws").expect("Cannot connect to node");
    client
        .stream_ref()
        .set_read_timeout(Some(Duration::from_secs(30)))
        .unwrap();

    // Check that no messages on start.
    assert!(client.recv_message().is_err());

    // Send transaction.
    let (pk, sk) = gen_keypair();
    let tx = Message::sign_transaction(CreateWallet::new(&pk, "Alice"), SERVICE_ID, pk, &sk);
    let tx_hash = tx.hash();
    let tx_json =
        serde_json::to_string(&json!({ "type": "transaction", "payload": { "tx_body": tx }}))
            .unwrap();
    client.send_message(&OwnedMessage::Text(tx_json)).unwrap();

    // Check response on set message.
    let resp_text = recv_text_msg(&mut client).unwrap();
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
        .set_read_timeout(Some(Duration::from_secs(30)))
        .unwrap();

    // Get one message and check that it is text.
    let resp_text = recv_text_msg(&mut client).unwrap();

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
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();

    // Send transaction.
    let (pk, sk) = gen_keypair();
    let tx = Message::sign_transaction(CreateWallet::new(&pk, "Alice"), SERVICE_ID, pk, &sk);
    let tx_json = json!({ "tx_body": tx });
    let http_client = reqwest::Client::new();
    let _res = http_client
        .post("http://localhost:8081/api/explorer/v1/transactions")
        .json(&tx_json)
        .send()
        .unwrap();

    // Get one message and check that it is text.
    let resp_text = recv_text_msg(&mut client).unwrap();

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
fn test_transactions_subscribe_with_filter() {
    let node_handler = run_node(6333, 8082);

    // Create client with filter
    let mut client = create_ws_client(
        "ws://localhost:8082/api/explorer/v1/transactions/subscribe?service_id=0&message_id=0",
    )
    .expect("Cannot connect to node");
    client
        .stream_ref()
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let (pk, sk) = gen_keypair();
    let tx = Message::sign_transaction(CreateWallet::new(&pk, "Bob"), SERVICE_ID, pk, &sk);
    let tx_json = json!({ "tx_body": tx });
    let http_client = reqwest::Client::new();
    let _res = http_client
        .post("http://localhost:8082/api/explorer/v1/transactions")
        .json(&tx_json)
        .send()
        .unwrap();

    // Get one message and check that it is text.
    let resp_text = recv_text_msg(&mut client).unwrap();

    // Try to parse incoming message into Block.
    let notification = serde_json::from_str::<Notification>(&resp_text).unwrap();
    match notification {
        Notification::Transaction(_) => (),
        other => panic!(
            "Incorrect notification type (expected Transaction): {:?}",
            other
        ),
    };

    let (pk, sk) = gen_keypair();
    let (to, _) = gen_keypair();
    let tx = Message::sign_transaction(Transfer::new(&pk, &to, 10), SERVICE_ID, pk, &sk);
    let tx_json = json!({ "tx_body": tx });
    let _res = http_client
        .post("http://localhost:8082/api/explorer/v1/transactions")
        .json(&tx_json)
        .send()
        .unwrap();

    // Try to get a one message and check that it is none in this case.
    // Cause Transfer transaction has another message id.
    assert!(recv_text_msg(&mut client).is_none());

    // Shutdown node.
    client.shutdown().unwrap();
    node_handler
        .api_tx
        .send_external_message(ExternalMessage::Shutdown)
        .unwrap();
    node_handler.node_thread.join().unwrap();
}

#[test]
fn test_transactions_subscribe_with_partial_filter() {
    let node_handler = run_node(6334, 8083);

    // Create client with filter
    let mut client =
        create_ws_client("ws://localhost:8083/api/explorer/v1/transactions/subscribe?service_id=0")
            .expect("Cannot connect to node");
    client
        .stream_ref()
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let (pk, sk) = gen_keypair();
    let tx = Message::sign_transaction(CreateWallet::new(&pk, "Bob"), SERVICE_ID, pk, &sk);
    let tx_json = json!({ "tx_body": tx });
    let http_client = reqwest::Client::new();
    let _res = http_client
        .post("http://localhost:8083/api/explorer/v1/transactions")
        .json(&tx_json)
        .send()
        .unwrap();

    // Get one message and check that it is text.
    let resp_text = recv_text_msg(&mut client).unwrap();

    // Try to parse incoming message into Block.
    let notification = serde_json::from_str::<Notification>(&resp_text).unwrap();
    match notification {
        Notification::Transaction(_) => (),
        other => panic!(
            "Incorrect notification type (expected Transaction): {:?}",
            other
        ),
    };

    let (pk, sk) = gen_keypair();
    let (to, _) = gen_keypair();
    let tx = Message::sign_transaction(Transfer::new(&pk, &to, 10), SERVICE_ID, pk, &sk);
    let tx_json = json!({ "tx_body": tx });
    let _res = http_client
        .post("http://localhost:8083/api/explorer/v1/transactions")
        .json(&tx_json)
        .send()
        .unwrap();

    // Get one message and check that it is text.
    let resp_text = recv_text_msg(&mut client).unwrap();

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
fn test_transactions_subscribe_with_bad_filter() {
    let node_handler = run_node(6335, 8084);
    // A service id is missing in filter !!!
    let mut client =
        create_ws_client("ws://localhost:8084/api/explorer/v1/transactions/subscribe?message_id=0")
            .unwrap();

    client
        .stream_ref()
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let (pk, sk) = gen_keypair();
    let tx = Message::sign_transaction(CreateWallet::new(&pk, "Bob"), SERVICE_ID, pk, &sk);
    let tx_json = json!({ "tx_body": tx });
    let http_client = reqwest::Client::new();
    let _res = http_client
        .post("http://localhost:8084/api/explorer/v1/transactions")
        .json(&tx_json)
        .send()
        .unwrap();

    // Get one message and check that it is text.
    let resp_text = recv_text_msg(&mut client);
    assert!(resp_text.is_none());

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
    let node_handler = run_node(6336, 8085);

    let mut client =
        create_ws_client("ws://localhost:8085/api/explorer/v1/ws").expect("Cannot connect to node");
    client
        .stream_ref()
        .set_read_timeout(Some(Duration::from_secs(30)))
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
    let resp_text = recv_text_msg(&mut client).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&resp_text).unwrap(),
        json!({"result": "success"})
    );

    // Get one message and check that it is text.
    let resp_text = recv_text_msg(&mut client).unwrap();

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
fn test_node_shutdown_with_active_ws_client_should_not_wait_for_timeout() {
    let node_handler = run_node(6337, 8086);

    let mut clients = (0..8)
        .map(|_| {
            let client = create_ws_client("ws://localhost:8086/api/explorer/v1/ws")
                .expect("Cannot connect to node");
            client
                .stream_ref()
                .set_read_timeout(Some(Duration::from_secs(30)))
                .unwrap();
            client
        })
        .collect::<Vec<_>>();

    let now = Instant::now();

    // Shutdown node before clients.
    node_handler
        .api_tx
        .send_external_message(ExternalMessage::Shutdown)
        .unwrap();
    node_handler.node_thread.join().unwrap();

    assert!(now.elapsed().as_secs() < 15);

    // Each client should receive Close message.
    let msg = OwnedMessage::from(WsMessage::close_because(1000, "node shutdown"));
    for client in clients.iter_mut() {
        assert_eq!(client.recv_message().unwrap(), msg);
    }

    for client in clients {
        // Behavior of TcpStream::shutdown on disconnected stream is platform-specific.
        let _ = client.shutdown();
    }
}

#[test]
fn test_sending_message_size() {
    let max_message_len = 512_usize;
    let node_handler = run_node_with_message_len(6338, 8087, max_message_len as u32);
    let mut client =
        create_ws_client("ws://localhost:8087/api/explorer/v1/ws").expect("Cannot connect to node");
    client
        .stream_ref()
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();

    // Send transaction.
    let (pk, sk) = gen_keypair();
    let name = "a".repeat(371); // With name length 371 chars we'll get tx size: 512 bytes.
    let tx = Message::sign_transaction(CreateWallet::new(&pk, name.as_str()), SERVICE_ID, pk, &sk);
    assert_eq!(tx.signed_message().raw().len(), max_message_len);
    let tx_hash = tx.hash();
    let tx_json =
        serde_json::to_string(&json!({ "type": "transaction", "payload": { "tx_body": tx }}))
            .unwrap();
    client.send_message(&OwnedMessage::Text(tx_json)).unwrap();

    // Check response on set message when the message is smaller than 512 bytes.
    let resp_text = recv_text_msg(&mut client).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&resp_text).unwrap(),
        json!({
            "result": "success",
            "response": { "tx_hash": tx_hash }
        })
    );

    let (pk, sk) = gen_keypair();
    let name = "a".repeat(372);
    let tx = Message::sign_transaction(CreateWallet::new(&pk, name.as_str()), SERVICE_ID, pk, &sk);
    assert_eq!(tx.signed_message().raw().len(), max_message_len + 1);
    let tx_json =
        serde_json::to_string(&json!({ "type": "transaction", "payload": { "tx_body": tx }}))
            .unwrap();
    client.send_message(&OwnedMessage::Text(tx_json)).unwrap();

    // Check response on set message when the message is bigger than 512 bytes.
    let resp_text = recv_text_msg(&mut client).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&resp_text).unwrap(),
        json!({
            "result": "error",
            "description": "Payload too large: the allowed message limit is 512 bytes, while received 513 bytes"
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
