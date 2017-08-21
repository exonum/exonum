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
use futures::{self, Future, Async, Poll, Sink, Stream};
use futures::stream::Peekable;
use futures::sync::mpsc;
use tokio_core::reactor::{Core, Handle, Timeout};

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::thread;
use std::time::{self, Duration};
use std::sync::{Arc, Mutex};

use blockchain::SharedNodeState;
use crypto::{gen_keypair, PublicKey, Signature};
use helpers;
use messages::{MessageWriter, RawMessage, Connect, Message};
use events::{NetworkEvent, NetworkRequest, EventHandler};
use events::network::{NetworkPart, NetworkConfiguration};
use node::{NodeChannel, EventsPoolCapacity};

struct TestHandler {
    listen_address: SocketAddr,
    network_events_rx: Option<mpsc::Receiver<NetworkEvent>>,
    network_requests_tx: mpsc::Sender<NetworkRequest>,
    handle: Handle,
}

impl TestHandler {
    fn new(
        listen_address: SocketAddr,
        handle: Handle,
        network_requests_tx: mpsc::Sender<NetworkRequest>,
        network_events_rx: mpsc::Receiver<NetworkEvent>,
    ) -> TestHandler {
        TestHandler {
            listen_address,
            network_requests_tx,
            network_events_rx: Some(network_events_rx),
            handle,
        }
    }

    fn wait_for_event(&mut self) -> Result<NetworkEvent, ()> {
        // TODO add timeout
        let mut wait = self.network_events_rx.take().unwrap().wait();
        let item = wait.next().unwrap();
        debug!("Event received {:?}", item);
        self.network_events_rx = Some(wait.into_inner());
        item
    }

    fn connect_with(&self, addr: SocketAddr) {
        let connect = connect_message(self.listen_address);
        self.network_requests_tx
            .clone()
            .send(NetworkRequest::SendMessage(addr, connect.raw().clone()))
            .wait()
            .unwrap();
    }

    fn wait_for_connect(&mut self) -> Connect {
        if let Ok(NetworkEvent::PeerConnected(_addr, connect)) = self.wait_for_event() {
            connect
        } else {
            panic!("Unexpected message received");
        }
    }
}

struct TestEvents {
    listen_address: SocketAddr,
    core: Core,
    channel: NodeChannel,
}

struct TestHandlerPart {
    handler: TestHandler,
    core: Core,
}

impl TestEvents {
    fn with_addr(listen_address: SocketAddr) -> TestEvents {
        let channel = NodeChannel::new(EventsPoolCapacity::default());
        let core = Core::new().unwrap();
        TestEvents {
            listen_address,
            core,
            channel,
        }
    }

    fn spawn<F>(mut self, test_fn: F)
    where
        F: Fn(&mut TestHandler) + 'static,
    {
        let (handler_part, network_part) = self.into_reactor();
        let network_thread = thread::spawn(move || network_part.run().unwrap());

        let mut handler = handler_part.handler;

        let mut core = handler_part.core;
        let test_fut = futures::lazy(move || -> Result<(), ()> {
            test_fn(&mut handler);
            debug!("Test fn finished");
            Ok(())
        });
        core.run(test_fut).unwrap();

        // TODO implement shutdown
        // network_thread.join().unwrap();
    }

    fn into_reactor(self) -> (TestHandlerPart, NetworkPart) {
        let channel = self.channel;
        let network_config = NetworkConfiguration::default();
        let (network_tx, network_rx) = channel.network_events;
        let network_requests_tx = channel.network_requests.0.clone();

        let network_part = NetworkPart {
            listen_address: self.listen_address,
            network_config,
            network_requests: channel.network_requests,
            network_tx: network_tx.clone(),
        };

        let handle = self.core.handle();
        let handler_part = TestHandlerPart {
            core: self.core,
            handler: TestHandler::new(self.listen_address, handle, network_requests_tx, network_rx),
        };
        (handler_part, network_part)
    }
}

// use env_logger;
// use tokio_core::reactor::{Handle, Core};

// use std::io;
// use std::thread;
// use std::net::SocketAddr;
// use std::collections::VecDeque;
// use std::time::{SystemTime, Duration};

// use messages::{MessageWriter, RawMessage};
// use blockchain::SharedNodeState;
// use crypto::gen_keypair;
// use super::{Events, Reactor, Event, InternalEvent, Channel, Network, NetworkConfiguration,
//             EventHandler};

