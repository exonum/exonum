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

#[cfg(test)]
use env_logger;

use std::io;
use std::net::SocketAddr;
use std::collections::VecDeque;
#[cfg(test)]
use std::thread;
use std::time::{SystemTime, Duration};

use messages::{MessageWriter, RawMessage};
use blockchain::SharedNodeState;
use crypto::gen_keypair;
use super::{Events, Reactor, Event, InternalEvent, Channel, Network, NetworkConfiguration,
            EventHandler};

pub type TestEvent = InternalEvent<(), u32>;

#[derive(Debug)]
pub struct BenchConfig {
    pub times: usize,
    pub len: usize,
    pub tcp_nodelay: bool,
}

#[derive(Debug, Default)]
pub struct TestHandler {
    events: VecDeque<TestEvent>,
    messages: VecDeque<RawMessage>,
}

pub trait TestPoller {
    fn event(&mut self) -> Option<TestEvent>;
    fn message(&mut self) -> Option<RawMessage>;
}

impl Default for TestHandler {
    fn default() -> Self {
        TestHandler {
            events: VecDeque::new(),
            messages: VecDeque::new(),
        }
    }
}

impl TestPoller for TestHandler {
    fn event(&mut self) -> Option<TestEvent> {
        self.events.pop_front()
    }
    fn message(&mut self) -> Option<RawMessage> {
        self.messages.pop_front()
    }
}

impl EventHandler for TestHandler {
    type Timeout = ();
    type ApplicationEvent = ();

    fn handle_event(&mut self, event: Event) {
        info!("handle event: {:?}", event);
        match event {
            Event::Incoming(raw) => self.messages.push_back(raw),
            _ => {
                self.events.push_back(InternalEvent::Node(event));
            }
        }
    }
    fn handle_timeout(&mut self, _: Self::Timeout) {}
    fn handle_application_event(&mut self, event: Self::ApplicationEvent) {
        self.events.push_back(InternalEvent::Application(event));
    }
}

#[derive(Debug)]
pub struct TestEvents(pub Events<TestHandler>);

impl TestEvents {
    pub fn with_addr(addr: SocketAddr) -> TestEvents {
        let network = Network::with_config(
            addr,
            NetworkConfiguration {
                max_incoming_connections: 128,
                max_outgoing_connections: 128,
                tcp_nodelay: true,
                tcp_keep_alive: Some(1),
                tcp_reconnect_timeout: 1000,
                tcp_reconnect_timeout_max: 600_000,
            },
            SharedNodeState::new(0),
        );
        let handler = TestHandler::default();

        TestEvents(Events::new(network, handler).unwrap())
    }

    pub fn wait_for_bind(&mut self, addr: &SocketAddr) -> Option<()> {
        self.0.bind().unwrap();
        self.wait_for_connect(addr)
    }

    pub fn wait_for_connect(&mut self, addr: &SocketAddr) -> Option<()> {
        self.0.channel().connect(addr);

        let start = SystemTime::now();
        loop {
            self.process_events().unwrap();

            if start + Duration::from_millis(10_000) < SystemTime::now() {
                return None;
            }
            while let Some(e) = self.0.inner.handler.event() {
                if let InternalEvent::Node(Event::Connected(_)) = e {
                    return Some(());
                }
            }
        }
    }

    pub fn wait_for_message(&mut self, duration: Duration) -> Option<RawMessage> {
        let start = SystemTime::now();
        loop {
            self.process_events().unwrap();

            if start + duration < SystemTime::now() {
                return None;
            }

            while let Some(e) = self.0.inner.handler.event() {
                if let InternalEvent::Node(Event::Error(e)) = e {
                    error!("An error during wait occurred {:?}", e);
                }
            }

            if let Some(msg) = self.0.inner.handler.message() {
                return Some(msg);
            }
        }
    }

    pub fn wait_for_messages(
        &mut self,
        mut count: usize,
        duration: Duration,
    ) -> Result<Vec<RawMessage>, String> {
        let mut v = Vec::new();
        let start = SystemTime::now();
        loop {
            self.process_events().unwrap();

            if start + duration < SystemTime::now() {
                return Err(format!(
                    "Timeout exceeded, {} messages is not received",
                    count
                ));
            }

            if let Some(msg) = self.0.inner.handler.message() {
                v.push(msg);
                count -= 1;
                if count == 0 {
                    return Ok(v);
                }
            }
        }
    }

    pub fn wait_for_disconnect(&mut self, max_duration: Duration) -> Option<()> {
        let start = SystemTime::now();
        loop {
            self.process_events().unwrap();

            if start + max_duration < SystemTime::now() {
                return None;
            }
            while let Some(e) = self.0.inner.handler.event() {
                if let InternalEvent::Node(Event::Disconnected(_)) = e {
                    return Some(());
                }
            }
        }
    }

    pub fn send_to(&mut self, addr: &SocketAddr, msg: RawMessage) {
        self.0.channel().send_to(addr, msg);
        self.process_events().unwrap();
    }

    pub fn process_events(&mut self) -> io::Result<()> {
        self.0.run_once(Some(100))
    }

