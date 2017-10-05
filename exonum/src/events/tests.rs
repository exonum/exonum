// Copyright 2017 The Exonum Team
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
use futures::{Future, Sink, Stream};
use futures::stream::Wait;
use futures::sync::mpsc;
use tokio_core::reactor::Core;
use tokio_timer::{TimeoutStream, Timer};

use std::net::SocketAddr;
use std::thread;
use std::time::{self, Duration};

use crypto::{gen_keypair, PublicKey, Signature};
use messages::{Connect, Message, MessageWriter, RawMessage};
use events::{NetworkEvent, NetworkRequest};
use events::network::{NetworkConfiguration, NetworkPart};
use node::{EventsPoolCapacity, NodeChannel};

#[derive(Debug)]
pub struct TestHandler {
    listen_address: SocketAddr,
    network_events_rx: Wait<TimeoutStream<mpsc::Receiver<NetworkEvent>>>,
    network_requests_tx: mpsc::Sender<NetworkRequest>,
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
            listen_address,
            network_requests_tx,
            network_events_rx: receiver.wait(),
        }
    }

    pub fn wait_for_event(&mut self) -> Result<NetworkEvent, ()> {
        let event = self.network_events_rx.next().unwrap()?;
        Ok(event)
    }

    pub fn connect_with(&self, addr: SocketAddr) {
        let connect = connect_message(self.listen_address);
        self.network_requests_tx
            .clone()
            .send(NetworkRequest::SendMessage(addr, connect.raw().clone()))
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

    pub fn send_to(&self, addr: SocketAddr, raw: RawMessage) {
        self.network_requests_tx
            .clone()
            .send(NetworkRequest::SendMessage(addr, raw))
            .wait()
            .unwrap();
    }

    pub fn wait_for_connect(&mut self) -> Connect {
        match self.wait_for_event() {
            Ok(NetworkEvent::PeerConnected(_addr, connect)) => connect,
            Ok(other) => panic!("Unexpected connect received, {:?}", other),
            Err(e) => panic!("An error during wait for connect occured, {:?}", e),
        }
    }

    pub fn wait_for_disconnect(&mut self) -> SocketAddr {
        match self.wait_for_event() {
            Ok(NetworkEvent::PeerDisconnected(addr)) => addr,
            Ok(other) => panic!("Unexpected disconnect received, {:?}", other),
            Err(e) => panic!("An error during wait for disconnect occured, {:?}", e),
        }
    }

    pub fn wait_for_message(&mut self) -> RawMessage {
        match self.wait_for_event() {
            Ok(NetworkEvent::MessageReceived(_addr, msg)) => msg,
            Ok(other) => panic!("Unexpected message received, {:?}", other),
            Err(e) => panic!("An error during wait for message occured, {:?}", e),
        }
    }
}