// pub type TestEvent = InternalEvent<(), u32>;

// pub struct TestHandler {
//     events: VecDeque<TestEvent>,
//     messages: VecDeque<RawMessage>,
// }

// pub trait TestPoller {
//     fn event(&mut self) -> Option<TestEvent>;
//     fn message(&mut self) -> Option<RawMessage>;
// }

// impl TestHandler {
//     pub fn new() -> TestHandler {
//         TestHandler {
//             events: VecDeque::new(),
//             messages: VecDeque::new(),
//         }
//     }
// }

// impl TestPoller for TestHandler {
//     fn event(&mut self) -> Option<TestEvent> {
//         self.events.pop_front()
//     }
//     fn message(&mut self) -> Option<RawMessage> {
//         self.messages.pop_front()
//     }
// }

// impl EventHandler for TestHandler {
//     type Timeout = ();
//     type ApplicationEvent = ();

//     fn handle_event(&mut self, event: Event) {
//         info!("handle event: {:?}", event);
//         match event {
//             Event::Incoming(raw) => self.messages.push_back(raw),
//             _ => {
//                 self.events.push_back(InternalEvent::Node(event));
//             }
//         }
//     }
//     fn handle_timeout(&mut self, _: Self::Timeout) {}
//     fn handle_application_event(&mut self, event: Self::ApplicationEvent) {
//         self.events.push_back(InternalEvent::Application(event));
//     }
// }

// pub struct TestEvents(pub Events<TestHandler>);

// impl TestEvents {
//     pub fn with_addr(addr: SocketAddr) -> TestEvents {
//         let network = Network::with_config(
//             addr,
//             NetworkConfiguration {
//                 max_incoming_connections: 128,
//                 max_outgoing_connections: 128,
//                 tcp_nodelay: true,
//                 tcp_keep_alive: Some(1),
//                 tcp_reconnect_timeout: 1000,
//                 tcp_reconnect_timeout_max: 600000,
//             },
//             SharedNodeState::new(0),
//         );
//         let handler = TestHandler::new();

//         TestEvents(Events::new(network, handler).unwrap())
//     }

//     pub fn handle(&self) -> Handle {
//         let dummy_core = Core::new().unwrap();
//         dummy_core.handle()
//     }

//     pub fn wait_for_bind(&mut self, addr: &SocketAddr) -> Option<()> {
//         self.0.bind().unwrap();
//         self.wait_for_connect(addr)
//     }

//     pub fn wait_for_connect(&mut self, addr: &SocketAddr) -> Option<()> {
//         self.0.channel().connect(self.handle(), addr);

//         let start = SystemTime::now();
//         loop {
//             self.process_events().unwrap();

//             if start + Duration::from_millis(10000) < SystemTime::now() {
//                 return None;
//             }
//             while let Some(e) = self.0.inner.handler.event() {
//                 match e {
//                     InternalEvent::Node(Event::Connected(_)) => return Some(()),
//                     _ => {}
//                 }
//             }
//         }
//     }

//     pub fn wait_for_message(&mut self, duration: Duration) -> Option<RawMessage> {
//         let start = SystemTime::now();
//         loop {
//             self.process_events().unwrap();

//             if start + duration < SystemTime::now() {
//                 return None;
//             }

//             while let Some(e) = self.0.inner.handler.event() {
//                 match e {
//                     InternalEvent::Node(Event::Error(e)) => {
//                         error!("An error during wait occured {:?}", e);
//                     }
//                     _ => {}
//                 }
//             }

//             if let Some(msg) = self.0.inner.handler.message() {
//                 return Some(msg);
//             }
//         }
//     }

//     pub fn wait_for_messages(
//         &mut self,
//         mut count: usize,
//         duration: Duration,
//     ) -> Result<Vec<RawMessage>, String> {
//         let mut v = Vec::new();
//         let start = SystemTime::now();
//         loop {
//             self.process_events().unwrap();

//             if start + duration < SystemTime::now() {
//                 return Err(format!(
//                     "Timeout exceeded, {} messages is not received",
//                     count
//                 ));
//             }

//             if let Some(msg) = self.0.inner.handler.message() {
//                 v.push(msg);
//                 count -= 1;
//                 if count == 0 {
//                     return Ok(v);
//                 }
//             }
//         }
//     }

