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

use futures::{sync::mpsc, Future, Sink, Stream};
use tokio::util::FutureExt;
use tokio_core::reactor::Core;

use std::{
    net::SocketAddr, thread, time::{self, Duration, SystemTime},
};

use blockchain::ConsensusConfig;
use crypto::{gen_keypair, gen_keypair_from_seed, PublicKey, SecretKey, Seed, SEED_LENGTH};
use env_logger;
use events::{
    error::log_error, network::{NetworkConfiguration, NetworkPart}, noise::HandshakeParams,
    NetworkEvent, NetworkRequest,
};
use helpers::user_agent;
use messages::{Connect, Message, MessageWriter, RawMessage};
use node::{state::SharedConnectList, ConnectInfo, ConnectList, EventsPoolCapacity, NodeChannel};

#[derive(Debug)]
pub struct TestHandler {
    handle: Option<thread::JoinHandle<()>>,
    listen_address: SocketAddr,
    network_events_rx: mpsc::Receiver<NetworkEvent>,
    network_requests_tx: mpsc::Sender<NetworkRequest>,
}

impl TestHandler {
    pub fn new(
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

    pub fn wait_for_event(&mut self) -> Result<NetworkEvent, ()> {
        let rx = self.network_events_rx.by_ref();
        let future = rx.into_future()
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

    pub fn connect_with(&self, key: PublicKey, connect: Connect) {
        self.network_requests_tx
            .clone()
            .send(NetworkRequest::SendMessage(key, connect.raw().clone()))
            .wait()
            .unwrap();
    }

    pub fn send_to(&self, key: PublicKey, raw: RawMessage) {
        self.network_requests_tx
            .clone()
            .send(NetworkRequest::SendMessage(key, raw))
            .wait()
            .unwrap();
    }

    pub fn wait_for_connect(&mut self) -> Connect {
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

    pub fn wait_for_message(&mut self) -> RawMessage {
        match self.wait_for_event() {
            Ok(NetworkEvent::MessageReceived(msg)) => msg,
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
        if !::std::thread::panicking() {
            self.shutdown();
        }
    }
}

#[derive(Debug)]
pub struct TestEvents {
    pub listen_address: SocketAddr,
    pub network_config: NetworkConfiguration,
    pub events_config: EventsPoolCapacity,
    pub connect_list: SharedConnectList,
}

impl TestEvents {
    pub fn with_addr(listen_address: SocketAddr, connect_list: &SharedConnectList) -> TestEvents {
        TestEvents {
            listen_address,
            network_config: NetworkConfiguration::default(),
            events_config: EventsPoolCapacity::default(),
            connect_list: connect_list.clone(),
        }
    }

    pub fn spawn(self, handshake_params: &HandshakeParams, connect: Connect) -> TestHandler {
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

    fn into_reactor(self, connect: Connect) -> (TestHandler, NetworkPart) {
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

pub fn connect_message(
    addr: SocketAddr,
    public_key: &PublicKey,
    secret_key: &SecretKey,
) -> Connect {
    let time = time::UNIX_EPOCH;
    Connect::new(
        public_key,
        &addr.to_string(),
        time.into(),
        &user_agent::get(),
        secret_key,
    )
}

pub fn raw_message(id: u16, len: usize) -> RawMessage {
    let writer = MessageWriter::new(::messages::PROTOCOL_MAJOR_VERSION, 0, id, len);
    RawMessage::new(writer.sign(&gen_keypair().1))
}

#[derive(Debug, Clone)]
pub struct ConnectionParams {
    pub connect: Connect,
    pub connect_info: ConnectInfo,
    address: SocketAddr,
    public_key: PublicKey,
    secret_key: SecretKey,
    handshake_params: HandshakeParams,
}

impl HandshakeParams {
    // Helper method to create `HandshakeParams` with empty `ConnectList` and
    // default `max_message_len`.
    #[doc(hidden)]
    pub fn with_default_params() -> Self {
        let (public_key, secret_key) = gen_keypair_from_seed(&Seed::new([1; SEED_LENGTH]));
        let address = "127.0.0.1:8000";

        let connect = Connect::new(
            &public_key,
            address,
            SystemTime::now().into(),
            &user_agent::get(),
            &secret_key,
        );

        let mut params = HandshakeParams::new(
            public_key,
            secret_key.clone(),
            SharedConnectList::default(),
            connect,
            ConsensusConfig::DEFAULT_MAX_MESSAGE_LEN,
        );

        params.set_remote_key(public_key);
        params
    }
}

impl ConnectionParams {
    pub fn from_address(address: SocketAddr) -> Self {
        let (public_key, secret_key) = gen_keypair();
        let connect = connect_message(address, &public_key, &secret_key);
        let handshake_params = HandshakeParams::new(
            public_key,
            secret_key.clone(),
            SharedConnectList::default(),
            connect.clone(),
            ConsensusConfig::DEFAULT_MAX_MESSAGE_LEN,
        );
        let connect_info = ConnectInfo {
            address: address.to_string(),
            public_key,
        };

        ConnectionParams {
            connect,
            address,
            public_key,
            secret_key,
            handshake_params,
            connect_info,
        }
    }

    pub fn spawn(&mut self, events: TestEvents, connect_list: SharedConnectList) -> TestHandler {
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

    let m1 = raw_message(15, 100000);
    let m2 = raw_message(16, 400);

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
    let _ = env_logger::try_init();
    let first = "127.0.0.1:17202".parse().unwrap();
    let second = "127.0.0.1:17303".parse().unwrap();

    let max_message_length = ConsensusConfig::DEFAULT_MAX_MESSAGE_LEN as usize;
    let max_payload_length =
        max_message_length - ::messages::HEADER_LENGTH - ::crypto::SIGNATURE_LENGTH;
    let acceptable_message = raw_message(15, max_payload_length);
    let too_big_message = raw_message(16, max_payload_length + 1000);

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
    assert!(e1.wait_for_event().is_err());
}

#[test]
fn test_network_reconnect() {
    let first = "127.0.0.1:19100".parse().unwrap();
    let second = "127.0.0.1:19101".parse().unwrap();

    let msg = raw_message(11, 1000);

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
        .map(|addr| ConnectionParams::from_address(addr))
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

    let message = raw_message(11, 1000);
    other_node.send_to(main_key, message.clone()); // should connect before send message

    assert_eq!(node.wait_for_connect(), t2.connect);
    assert_eq!(node.wait_for_message(), message);
}

#[test]
#[should_panic(expected = "An error during wait for connect occurred")]
fn test_connect_list_ignore_when_connecting() {
    let first = "127.0.0.1:20230".parse().unwrap();
    let second = "127.0.0.1:20231".parse().unwrap();

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
