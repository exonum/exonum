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

use futures::{stream::Wait, sync::mpsc, Future, Sink, Stream};
use tokio_core::reactor::Core;
use tokio_timer::{TimeoutStream, Timer};

use std::{
    net::SocketAddr, thread, time::{self, Duration},
};

use blockchain::ConsensusConfig;
use crypto::{gen_keypair_from_seed, PublicKey, SecretKey, Seed};
use events::{
    error::log_error, network::{NetworkConfiguration, NetworkPart}, noise::HandshakeParams,
    NetworkEvent, NetworkRequest,
};
use helpers::user_agent;
use messages::{self, Connect, Message, SignedMessage};
use node::{EventsPoolCapacity, NodeChannel};

//TODO: Reduce crypto usage in tests.
static FAKE_SEED: [u8; 32] = [1; 32];

#[derive(Debug)]
pub struct TestHandler {
    handle: Option<thread::JoinHandle<()>>,
    listen_address: SocketAddr,
    network_events_rx: Wait<TimeoutStream<mpsc::Receiver<NetworkEvent>>>,
    network_requests_tx: mpsc::Sender<NetworkRequest>,
    keypair: (PublicKey, SecretKey),
}

impl TestHandler {
    pub fn new(
        listen_address: SocketAddr,
        network_requests_tx: mpsc::Sender<NetworkRequest>,
        network_events_rx: mpsc::Receiver<NetworkEvent>,
    ) -> TestHandler {
        let timer = Timer::default();
        let receiver = timer.timeout_stream(network_events_rx, Duration::from_secs(30));
        TestHandler {
            handle: None,
            listen_address,
            network_requests_tx,
            keypair: gen_keypair_from_seed(&Seed::new([12; 32])),
            network_events_rx: receiver.wait(),
        }
    }

    pub fn wait_for_event(&mut self) -> Result<NetworkEvent, ()> {
        let event = self.network_events_rx.next().unwrap()?;
        Ok(event)
    }

    pub fn connect_with(&self, addr: SocketAddr) {
        let connect = connect_message(self.listen_address, (self.keypair.0, &self.keypair.1));
        let (public_key, _) = gen_keypair_from_seed(&Seed::new(FAKE_SEED));
        self.network_requests_tx
            .clone()
            .send(NetworkRequest::SendMessage(
                addr,
                connect.into(),
                public_key,
            ))
            .wait()
            .unwrap();
    }

    pub fn disconnect_with(&self, addr: SocketAddr) {
        self.network_requests_tx
            .clone()
            .send(NetworkRequest::DisconnectWithPeer(addr))
            .wait()
            .unwrap();
    }

    pub fn send_to(&self, addr: SocketAddr, raw: SignedMessage) {
        let (public_key, _) = gen_keypair_from_seed(&Seed::new(FAKE_SEED));
        self.network_requests_tx
            .clone()
            .send(NetworkRequest::SendMessage(addr, raw, public_key))
            .wait()
            .unwrap();
    }

    pub fn wait_for_connect(&mut self) -> Message<Connect> {
        match self.wait_for_event() {
            Ok(NetworkEvent::PeerConnected(_addr, connect)) => connect,
            Ok(other) => panic!("Unexpected connect received, {:?}", other),
            Err(e) => panic!("An error during wait for connect occurred, {:?}", e),
        }
    }

    pub fn wait_for_disconnect(&mut self) -> SocketAddr {
        match self.wait_for_event() {
            Ok(NetworkEvent::PeerDisconnected(addr)) => addr,
            Ok(other) => panic!("Unexpected disconnect received, {:?}", other),
            Err(e) => panic!("An error during wait for disconnect occurred, {:?}", e),
        }
    }