//     pub fn wait_for_disconnect(&mut self, max_duration: Duration) -> Option<()> {
//         let start = SystemTime::now();
//         loop {
//             self.process_events().unwrap();

//             if start + max_duration < SystemTime::now() {
//                 return None;
//             }
//             while let Some(e) = self.0.inner.handler.event() {
//                 match e {
//                     InternalEvent::Node(Event::Disconnected(_)) => return Some(()),
//                     _ => {}
//                 }
//             }
//         }
//     }

//     pub fn send_to(&mut self, addr: &SocketAddr, msg: RawMessage) {
//         self.0.channel().send_to(self.handle(), addr, msg);
//         self.process_events().unwrap();
//     }

//     pub fn process_events(&mut self) -> io::Result<()> {
//         self.0.run_once(Some(100))
//     }
// }

fn connect_message(addr: SocketAddr) -> Connect {
    let time = time::UNIX_EPOCH;
    Connect::new_with_signature(&PublicKey::zero(), addr, time, &Signature::zero())
}

fn gen_message(id: u16, len: usize) -> RawMessage {
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
    let _ = helpers::init_logger();

    let addrs: [SocketAddr; 2] =
        ["127.0.0.1:7200".parse().unwrap(), "127.0.0.1:7201".parse().unwrap()];

    let t0 = thread::spawn(move || {
        let e0 = TestEvents::with_addr(addrs[0]);
        e0.spawn(move |handler: &mut TestHandler| {
            handler.connect_with(addrs[1]);
            assert_eq!(handler.wait_for_connect(), connect_message(addrs[1]));
            debug!("t0 finished");
        });
    });
    let t1 = thread::spawn(move || {
        let e1 = TestEvents::with_addr(addrs[1]);
        e1.spawn(move |handler: &mut TestHandler| {
            assert_eq!(handler.wait_for_connect(), connect_message(addrs[0]));
            handler.connect_with(addrs[0]);
            debug!("t1 finished");
        });
    });

    t0.join().unwrap();
    t1.join().unwrap();
}

// #[test]
// fn big_message() {
//     let _ = env_logger::init();
//     let addrs: [SocketAddr; 2] =
//         ["127.0.0.1:7200".parse().unwrap(), "127.0.0.1:7201".parse().unwrap()];

//     let m1 = gen_message(15, 100000);
//     let m2 = gen_message(16, 400);

//     let mut e1 = TestEvents::with_addr(addrs[0]);
//     let mut e2 = TestEvents::with_addr(addrs[1]);
//     e1.0.bind().unwrap();
//     e2.0.bind().unwrap();

//     let t1;
//     {
//         let m1 = m1.clone();
//         let m2 = m2.clone();
//         t1 = thread::spawn(move || {
//             let mut e = e1;
//             e.wait_for_connect(&addrs[1]);

//             e.send_to(&addrs[1], m1.clone());
//             e.send_to(&addrs[1], m2.clone());
//             e.send_to(&addrs[1], m1.clone());

//             let msgs = e.wait_for_messages(3, Duration::from_millis(10000))
//                 .unwrap();
//             assert_eq!(msgs[0], m2);
//             assert_eq!(msgs[1], m1);
//             assert_eq!(msgs[2], m2);
//         });
//     }

//     let t2;
//     {
//         let m1 = m1.clone();
//         let m2 = m2.clone();
//         t2 = thread::spawn(move || {
//             let mut e = e2;
//             e.wait_for_connect(&addrs[0]);

//             e.send_to(&addrs[0], m2.clone());
//             e.send_to(&addrs[0], m1.clone());
//             e.send_to(&addrs[0], m2.clone());
//             let msgs = e.wait_for_messages(3, Duration::from_millis(10000))
//                 .unwrap();
//             assert_eq!(msgs[0], m1);
//             assert_eq!(msgs[1], m2);
//             assert_eq!(msgs[2], m1);
//             e.wait_for_disconnect(Duration::from_millis(10000)).unwrap();
//         });
//     }

//     t2.join().unwrap();
//     t1.join().unwrap();
// }

// #[test]
// fn reconnect() {
//     let _ = env_logger::init();
//     let addrs: [SocketAddr; 2] =
//         ["127.0.0.1:9100".parse().unwrap(), "127.0.0.1:9101".parse().unwrap()];

