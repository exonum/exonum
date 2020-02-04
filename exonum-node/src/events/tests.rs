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
    blockchain::ConsensusConfig,
    crypto::{KeyPair, PublicKey, Seed, PUBLIC_KEY_LENGTH, SEED_LENGTH, SIGNATURE_LENGTH},
    helpers::user_agent,
    merkledb::BinaryValue,
    messages::{SignedMessage, Verified},
};
use futures::{sync::mpsc, Future, Sink, Stream};
use pretty_assertions::assert_eq;
use tokio::util::FutureExt;
use tokio_core::reactor::Core;

use std::{
    net::SocketAddr,
    thread,
    time::{self, Duration, SystemTime},
};

use crate::{
    connect_list::ConnectList,
    events::{
        error::log_error, network::NetworkPart, noise::HandshakeParams, NetworkEvent,
        NetworkRequest,
    },
    messages::Connect,
    state::SharedConnectList,
    ConnectInfo, EventsPoolCapacity, NetworkConfiguration, NodeChannel,
};

#[derive(Debug)]
struct TestHandler {
    handle: Option<thread::JoinHandle<()>>,
    listen_address: SocketAddr,
    network_events_rx: mpsc::Receiver<NetworkEvent>,
    network_requests_tx: mpsc::Sender<NetworkRequest>,
}

impl TestHandler {
    fn new(
        listen_address: SocketAddr,
        network_requests_tx: mpsc::Sender<NetworkRequest>,
        network_events_rx: mpsc::Receiver<NetworkEvent>,
    ) -> TestHandler {
        TestHandler {
            handle: None,
            listen_address,
            network_events_rx,
            network_requests_tx,
        }
    }

    fn wait_for_event(&mut self) -> Result<NetworkEvent, ()> {
        let rx = self.network_events_rx.by_ref();
        let future = rx
            .into_future()
            .timeout(Duration::from_secs(30))
            .map_err(drop);

        let mut core = Core::new().unwrap();
        let (event, _) = core.run(future)?;
        event.ok_or(())
    }

    pub fn disconnect_with(&self, key: PublicKey) {
        self.network_requests_tx
            .clone()
            .send(NetworkRequest::DisconnectWithPeer(key))
            .wait()
            .unwrap();
    }

    pub fn connect_with(&self, key: PublicKey, connect: Verified<Connect>) {
        self.network_requests_tx
            .clone()
            .send(NetworkRequest::SendMessage(key, connect.into()))
            .wait()
            .unwrap();
    }

    pub fn send_to(&self, key: PublicKey, raw: SignedMessage) {
        self.network_requests_tx
            .clone()
            .send(NetworkRequest::SendMessage(key, raw))
            .wait()
            .unwrap();
    }

    pub fn wait_for_connect(&mut self) -> Verified<Connect> {
        match self.wait_for_event() {
            Ok(NetworkEvent::PeerConnected(_addr, connect)) => connect,
            Ok(other) => panic!("Unexpected connect received, {:?}", other),
            Err(e) => panic!("An error during wait for connect occurred, {:?}", e),
        }
    }

    pub fn wait_for_disconnect(&mut self) -> PublicKey {
        match self.wait_for_event() {
            Ok(NetworkEvent::PeerDisconnected(addr)) => addr,
            Ok(other) => panic!("Unexpected disconnect received, {:?}", other),
            Err(e) => panic!("An error during wait for disconnect occurred, {:?}", e),
        }
    }

    pub fn wait_for_message(&mut self) -> SignedMessage {
        match self.wait_for_event() {
            Ok(NetworkEvent::MessageReceived(msg)) => {
                SignedMessage::from_bytes(msg.into()).expect("Unable to decode signed message")
            }
            Ok(other) => panic!("Unexpected message received, {:?}", other),
            Err(e) => panic!("An error during wait for message occurred, {:?}", e),
        }
    }

    pub fn shutdown(&mut self) {
        self.network_requests_tx
            .clone()
            .send(NetworkRequest::Shutdown)
            .wait()
            .unwrap();
        self.handle.take().expect("shutdown twice").join().unwrap();
    }
}