    pub fn wait_for_message(&mut self) -> SignedMessage {
        match self.wait_for_event() {
            Ok(NetworkEvent::MessageReceived(_addr, msg)) => {
                SignedMessage::verify_buffer(msg).unwrap()
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
}

impl TestEvents {
    pub fn with_addr(listen_address: SocketAddr) -> TestEvents {
        Self {
            listen_address,
            network_config: NetworkConfiguration::default(),
            events_config: EventsPoolCapacity::default(),
        }
    }

    pub fn spawn(self) -> TestHandler {

        let (mut handler_part, network_part) = self.into_reactor();
        let handle = thread::spawn(move || {
            let mut core = Core::new().unwrap();
            let (p, k) = gen_keypair_from_seed(&Seed::new(FAKE_SEED));
            let handshake_params =
                HandshakeParams::new(p, k, network_part.max_message_len);
            let fut = network_part.run(&core.handle(), &handshake_params);
            core.run(fut).map_err(log_error).unwrap();
        });
        handler_part.handle = Some(handle);
        handler_part
    }

    fn into_reactor(self) -> (TestHandler, NetworkPart) {
        let channel = NodeChannel::new(&self.events_config);
        let network_config = self.network_config;
        let (network_tx, network_rx) = channel.network_events;
        let network_requests_tx = channel.network_requests.0.clone();
        let (p, k) = gen_keypair_from_seed(&Seed::new(FAKE_SEED));
        let network_part = NetworkPart {
            our_connect_message: connect_message(
                self.listen_address,
                (p, &k),
            ),
            listen_address: self.listen_address,
            network_config,
            max_message_len: ConsensusConfig::DEFAULT_MAX_MESSAGE_LEN,
            network_requests: channel.network_requests,
            network_tx: network_tx.clone(),
        };

        let handler_part = TestHandler::new(self.listen_address, network_requests_tx, network_rx);
        (handler_part, network_part)
    }
}

pub fn connect_message(addr: SocketAddr, keypair: (PublicKey, &SecretKey)) -> Message<Connect> {
    let time = time::UNIX_EPOCH;

    let connect = Connect::new(addr, time.into(), &user_agent::get());
    Message::new(connect, keypair.0, keypair.1)
}

pub fn raw_message(id: u16, tx_len: usize, keypair: (PublicKey, &SecretKey)) -> SignedMessage {
    Message::create_raw_tx(vec![0; tx_len], id, keypair).into()
}

#[test]
fn test_network_handshake() {
    let first = "127.0.0.1:17230".parse().unwrap();
    let second = "127.0.0.1:17231".parse().unwrap();
    let e1 = TestEvents::with_addr(first);
    let e2 = TestEvents::with_addr(second);

    let (p, s) = gen_keypair_from_seed(&Seed::new(FAKE_SEED));
    let c1 = connect_message(first, (p, &s));
    let c2 = connect_message(second, (p, &s));

    let mut e1 = e1.spawn();
    let mut e2 = e2.spawn();

    e1.connect_with(second);
    assert_eq!(e2.wait_for_connect(), c1);

    e2.connect_with(first);
    assert_eq!(e1.wait_for_connect(), c2);

    e1.disconnect_with(second);
    assert_eq!(e1.wait_for_disconnect(), second);

    e2.disconnect_with(first);
    assert_eq!(e2.wait_for_disconnect(), first);
}

#[test]
fn test_network_big_message() {
    let first = "127.0.0.1:17200".parse().unwrap();
    let second = "127.0.0.1:17201".parse().unwrap();
    let (p, s) = gen_keypair_from_seed(&Seed::new(FAKE_SEED));
    let keypair = (p, &s);
    let m1 = raw_message(15, 100000, keypair);
    let m2 = raw_message(16, 400, keypair);
    let e1 = TestEvents::with_addr(first);
    let e2 = TestEvents::with_addr(second);

    let mut e1 = e1.spawn();
    let mut e2 = e2.spawn();

    e1.connect_with(second);
    e2.wait_for_connect();

    e2.connect_with(first);
    e1.wait_for_connect();

    e1.send_to(second, m1.clone());
    assert_eq!(e2.wait_for_message(), m1);

    e1.send_to(second, m2.clone());
    assert_eq!(e2.wait_for_message(), m2);

    e1.send_to(second, m1.clone());
    assert_eq!(e2.wait_for_message(), m1);

    e2.send_to(first, m2.clone());
    assert_eq!(e1.wait_for_message(), m2);

    e2.send_to(first, m1.clone());
    assert_eq!(e1.wait_for_message(), m1);

    e2.send_to(first, m2.clone());
    assert_eq!(e1.wait_for_message(), m2);

    e1.disconnect_with(second);
    assert_eq!(e1.wait_for_disconnect(), second);

    e2.disconnect_with(first);
    assert_eq!(e2.wait_for_disconnect(), first);
}

#[test]
fn test_network_max_message_len() {
    let first = "127.0.0.1:17202".parse().unwrap();
    let second = "127.0.0.1:17303".parse().unwrap();

    let (p, s) = gen_keypair_from_seed(&Seed::new(FAKE_SEED));
    let keypair = (p, &s);
    let max_message_length = ConsensusConfig::DEFAULT_MAX_MESSAGE_LEN as usize;
    let max_payload_length = max_message_length - messages::RAW_TRANSACTION_EMPTY_SIZE;
    let acceptable_message = raw_message(15, max_payload_length, keypair);
    let too_big_message = raw_message(16, max_payload_length + 1000, keypair);

    let e1 = TestEvents::with_addr(first);
    let e2 = TestEvents::with_addr(second);

    let mut e1 = e1.spawn();
    let mut e2 = e2.spawn();

    e1.connect_with(second);
    e2.wait_for_connect();

    e2.connect_with(first);
    e1.wait_for_connect();

    e1.send_to(second, acceptable_message.clone());
    assert_eq!(e2.wait_for_message(), acceptable_message);

    e2.send_to(first, too_big_message.clone());
    assert!(e1.wait_for_event().is_err());
}

#[test]
fn test_network_reconnect() {
    let first = "127.0.0.1:19100".parse().unwrap();
    let second = "127.0.0.1:19101".parse().unwrap();
    let (p, s) = gen_keypair_from_seed(&Seed::new(FAKE_SEED));
    let keypair = (p, &s);

    let msg = raw_message(11, 1000, keypair);
    let c1 = connect_message(first, keypair);
    let mut t1 = TestEvents::with_addr(first).spawn();

    // First connect attempt.
    let mut t2 = TestEvents::with_addr(second).spawn();

    // Handle first attempt.
    t1.connect_with(second);
    assert_eq!(t2.wait_for_connect(), c1);

    t1.send_to(second, msg.clone());
    assert_eq!(t2.wait_for_message(), msg);

    t1.disconnect_with(second);
    drop(t2);
    assert_eq!(t1.wait_for_disconnect(), second);

    // Handle second attempt.
    let mut t2 = TestEvents::with_addr(second).spawn();

    t1.connect_with(second);
    assert_eq!(t2.wait_for_connect(), c1);

    t1.send_to(second, msg.clone());
    assert_eq!(t2.wait_for_message(), msg);

    t1.disconnect_with(second);
    assert_eq!(t1.wait_for_disconnect(), second);
}

#[test]
fn test_network_multiple_connect() {
    let main = "127.0.0.1:19600".parse().unwrap();
    let nodes = [
        "127.0.0.1:19601".parse().unwrap(),
        "127.0.0.1:19602".parse().unwrap(),
        "127.0.0.1:19603".parse().unwrap(),
    ];

    let mut node = TestEvents::with_addr(main).spawn();
    let keypair = gen_keypair_from_seed(&Seed::new(FAKE_SEED));
    let connect_messages: Vec<_> = nodes
        .iter()
        .cloned()
        .map(|a| connect_message(a, (keypair.0, &keypair.1)))
        .collect();

    let connectors: Vec<_> = nodes
        .iter()
        .map(|addr| TestEvents::with_addr(*addr).spawn())
        .collect();

    connectors[0].connect_with(main);
    assert_eq!(node.wait_for_connect(), connect_messages[0]);
    connectors[1].connect_with(main);
    assert_eq!(node.wait_for_connect(), connect_messages[1]);
    connectors[2].connect_with(main);
    assert_eq!(node.wait_for_connect(), connect_messages[2]);
}

#[test]
fn test_send_first_not_connect() {
    let main = "127.0.0.1:19500".parse().unwrap();
    let other = "127.0.0.1:19501".parse().unwrap();
    let mut node = TestEvents::with_addr(main).spawn();
    let other_node = TestEvents::with_addr(other).spawn();

    let keypair = gen_keypair_from_seed(&Seed::new(FAKE_SEED));
    let message = raw_message(11, 1000, (keypair.0, &keypair.1));
    other_node.send_to(main, message.clone()); // should connect before send message

    assert_eq!(
        node.wait_for_connect(),
        connect_message(other, (keypair.0, &keypair.1))
    );
    assert_eq!(node.wait_for_message(), message);
}