//     let m1 = gen_message(15, 250);
//     let m2 = gen_message(16, 400);
//     let m3 = gen_message(17, 600);

//     let mut e1 = TestEvents::with_addr(addrs[0]);
//     let mut e2 = TestEvents::with_addr(addrs[1]);
//     e1.0.bind().unwrap();
//     e2.0.bind().unwrap();

//     let t1;
//     {
//         let m1 = m1.clone();
//         let m2 = m2.clone();
//         let m3 = m3.clone();
//         t1 = thread::spawn(move || {
//             {
//                 let mut e = e1;
//                 e.wait_for_connect(&addrs[1]).unwrap();

//                 trace!("t1: connection opened");
//                 trace!("t1: send m1 to t2");
//                 e.send_to(&addrs[1], m1);
//                 trace!("t1: wait for m2");
//                 assert_eq!(e.wait_for_message(Duration::from_millis(5000)), Some(m2));
//                 trace!("t1: received m2 from t2");
//             }
//             trace!("t1: connection closed");
//             {
//                 let mut e = TestEvents::with_addr(addrs[0]);
//                 e.wait_for_bind(&addrs[1]).unwrap();

//                 trace!("t1: connection reopened");
//                 trace!("t1: send m3 to t2");
//                 e.send_to(&addrs[1], m3.clone());
//                 trace!("t1: wait for m3");
//                 assert_eq!(e.wait_for_message(Duration::from_millis(5000)), Some(m3));
//                 trace!("t1: received m3 from t2");
//                 e.process_events().unwrap();
//             }
//             trace!("t1: finished");
//         });
//     }

//     let t2;
//     {
//         let m1 = m1.clone();
//         let m2 = m2.clone();
//         let m3 = m3.clone();
//         t2 = thread::spawn(move || {
//             {
//                 let mut e = e2;
//                 e.wait_for_connect(&addrs[0]).unwrap();

//                 trace!("t2: connection opened");
//                 trace!("t2: send m2 to t1");
//                 e.send_to(&addrs[0], m2);
//                 trace!("t2: wait for m1");
//                 assert_eq!(e.wait_for_message(Duration::from_millis(5000)), Some(m1));
//                 trace!("t2: received m1 from t1");
//                 trace!("t2: wait for m3");
//                 assert_eq!(
//                     e.wait_for_message(Duration::from_millis(5000)),
//                     Some(m3.clone())
//                 );
//                 trace!("t2: received m3 from t1");
//             }
//             trace!("t2: connection closed");
//             {
//                 trace!("t2: connection reopened");
//                 let mut e = TestEvents::with_addr(addrs[1]);
//                 e.wait_for_bind(&addrs[0]).unwrap();

//                 trace!("t2: send m3 to t1");
//                 e.send_to(&addrs[0], m3.clone());
//                 e.wait_for_disconnect(Duration::from_millis(5000)).unwrap();
//             }
//             trace!("t2: finished");
//         });
//     }

//     t2.join().unwrap();
//     t1.join().unwrap();
// }

// #[cfg(feature = "long_benchmarks")]
// mod benches {

//     use std::thread;
//     use std::net::SocketAddr;

//     use time::Duration;

//     use events::{Network, NetworkConfiguration, Events, Reactor};
//     use super::{gen_message, TestEvents, TestHandler};

//     use test::Bencher;

//     struct BenchConfig {
//         times: usize,
//         len: usize,
//         tcp_nodelay: bool,
//     }

//     impl TestEvents {
//         fn with_cfg(cfg: &BenchConfig, addr: SocketAddr) -> TestEvents {
//             let network = Network::with_config(
//                 addr,
//                 NetworkConfiguration {
//                     max_incoming_connections: 128,
//                     max_outgoing_connections: 128,
//                     tcp_nodelay: cfg.tcp_nodelay,
//                     tcp_keep_alive: None,
//                     tcp_reconnect_timeout: 1000,
//                     tcp_reconnect_timeout_max: 600000,
//                 },
//             );
//             let handler = TestHandler::new();

//             TestEvents(Events::new(network, handler).unwrap())
//         }
//     }

//     fn bench_network(b: &mut Bencher, addrs: [SocketAddr; 2], cfg: BenchConfig) {
//         b.iter(|| {
//             let mut e1 = TestEvents::with_cfg(&cfg, addrs[0]);
//             let mut e2 = TestEvents::with_cfg(&cfg, addrs[1]);
//             e1.0.bind().unwrap();
//             e2.0.bind().unwrap();