impl Drop for TestHandler {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            self.shutdown();
        }
    }
}

#[derive(Debug)]
struct TestEvents {
    listen_address: SocketAddr,
    network_config: NetworkConfiguration,
    events_config: EventsPoolCapacity,
    connect_list: SharedConnectList,
}

impl TestEvents {
    fn with_addr(listen_address: SocketAddr, connect_list: &SharedConnectList) -> TestEvents {
        TestEvents {
            listen_address,
            network_config: NetworkConfiguration::default(),
            events_config: EventsPoolCapacity::default(),
            connect_list: connect_list.clone(),
        }
    }

    fn spawn(self, handshake_params: &HandshakeParams, connect: Verified<Connect>) -> TestHandler {
        let (mut handler_part, network_part) = self.into_reactor(connect);
        let handshake_params = handshake_params.clone();
        let handle = thread::spawn(move || {
            let mut core = Core::new().unwrap();
            let fut = network_part.run(&core.handle(), &handshake_params);
            core.run(fut).map_err(log_error).unwrap();
        });
        handler_part.handle = Some(handle);
        handler_part
    }

    fn into_reactor(self, connect: Verified<Connect>) -> (TestHandler, NetworkPart) {
        let channel = NodeChannel::new(&self.events_config);
        let network_config = self.network_config;
        let (network_tx, network_rx) = channel.network_events;
        let network_requests_tx = channel.network_requests.0.clone();

        let network_part = NetworkPart {
            our_connect_message: connect,
            listen_address: self.listen_address,
            network_config,
            max_message_len: ConsensusConfig::DEFAULT_MAX_MESSAGE_LEN,
            network_requests: channel.network_requests,
            network_tx: network_tx.clone(),
            connect_list: self.connect_list,
        };

        let handler_part = TestHandler::new(self.listen_address, network_requests_tx, network_rx);
        (handler_part, network_part)
    }
}

pub fn connect_message(addr: SocketAddr, keypair: &KeyPair) -> Verified<Connect> {
    let time = time::UNIX_EPOCH;
    let inner = Connect::new(&addr.to_string(), time.into(), &user_agent());
    Verified::from_value(inner, keypair.public_key(), keypair.secret_key())
}

pub fn raw_message(payload_len: usize) -> SignedMessage {
    let buffer = vec![0u8; payload_len];
    let keys = KeyPair::random();
    SignedMessage::new(buffer, keys.public_key(), keys.secret_key())
}

#[derive(Debug, Clone)]
struct ConnectionParams {
    connect: Verified<Connect>,
    connect_info: ConnectInfo,
    address: SocketAddr,
    handshake_params: HandshakeParams,
}

impl HandshakeParams {
    // Helper method to create `HandshakeParams` with empty `ConnectList` and
    // default `max_message_len`.
    #[doc(hidden)]
    pub fn with_default_params() -> Self {
        let keypair = KeyPair::from_seed(&Seed::new([1; SEED_LENGTH]));
        let address = "127.0.0.1:8000";

        let connect = Verified::from_value(
            Connect::new(address, SystemTime::now().into(), &user_agent()),
            keypair.public_key(),
            keypair.secret_key(),
        );

        let mut params = HandshakeParams::new(
            &keypair,
            SharedConnectList::default(),
            connect,
            ConsensusConfig::DEFAULT_MAX_MESSAGE_LEN,
        );

        params.set_remote_key(keypair.public_key());
        params
    }
}

impl ConnectionParams {
    fn from_address(address: SocketAddr) -> Self {
        let keypair = KeyPair::random();
        let connect = connect_message(address, &keypair);
        let handshake_params = HandshakeParams::new(
            &keypair,
            SharedConnectList::default(),
            connect.clone(),
            ConsensusConfig::DEFAULT_MAX_MESSAGE_LEN,
        );
        let connect_info = ConnectInfo {
            address: address.to_string(),
            public_key: keypair.public_key(),
        };

        ConnectionParams {
            connect,
            address,
            handshake_params,
            connect_info,
        }
    }

