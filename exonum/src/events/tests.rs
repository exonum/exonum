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

use futures::{self, Future, Stream, Sink};
use futures::stream::Wait;
use futures::sync::mpsc;
use tokio_core::reactor::Core;
use tokio_timer::{TimeoutStream, Timer};

use std::net::SocketAddr;
use std::thread;
use std::time::{self, Duration};

use crypto::{gen_keypair, PublicKey, Signature};
use messages::{MessageWriter, RawMessage, Connect, Message};
use events::{NetworkEvent, NetworkRequest};
use events::network::{NetworkPart, NetworkConfiguration};
use node::{NodeChannel, EventsPoolCapacity};

struct TestHandler {
    listen_address: SocketAddr,
    network_events_rx: Wait<TimeoutStream<mpsc::Receiver<NetworkEvent>>>,
    network_requests_tx: mpsc::Sender<NetworkRequest>,
}

impl TestHandler {
    fn new(
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

    fn wait_for_event(&mut self) -> Result<NetworkEvent, ()> {
        let event = self.network_events_rx.next().unwrap()?;
        Ok(event)
    }

    fn connect_with(&self, addr: SocketAddr) {
        let connect = connect_message(self.listen_address);
        self.network_requests_tx
            .clone()
            .send(NetworkRequest::SendMessage(addr, connect.raw().clone()))
            .wait()
            .unwrap();
    }

    fn disconnect_with(&self, addr: SocketAddr) {
        self.network_requests_tx
            .clone()
            .send(NetworkRequest::DisconnectWithPeer(addr))
            .wait()
            .unwrap();
    }

    fn send_to(&self, addr: SocketAddr, raw: RawMessage) {
        self.network_requests_tx
            .clone()
            .send(NetworkRequest::SendMessage(addr, raw))
            .wait()
            .unwrap();
    }

    fn wait_for_connect(&mut self) -> Connect {
        match self.wait_for_event() {
            Ok(NetworkEvent::PeerConnected(_addr, connect)) => connect,
            Ok(other) => panic!("Unexpected connect received, {:?}", other),
            Err(e) => panic!("An error occured, {:?}", e),
        }
    }

    fn wait_for_disconnect(&mut self) -> SocketAddr {
        match self.wait_for_event() {
            Ok(NetworkEvent::PeerDisconnected(addr)) => addr,
            Ok(other) => panic!("Unexpected disconnect received, {:?}", other),
            Err(e) => panic!("An error occured, {:?}", e),
        }
    }

    fn wait_for_message(&mut self) -> RawMessage {
        match self.wait_for_event() {
            Ok(NetworkEvent::MessageReceived(_addr, msg)) => msg,
            Ok(other) => panic!("Unexpected message received, {:?}", other),
            Err(e) => panic!("An error occured, {:?}", e),
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

struct TestEvents {
    listen_address: SocketAddr,
    network_config: NetworkConfiguration,
    events_config: EventsPoolCapacity,
}

struct TestHandlerPart {
    handler: TestHandler,
    core: Core,
}

impl TestEvents {
    fn with_addr(listen_address: SocketAddr) -> TestEvents {
        TestEvents {
            listen_address,
            network_config: NetworkConfiguration::default(),
            events_config: EventsPoolCapacity::default(),
        }
    }

    fn spawn<F>(self, test_fn: F) -> thread::JoinHandle<()>
    where
        F: Fn(&mut TestHandler) + 'static + Send,
    {
        thread::spawn(move || {
            let (handler_part, network_part) = self.into_reactor();
            let network_thread = thread::spawn(move || network_part.run().unwrap());

            let mut handler = handler_part.handler;

            let mut core = handler_part.core;
            let test_fut = futures::lazy(move || -> Result<(), ()> {
                test_fn(&mut handler);
                Ok(())
            });
            core.run(test_fut).unwrap();
            network_thread.join().unwrap();
        })
    }

    fn into_reactor(self) -> (TestHandlerPart, NetworkPart) {
        let channel = NodeChannel::new(self.events_config);
        let core = Core::new().unwrap();
        let network_config = self.network_config;
        let (network_tx, network_rx) = channel.network_events;
        let network_requests_tx = channel.network_requests.0.clone();

        let network_part = NetworkPart {
            listen_address: self.listen_address,
            network_config,
            network_requests: channel.network_requests,
            network_tx: network_tx.clone(),
        };

        let handler_part = TestHandlerPart {
            core,
            handler: TestHandler::new(self.listen_address, network_requests_tx, network_rx),
        };
        (handler_part, network_part)
    }
}

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
    let addrs: [SocketAddr; 2] =
        ["127.0.0.1:17230".parse().unwrap(), "127.0.0.1:17231".parse().unwrap()];

    let e1 = TestEvents::with_addr(addrs[0]);
    let e2 = TestEvents::with_addr(addrs[1]);

    let c1 = connect_message(addrs[0]);
    let c2 = connect_message(addrs[1]);

    let t1 = e1.spawn(move |e: &mut TestHandler| {
        e.connect_with(addrs[1]);
        assert_eq!(e.wait_for_connect(), c2);
    });
    let t2 = e2.spawn(move |e: &mut TestHandler| {
        assert_eq!(e.wait_for_connect(), c1);
        e.connect_with(addrs[0]);
        e.disconnect_with(addrs[0]);
        assert_eq!(e.wait_for_disconnect(), addrs[0]);
    });

    t1.join().unwrap();
    t2.join().unwrap();
}

#[test]
fn test_network_big_message() {
    let addrs: [SocketAddr; 2] =
        ["127.0.0.1:17200".parse().unwrap(), "127.0.0.1:17201".parse().unwrap()];

    let msg1 = gen_message(15, 100000);
    let msg2 = gen_message(16, 400);

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


    let c1 = connect_message(addrs[0]);
    let c2 = connect_message(addrs[1]);
    let t1 = TestEvents::with_addr(addrs[0]).spawn(move |e: &mut TestHandler| {
        // Handle first attempt
        e.connect_with(addrs[1]);
        assert_eq!(e.wait_for_connect(), c2);
        assert_eq!(e.wait_for_disconnect(), addrs[1]);
        // Handle second attempt
        assert_eq!(e.wait_for_connect(), c2);
        e.connect_with(addrs[1]);
        e.disconnect_with(addrs[1]);
        assert_eq!(e.wait_for_disconnect(), addrs[1]);
    });
    // First connect attempt.
    let c1_cloned = c1.clone();
    TestEvents::with_addr(addrs[1])
        .spawn(move |e: &mut TestHandler| {
            assert_eq!(e.wait_for_connect(), c1_cloned);
            e.connect_with(addrs[0]);
            e.disconnect_with(addrs[0]);
            assert_eq!(e.wait_for_disconnect(), addrs[0]);
        })
        .join()
        .unwrap();
    // Second connect attempt.
    let c1_cloned = c1.clone();
    TestEvents::with_addr(addrs[1])
        .spawn(move |e: &mut TestHandler| {
            e.connect_with(addrs[0]);
            assert_eq!(e.wait_for_connect(), c1_cloned);
            assert_eq!(e.wait_for_disconnect(), addrs[0]);
        })
        .join()
        .unwrap();
    // Wait for first server
    t1.join().unwrap();
}

#[cfg(feature = "network_benchmarks")]
mod benches {
    use super::*;

    use test::Bencher;

    struct BenchConfig {
        times: usize,
        len: usize,
        tcp_nodelay: bool,
    }

    impl TestEvents {
        fn with_cfg(cfg: &BenchConfig, listen_address: SocketAddr) -> TestEvents {
            let network_config = NetworkConfiguration {
                tcp_nodelay: cfg.tcp_nodelay,
                ..Default::default()
            };
            TestEvents {
                listen_address,
                network_config,
                events_config: EventsPoolCapacity::default(),
            }
        }
    }

    fn bench_network(b: &mut Bencher, addrs: [SocketAddr; 2], cfg: BenchConfig) {
        b.iter(|| {
            let times = cfg.times;
            let len = cfg.len;

            let c1 = connect_message(addrs[0]);
            let c2 = connect_message(addrs[1]);

            let t1 = TestEvents::with_cfg(&cfg, addrs[0]).spawn(move |e: &mut TestHandler| {
                e.connect_with(addrs[1]);
                assert_eq!(e.wait_for_connect(), c2);
                for _ in 0..times {
                    let msg = gen_message(0, len);
                    e.send_to(addrs[1], msg);
                    e.wait_for_message();
                }
                e.disconnect_with(addrs[1]);
                assert_eq!(e.wait_for_disconnect(), addrs[1]);
            });
            let t2 = TestEvents::with_cfg(&cfg, addrs[1]).spawn(move |e: &mut TestHandler| {
                assert_eq!(e.wait_for_connect(), c1);
                e.connect_with(addrs[0]);
                for _ in 0..times {
                    let msg = gen_message(1, len);
                    e.send_to(addrs[0], msg);
                    e.wait_for_message();
                }
                assert_eq!(e.wait_for_disconnect(), addrs[0]);
            });

            t1.join().unwrap();
            t2.join().unwrap();
        })
    }

    #[bench]
    fn bench_msg_short_100(b: &mut Bencher) {
        let cfg = BenchConfig {
            tcp_nodelay: false,
            len: 100,
            times: 100,
        };
        let addrs = ["127.0.0.1:6990".parse().unwrap(), "127.0.0.1:6991".parse().unwrap()];
        bench_network(b, addrs, cfg);
    }

    #[bench]
    fn bench_msg_short_1000(b: &mut Bencher) {
        let cfg = BenchConfig {
            tcp_nodelay: false,
            len: 100,
            times: 1000,
        };
        let addrs = ["127.0.0.1:9792".parse().unwrap(), "127.0.0.1:9793".parse().unwrap()];
        bench_network(b, addrs, cfg);
    }

    #[bench]
    fn bench_msg_short_100_nodelay(b: &mut Bencher) {
        let cfg = BenchConfig {
            tcp_nodelay: true,
            len: 100,
            times: 100,
        };
        let addrs = ["127.0.0.1:4990".parse().unwrap(), "127.0.0.1:4991".parse().unwrap()];
        bench_network(b, addrs, cfg);
    }

    #[bench]
    fn bench_msg_short_1000_nodelay(b: &mut Bencher) {
        let cfg = BenchConfig {
            tcp_nodelay: true,
            len: 100,
            times: 1000,
        };
        let addrs = ["127.0.0.1:5990".parse().unwrap(), "127.0.0.1:5991".parse().unwrap()];
        bench_network(b, addrs, cfg);
    }

    #[bench]
    fn bench_msg_long_10(b: &mut Bencher) {
        let cfg = BenchConfig {
            tcp_nodelay: false,
            len: 100000,
            times: 10,
        };
        let addrs = ["127.0.0.1:9984".parse().unwrap(), "127.0.0.1:9985".parse().unwrap()];
        bench_network(b, addrs, cfg);
    }

    #[bench]
    fn bench_msg_long_100(b: &mut Bencher) {
        let cfg = BenchConfig {
            tcp_nodelay: false,
            len: 100000,
            times: 100,
        };
        let addrs = ["127.0.0.1:9946".parse().unwrap(), "127.0.0.1:9947".parse().unwrap()];
        bench_network(b, addrs, cfg);
    }

    #[bench]
    fn bench_msg_long_10_nodelay(b: &mut Bencher) {
        let cfg = BenchConfig {
            tcp_nodelay: true,
            len: 100000,
            times: 10,
        };
        let addrs = ["127.0.0.1:9198".parse().unwrap(), "127.0.0.1:9199".parse().unwrap()];
        bench_network(b, addrs, cfg);
    }

    #[bench]
    fn bench_msg_long_100_nodelay(b: &mut Bencher) {
        let cfg = BenchConfig {
            tcp_nodelay: true,
            len: 100000,
            times: 100,
        };
        let addrs = ["127.0.0.1:9198".parse().unwrap(), "127.0.0.1:9199".parse().unwrap()];
        bench_network(b, addrs, cfg);
    }
}