//             let timeout = Duration::seconds(30);
//             let len = cfg.len;
//             let times = cfg.times;
//             let t1 = thread::spawn(move || {
//                 e1.wait_for_connect(&addrs[1]).unwrap();
//                 for _ in 0..times {
//                     let msg = gen_message(0, len);
//                     e1.send_to(&addrs[1], msg);
//                     e1.wait_for_messages(1, timeout).unwrap();
//                 }
//                 e1.wait_for_disconnect(Duration::from_millis(1000)).unwrap();
//             });
//             let t2 = thread::spawn(move || {
//                 e2.wait_for_connect(&addrs[0]).unwrap();
//                 for _ in 0..times {
//                     let msg = gen_message(1, len);
//                     e2.send_to(&addrs[0], msg);
//                     e2.wait_for_messages(1, timeout).unwrap();
//                 }
//             });
//             t1.join().unwrap();
//             t2.join().unwrap();
//         })
//     }

//     #[bench]
//     fn bench_msg_short_100(b: &mut Bencher) {
//         let cfg = BenchConfig {
//             tcp_nodelay: false,
//             len: 100,
//             times: 100,
//         };
//         let addrs = ["127.0.0.1:6990".parse().unwrap(), "127.0.0.1:6991".parse().unwrap()];
//         bench_network(b, addrs, cfg);
//     }

//     #[bench]
//     fn bench_msg_short_1000(b: &mut Bencher) {
//         let cfg = BenchConfig {
//             tcp_nodelay: false,
//             len: 100,
//             times: 1000,
//         };
//         let addrs = ["127.0.0.1:9792".parse().unwrap(), "127.0.0.1:9793".parse().unwrap()];
//         bench_network(b, addrs, cfg);
//     }

//     #[bench]
//     fn bench_msg_short_10000(b: &mut Bencher) {
//         let cfg = BenchConfig {
//             tcp_nodelay: false,
//             len: 100,
//             times: 10000,
//         };
//         let addrs = ["127.0.0.1:9982".parse().unwrap(), "127.0.0.1:9983".parse().unwrap()];
//         bench_network(b, addrs, cfg);
//     }

//     #[bench]
//     fn bench_msg_short_100_nodelay(b: &mut Bencher) {
//         let cfg = BenchConfig {
//             tcp_nodelay: true,
//             len: 100,
//             times: 100,
//         };
//         let addrs = ["127.0.0.1:4990".parse().unwrap(), "127.0.0.1:4991".parse().unwrap()];
//         bench_network(b, addrs, cfg);
//     }

//     #[bench]
//     fn bench_msg_short_10000_nodelay(b: &mut Bencher) {
//         let cfg = BenchConfig {
//             tcp_nodelay: true,
//             len: 100,
//             times: 10000,
//         };
//         let addrs = ["127.0.0.1:5990".parse().unwrap(), "127.0.0.1:5991".parse().unwrap()];
//         bench_network(b, addrs, cfg);
//     }

//     #[bench]
//     fn bench_msg_long_10(b: &mut Bencher) {
//         let cfg = BenchConfig {
//             tcp_nodelay: false,
//             len: 100000,
//             times: 10,
//         };
//         let addrs = ["127.0.0.1:9984".parse().unwrap(), "127.0.0.1:9985".parse().unwrap()];
//         bench_network(b, addrs, cfg);
//     }

//     #[bench]
//     fn bench_msg_long_100(b: &mut Bencher) {
//         let cfg = BenchConfig {
//             tcp_nodelay: false,
//             len: 100000,
//             times: 100,
//         };
//         let addrs = ["127.0.0.1:9946".parse().unwrap(), "127.0.0.1:9947".parse().unwrap()];
//         bench_network(b, addrs, cfg);
//     }

//     #[bench]
//     fn bench_msg_long_100_nodelay(b: &mut Bencher) {
//         let cfg = BenchConfig {
//             tcp_nodelay: true,
//             len: 100000,
//             times: 100,
//         };
//         let addrs = ["127.0.0.1:9198".parse().unwrap(), "127.0.0.1:9199".parse().unwrap()];
//         bench_network(b, addrs, cfg);
//     }
// }