    fn spawn(&mut self, events: TestEvents, connect_list: SharedConnectList) -> TestHandler {
        self.handshake_params.connect_list = connect_list.clone();
        events.spawn(&self.handshake_params, self.connect.clone())
    }
}

#[test]
fn test_network_handshake() {
    let first = "127.0.0.1:17230".parse().unwrap();
    let second = "127.0.0.1:17231".parse().unwrap();

    let mut connect_list = ConnectList::default();

    let mut t1 = ConnectionParams::from_address(first);
    let first_key = t1.connect_info.public_key;
    connect_list.add(t1.connect_info.clone());

    let mut t2 = ConnectionParams::from_address(second);
    let second_key = t2.connect_info.public_key;
    connect_list.add(t2.connect_info.clone());

    let connect_list = SharedConnectList::from_connect_list(connect_list);

    let e1 = TestEvents::with_addr(first, &connect_list);
    let e2 = TestEvents::with_addr(second, &connect_list);

    let mut e1 = t1.spawn(e1, connect_list.clone());
    let mut e2 = t2.spawn(e2, connect_list);

    e1.connect_with(second_key, t1.connect.clone());
    assert_eq!(e2.wait_for_connect(), t1.connect.clone());
    assert_eq!(e1.wait_for_connect(), t2.connect.clone());

    e1.disconnect_with(second_key);
    assert_eq!(e1.wait_for_disconnect(), second_key);

    e2.disconnect_with(first_key);
    assert_eq!(e2.wait_for_disconnect(), first_key);
}

#[test]
fn test_network_big_message() {
    let first = "127.0.0.1:17200".parse().unwrap();
    let second = "127.0.0.1:17201".parse().unwrap();

    let m1 = raw_message(100_000);
    let m2 = raw_message(400);

    let mut connect_list = ConnectList::default();

    let mut t1 = ConnectionParams::from_address(first);
    let first_key = t1.connect_info.public_key;
    connect_list.add(t1.connect_info.clone());

    let mut t2 = ConnectionParams::from_address(second);
    let second_key = t2.connect_info.public_key;
    connect_list.add(t2.connect_info.clone());

    let connect_list = SharedConnectList::from_connect_list(connect_list);

    let e1 = TestEvents::with_addr(first, &connect_list);
    let e2 = TestEvents::with_addr(second, &connect_list);

    let mut e1 = t1.spawn(e1, connect_list.clone());
    let mut e2 = t2.spawn(e2, connect_list);

    e1.connect_with(second_key, t1.connect.clone());

    e2.wait_for_connect();
    e1.wait_for_connect();

    e1.send_to(second_key, m1.clone());
    assert_eq!(e2.wait_for_message(), m1);

    e1.send_to(second_key, m2.clone());
    assert_eq!(e2.wait_for_message(), m2);

    e1.send_to(second_key, m1.clone());
    assert_eq!(e2.wait_for_message(), m1);

    e2.send_to(first_key, m2.clone());
    assert_eq!(e1.wait_for_message(), m2);

    e2.send_to(first_key, m1.clone());
    assert_eq!(e1.wait_for_message(), m1);

    e2.send_to(first_key, m2.clone());
    assert_eq!(e1.wait_for_message(), m2);

    e1.disconnect_with(second_key);
    assert_eq!(e1.wait_for_disconnect(), second_key);

    e2.disconnect_with(first_key);
    assert_eq!(e2.wait_for_disconnect(), first_key);
}
#[test]
fn test_network_max_message_len() {
    let first = "127.0.0.1:17202".parse().unwrap();
    let second = "127.0.0.1:17303".parse().unwrap();

    let max_message_length = ConsensusConfig::DEFAULT_MAX_MESSAGE_LEN as usize;
    // Minimal size of protobuf messages can't be determined exactly.
    let acceptable_message =
        raw_message(max_message_length - SIGNATURE_LENGTH - PUBLIC_KEY_LENGTH - 100);
    let too_big_message = raw_message(max_message_length + 1000);
    assert!(too_big_message.to_bytes().len() > max_message_length);
    assert!(acceptable_message.to_bytes().len() <= max_message_length);
    let mut connect_list = ConnectList::default();
    let mut t1 = ConnectionParams::from_address(first);
    connect_list.add(t1.connect_info.clone());
    let first_key = t1.connect_info.public_key;

    let mut t2 = ConnectionParams::from_address(second);
    connect_list.add(t2.connect_info.clone());
    let second_key = t2.connect_info.public_key;

    let connect_list = SharedConnectList::from_connect_list(connect_list);

    let e1 = TestEvents::with_addr(first, &connect_list);
    let e2 = TestEvents::with_addr(second, &connect_list);

    let mut e1 = t1.spawn(e1, connect_list.clone());
    let mut e2 = t2.spawn(e2, connect_list);

    e1.connect_with(second_key, t1.connect.clone());

    e2.wait_for_connect();
    e1.wait_for_connect();

    e1.send_to(second_key, acceptable_message.clone());
    assert_eq!(e2.wait_for_message(), acceptable_message);

    e2.send_to(first_key, too_big_message.clone());
    assert_eq!(e1.wait_for_disconnect(), second_key);
}