impl Drop for TestHandler {
    fn drop(&mut self) {
        if !::std::thread::panicking() {
            self.network_requests_tx
                .clone()
                .send(NetworkRequest::Shutdown)
                .wait()
                .unwrap();
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
        TestEvents {
            listen_address,
            network_config: NetworkConfiguration::default(),
            events_config: EventsPoolCapacity::default(),
        }
    }

    pub fn spawn(self) -> TestHandler
    {
        use tokio_core::reactor::Timeout;
        use std::time::Duration;
        let (handler_part, network_part) = self.into_reactor();
        thread::spawn(move || {
            let mut core = Core::new().unwrap();
            let fut = network_part.run(core.handle());
            core.run(fut)
        });
        handler_part
    }

    fn into_reactor(self) -> (TestHandler, NetworkPart) {
        let channel = NodeChannel::new(self.events_config);
        let network_config = self.network_config;
        let (network_tx, network_rx) = channel.network_events;
        let network_requests_tx = channel.network_requests.0.clone();

        let network_part = NetworkPart {
            listen_address: self.listen_address,
            network_config,
            network_requests: channel.network_requests,
            network_tx: network_tx.clone(),
        };

        let handler_part =
            TestHandler::new(self.listen_address, network_requests_tx, network_rx);
        (handler_part, network_part)
    }
}

pub fn connect_message(addr: SocketAddr) -> Connect {
    let time = time::UNIX_EPOCH;
    Connect::new_with_signature(&PublicKey::zero(), addr, time, &Signature::zero())
}

pub fn raw_message(id: u16, len: usize) -> RawMessage {
    let writer = MessageWriter::new(
        ::messages::PROTOCOL_MAJOR_VERSION,
        ::messages::TEST_NETWORK_ID,
        0,
        id,
        len,
    );
    RawMessage::new(writer.sign(&gen_keypair().1))
}

#[test]
fn test_network_handshake() {
    extern crate env_logger;
    drop(env_logger::init());
    use std::{thread, time};
    let addrs: [SocketAddr; 2] =
        ["127.0.0.1:17230".parse().unwrap(), "127.0.0.1:17231".parse().unwrap()];

    let e1 = TestEvents::with_addr(addrs[0]);
    let e2 = TestEvents::with_addr(addrs[1]);

    let c1 = connect_message(addrs[0]);
    let c2 = connect_message(addrs[1]);

    let mut e1 = e1.spawn();
    let mut e2 = e2.spawn();

    e1.connect_with(addrs[1]);
    assert_eq!(e2.wait_for_connect(), c1);

    e2.connect_with(addrs[0]);
    assert_eq!(e1.wait_for_connect(), c2);

    e1.disconnect_with(addrs[1]);
    assert_eq!(e1.wait_for_disconnect(), addrs[1]);

    e2.disconnect_with(addrs[0]);
    assert_eq!(e2.wait_for_disconnect(), addrs[0]);
}

/*
#[test]
fn test_network_big_message() {
    let addrs: [SocketAddr; 2] =
        ["127.0.0.1:17200".parse().unwrap(), "127.0.0.1:17201".parse().unwrap()];

    let msg1 = raw_message(15, 100000);
    let msg2 = raw_message(16, 400);

    let e1 = TestEvents::with_addr(addrs[0]);
    let e2 = TestEvents::with_addr(addrs[1]);

    let m1 = msg1.clone();
    let m2 = msg2.clone();
    let t1 = e1.spawn(move |e: &mut TestHandler| {
        e.connect_with(addrs[1]);
        e.wait_for_connect();

        e.send_to(addrs[1], m1.clone());
        e.send_to(addrs[1], m2.clone());
        e.send_to(addrs[1], m1.clone());

        assert_eq!(e.wait_for_message(), m2);
        assert_eq!(e.wait_for_message(), m1);
        assert_eq!(e.wait_for_message(), m2);

        e.disconnect_with(addrs[1]);
        assert_eq!(e.wait_for_disconnect(), addrs[1]);
    });

    let m1 = msg1.clone();
    let m2 = msg2.clone();
    let t2 = e2.spawn(move |e: &mut TestHandler| {
        e.connect_with(addrs[0]);
        e.wait_for_connect();

        e.send_to(addrs[0], m2.clone());
        e.send_to(addrs[0], m1.clone());
        e.send_to(addrs[0], m2.clone());

        assert_eq!(e.wait_for_message(), m1);
        assert_eq!(e.wait_for_message(), m2);
        assert_eq!(e.wait_for_message(), m1);

        e.disconnect_with(addrs[0]);
        assert_eq!(e.wait_for_disconnect(), addrs[0]);
    });

    t2.join().unwrap();
    t1.join().unwrap();
}

#[test]
fn test_network_reconnect() {
    let addrs: [SocketAddr; 2] =
        ["127.0.0.1:19100".parse().unwrap(), "127.0.0.1:19101".parse().unwrap()];

    let msg = raw_message(11, 1000);
    let c1 = connect_message(addrs[0]);
    let c2 = connect_message(addrs[1]);
    let msg_cloned = msg.clone();
    let t1 = TestEvents::with_addr(addrs[0]).spawn(move |e: &mut TestHandler| {
        // Handle first attempt
        e.connect_with(addrs[1]);
        assert_eq!(e.wait_for_connect(), c2);
        assert_eq!(e.wait_for_message(), msg_cloned);
        assert_eq!(e.wait_for_disconnect(), addrs[1]);
        // Handle second attempt
        assert_eq!(e.wait_for_connect(), c2);
        e.connect_with(addrs[1]);
        assert_eq!(e.wait_for_message(), msg_cloned);
        e.disconnect_with(addrs[1]);
        assert_eq!(e.wait_for_disconnect(), addrs[1]);
    });
    // First connect attempt.
    let c1_cloned = c1.clone();
    let msg_cloned = msg.clone();
    TestEvents::with_addr(addrs[1])
        .spawn(move |e: &mut TestHandler| {
            assert_eq!(e.wait_for_connect(), c1_cloned);
            e.connect_with(addrs[0]);
            e.send_to(addrs[0], msg_cloned.clone());
            e.disconnect_with(addrs[0]);
            assert_eq!(e.wait_for_disconnect(), addrs[0]);
        })
        .join()
        .unwrap();
    // Second connect attempt.
    let c1_cloned = c1.clone();
    let msg_cloned = msg.clone();
    TestEvents::with_addr(addrs[1])
        .spawn(move |e: &mut TestHandler| {
            e.connect_with(addrs[0]);
            assert_eq!(e.wait_for_connect(), c1_cloned);
            e.send_to(addrs[0], msg_cloned.clone());
            e.disconnect_with(addrs[0]);
            assert_eq!(e.wait_for_disconnect(), addrs[0]);
        })
        .join()
        .unwrap();
    // Wait for first server
    t1.join().unwrap();
}
*/