    pub fn with_cfg(_: &BenchConfig, addr: SocketAddr) -> TestEvents {
        let network = Network::with_config(
            addr,
            NetworkConfiguration::default(),
            SharedNodeState::new(1000),
        );
        let handler = TestHandler::default();

        TestEvents(Events::new(network, handler).unwrap())
    }
}

pub fn gen_message(id: u16, len: usize) -> RawMessage {
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
fn big_message() {
    let _ = env_logger::init();
    let addrs: [SocketAddr; 2] =
        ["127.0.0.1:7200".parse().unwrap(), "127.0.0.1:7201".parse().unwrap()];

    let m1 = gen_message(15, 100000);
    let m2 = gen_message(16, 400);

    let mut e1 = TestEvents::with_addr(addrs[0]);
    let mut e2 = TestEvents::with_addr(addrs[1]);
    e1.0.bind().unwrap();
    e2.0.bind().unwrap();

    let t1;
    {
        let m1 = m1.clone();
        let m2 = m2.clone();
        t1 = thread::spawn(move || {
            let mut e = e1;
            e.wait_for_connect(&addrs[1]);

            e.send_to(&addrs[1], m1.clone());
            e.send_to(&addrs[1], m2.clone());
            e.send_to(&addrs[1], m1.clone());

            let msgs = e.wait_for_messages(3, Duration::from_millis(10000))
                .unwrap();
            assert_eq!(msgs[0], m2);
            assert_eq!(msgs[1], m1);
            assert_eq!(msgs[2], m2);
        });
    }

    let t2;
    {
        let m1 = m1.clone();
        let m2 = m2.clone();
        t2 = thread::spawn(move || {
            let mut e = e2;
            e.wait_for_connect(&addrs[0]);

            e.send_to(&addrs[0], m2.clone());
            e.send_to(&addrs[0], m1.clone());
            e.send_to(&addrs[0], m2.clone());
            let msgs = e.wait_for_messages(3, Duration::from_millis(10000))
                .unwrap();
            assert_eq!(msgs[0], m1);
            assert_eq!(msgs[1], m2);
            assert_eq!(msgs[2], m1);
            e.wait_for_disconnect(Duration::from_millis(10000)).unwrap();
        });
    }

    t2.join().unwrap();
    t1.join().unwrap();
}

#[test]
fn reconnect() {
    let _ = env_logger::init();
    let addrs: [SocketAddr; 2] =
        ["127.0.0.1:9100".parse().unwrap(), "127.0.0.1:9101".parse().unwrap()];

    let m1 = gen_message(15, 250);
    let m2 = gen_message(16, 400);
    let m3 = gen_message(17, 600);

    let mut e1 = TestEvents::with_addr(addrs[0]);
    let mut e2 = TestEvents::with_addr(addrs[1]);
    e1.0.bind().unwrap();
    e2.0.bind().unwrap();

    let t1;
    {
        let m1 = m1.clone();
        let m2 = m2.clone();
        let m3 = m3.clone();
        t1 = thread::spawn(move || {
            {
                let mut e = e1;
                e.wait_for_connect(&addrs[1]).unwrap();

                trace!("t1: connection opened");
                trace!("t1: send m1 to t2");
                e.send_to(&addrs[1], m1);
                trace!("t1: wait for m2");
                assert_eq!(e.wait_for_message(Duration::from_millis(5000)), Some(m2));
                trace!("t1: received m2 from t2");
            }
            trace!("t1: connection closed");
            {
                let mut e = TestEvents::with_addr(addrs[0]);
                e.wait_for_bind(&addrs[1]).unwrap();

                trace!("t1: connection reopened");
                trace!("t1: send m3 to t2");
                e.send_to(&addrs[1], m3.clone());
                trace!("t1: wait for m3");
                assert_eq!(e.wait_for_message(Duration::from_millis(5000)), Some(m3));
                trace!("t1: received m3 from t2");
                e.process_events().unwrap();
            }
            trace!("t1: finished");
        });
    }

    let t2;
    {
        let m1 = m1.clone();
        let m2 = m2.clone();
        let m3 = m3.clone();
        t2 = thread::spawn(move || {
            {
                let mut e = e2;
                e.wait_for_connect(&addrs[0]).unwrap();

                trace!("t2: connection opened");
                trace!("t2: send m2 to t1");
                e.send_to(&addrs[0], m2);
                trace!("t2: wait for m1");
                assert_eq!(e.wait_for_message(Duration::from_millis(5000)), Some(m1));
                trace!("t2: received m1 from t1");
                trace!("t2: wait for m3");
                assert_eq!(
                    e.wait_for_message(Duration::from_millis(5000)),
                    Some(m3.clone())
                );
                trace!("t2: received m3 from t1");
            }
            trace!("t2: connection closed");
            {
                trace!("t2: connection reopened");
                let mut e = TestEvents::with_addr(addrs[1]);
                e.wait_for_bind(&addrs[0]).unwrap();

                trace!("t2: send m3 to t1");
                e.send_to(&addrs[0], m3.clone());
                e.wait_for_disconnect(Duration::from_millis(5000)).unwrap();
            }
            trace!("t2: finished");
        });
    }

    t2.join().unwrap();
    t1.join().unwrap();
}