#[test]
fn test_network_reconnect() {
    let first = "127.0.0.1:19100".parse().unwrap();
    let second = "127.0.0.1:19101".parse().unwrap();

    let msg = raw_message(1000);

    let mut connect_list = ConnectList::default();
    let mut t1 = ConnectionParams::from_address(first);
    connect_list.add(t1.connect_info.clone());

    let mut t2 = ConnectionParams::from_address(second);
    let second_key = t2.connect_info.public_key;
    connect_list.add(t2.connect_info.clone());

    let connect_list = SharedConnectList::from_connect_list(connect_list);

    let e1 = TestEvents::with_addr(first, &connect_list);
    let e2 = TestEvents::with_addr(second, &connect_list);

    let mut e1 = t1.spawn(e1, connect_list.clone());

    // First connect attempt.
    let mut e2 = t2.spawn(e2, connect_list.clone());

    // Handle first attempt.
    e1.connect_with(second_key, t1.connect.clone());
    assert_eq!(e2.wait_for_connect(), t1.connect.clone());
    assert_eq!(e1.wait_for_connect(), t2.connect.clone());

    e1.send_to(second_key, msg.clone());
    assert_eq!(e2.wait_for_message(), msg);

    e1.disconnect_with(second_key);
    drop(e2);
    assert_eq!(e1.wait_for_disconnect(), second_key);

    // Handle second attempt.
    let e2 = TestEvents::with_addr(second, &connect_list);
    let mut e2 = t2.spawn(e2, connect_list);

    e1.connect_with(second_key, t1.connect.clone());
    assert_eq!(e2.wait_for_connect(), t1.connect.clone());
    assert_eq!(e1.wait_for_connect(), t2.connect.clone());

    e1.send_to(second_key, msg.clone());
    assert_eq!(e2.wait_for_message(), msg);

    e1.disconnect_with(second_key);
    assert_eq!(e1.wait_for_disconnect(), second_key);
}

#[test]
fn test_network_multiple_connect() {
    let main = "127.0.0.1:19600".parse().unwrap();

    let nodes = [
        "127.0.0.1:19601".parse().unwrap(),
        "127.0.0.1:19602".parse().unwrap(),
        "127.0.0.1:19603".parse().unwrap(),
    ];

    let mut connect_list = ConnectList::default();

    let mut connection_params: Vec<_> = nodes
        .iter()
        .cloned()
        .map(ConnectionParams::from_address)
        .collect();

    for params in connection_params.iter().cloned() {
        connect_list.add(params.connect_info.clone());
    }

    let mut t1 = ConnectionParams::from_address(main);
    let main_key = t1.connect_info.public_key;

    connect_list.add(t1.connect_info.clone());

    let connect_list = SharedConnectList::from_connect_list(connect_list);
    let events = TestEvents::with_addr(t1.address, &connect_list);

    let mut node = t1.spawn(events, connect_list.clone());

    let connectors: Vec<_> = connection_params
        .iter_mut()
        .map(|params| {
            let events = TestEvents::with_addr(params.address, &connect_list);
            params.spawn(events, connect_list.clone())
        })
        .collect();

    connectors[0].connect_with(main_key, connection_params[0].connect.clone());
    assert_eq!(
        node.wait_for_connect(),
        connection_params[0].connect.clone()
    );
    connectors[1].connect_with(main_key, connection_params[1].connect.clone());
    assert_eq!(
        node.wait_for_connect(),
        connection_params[1].connect.clone()
    );
    connectors[2].connect_with(main_key, connection_params[2].connect.clone());
    assert_eq!(
        node.wait_for_connect(),
        connection_params[2].connect.clone()
    );
}

#[test]
fn test_send_first_not_connect() {
    let main = "127.0.0.1:19500".parse().unwrap();
    let other = "127.0.0.1:19501".parse().unwrap();

    let mut connect_list = ConnectList::default();
    let mut t1 = ConnectionParams::from_address(main);
    let main_key = t1.connect_info.public_key;
    connect_list.add(t1.connect_info.clone());
    let mut t2 = ConnectionParams::from_address(other);
    connect_list.add(t2.connect_info.clone());
    let connect_list = SharedConnectList::from_connect_list(connect_list);

    let node = TestEvents::with_addr(main, &connect_list);
    let other_node = TestEvents::with_addr(other, &connect_list);

    let mut node = t1.spawn(node, connect_list.clone());
    let other_node = t2.spawn(other_node, connect_list.clone());

    let message = raw_message(1000);
    other_node.send_to(main_key, message.clone()); // should connect before send message

    assert_eq!(node.wait_for_connect(), t2.connect);
    assert_eq!(node.wait_for_message(), message);
}

#[test]
#[should_panic(expected = "An error during wait for connect occurred")]
fn test_connect_list_ignore_when_connecting() {
    let first = "127.0.0.1:27230".parse().unwrap();
    let second = "127.0.0.1:27231".parse().unwrap();

    let mut connect_list = ConnectList::default();

    let mut t1 = ConnectionParams::from_address(first);
    connect_list.add(t1.connect_info.clone());

    let mut t2 = ConnectionParams::from_address(second);
    let second_key = t2.connect_info.public_key;

    let connect_list = SharedConnectList::from_connect_list(connect_list);

    let e1 = TestEvents::with_addr(first, &connect_list);
    let e2 = TestEvents::with_addr(second, &connect_list);

    let mut e1 = t1.spawn(e1, connect_list.clone());
    let mut e2 = t2.spawn(e2, connect_list);

    e1.connect_with(second_key, t1.connect.clone());
    e2.wait_for_connect();
    e1.wait_for_connect();
}

#[test]
#[should_panic(expected = "An error during wait for connect occurred")]
fn test_connect_list_ignore_when_listening() {
    let first = "127.0.0.1:20230".parse().unwrap();
    let second = "127.0.0.1:20231".parse().unwrap();

    let mut connect_list = ConnectList::default();

    let mut t1 = ConnectionParams::from_address(first);
    let first_key = t1.connect_info.public_key;
    connect_list.add(t1.connect_info.clone());

    let mut t2 = ConnectionParams::from_address(second);

    let connect_list = SharedConnectList::from_connect_list(connect_list);

    let e1 = TestEvents::with_addr(first, &connect_list);
    let e2 = TestEvents::with_addr(second, &connect_list);

    let mut e1 = t1.spawn(e1, connect_list.clone());
    let mut e2 = t2.spawn(e2, connect_list);

    e2.connect_with(first_key, t1.connect.clone());
    e1.wait_for_connect();
    e2.wait_for_connect();
}
