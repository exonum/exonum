// #![feature(test)]

// extern crate test;
// extern crate exonum;
// extern crate time;
// extern crate env_logger;

// #[cfg(test)]
// mod tests {
//     use std::{time, thread};
//     use std::net::SocketAddr;
//     use std::collections::VecDeque;

//     use test::Bencher;
//     use time::{get_time, Duration};

//     use exonum::events::{Events, Reactor, Event, InternalEvent, Channel};
//     use exonum::events::{Network, NetworkConfiguration, EventHandler};
//     use exonum::messages::{MessageWriter, RawMessage};
//     use exonum::crypto::gen_keypair;

//     pub type TestEvent = InternalEvent<(), u32>;

//     pub struct TestHandler {
//         events: VecDeque<TestEvent>,
//     }

//     pub trait TestPoller {
//         fn poll(&mut self) -> Option<TestEvent>;
//     }

//     impl TestHandler {
//         pub fn new() -> TestHandler {
//             TestHandler { events: VecDeque::new() }
//         }
//     }

//     impl TestPoller for TestHandler {
//         fn poll(&mut self) -> Option<TestEvent> {
//             self.events.pop_front()
//         }
//     }

//     impl EventHandler for TestHandler {
//         type Timeout = ();
//         type ApplicationEvent = ();

//         fn handle_event(&mut self, event: Event) {
//             self.events.push_back(InternalEvent::Node(event));
//         }
//         fn handle_timeout(&mut self, _: Self::Timeout) {}
//         fn handle_application_event(&mut self, event: Self::ApplicationEvent) {
//             self.events.push_back(InternalEvent::Application(event));
//         }
//     }

//     pub struct TestEvents(Events<TestHandler>);

//     impl TestEvents {
//         pub fn with_addr(addr: SocketAddr) -> TestEvents {
//             let network = Network::with_config(NetworkConfiguration {
//                 listen_address: addr,
//                 max_incoming_connections: 128,
//                 max_outgoing_connections: 128,
//                 tcp_nodelay: true,
//                 tcp_keep_alive: None,
//                 tcp_reconnect_timeout: 1000,
//                 tcp_reconnect_timeout_max: 600000,
//             });
//             let handler = TestHandler::new();

//             TestEvents(Events::new(network, handler).unwrap())
//         }

//         pub fn wait_for_bind(&mut self, addr: &SocketAddr) {
//             self.0.bind().unwrap();
//             thread::sleep(time::Duration::from_millis(1000));

//             self.0.channel().connect(addr);

//             let start = get_time();
//             loop {
//                 self.0.run_once(Some(100)).unwrap();

//                 if start + Duration::milliseconds(100) < get_time() {
//                     return;
//                 }
//                 while let Some(e) = self.0.inner.handler.poll() {
//                     match e {
//                         InternalEvent::Node(Event::Connected(_)) => return,
//                         _ => {}
//                     }
//                 }
//             }
//         }

//         pub fn wait_for_msg(&mut self, duration: Duration) -> Option<RawMessage> {
//             let start = get_time();
//             loop {
//                 self.0.run_once(Some(100)).unwrap();

//                 if start + duration < get_time() {
//                     return None;
//                 }
//                 while let Some(e) = self.0.inner.handler.poll() {
//                     match e {
//                         InternalEvent::Node(Event::Incoming(msg)) => return Some(msg),
//                         InternalEvent::Node(Event::Error(_)) => return None,
//                         _ => {}
//                     }
//                 }
//             }
//         }

//         pub fn process_events(&mut self, duration: Duration) {
//             let start = get_time();
//             loop {
//                 self.0.run_once(Some(100)).unwrap();

//                 if start + duration < get_time() {
//                     return;
//                 }
//             }
//         }

//         pub fn send_to(&mut self, addr: &SocketAddr, msg: RawMessage) {
//             self.0.channel().send_to(addr, msg);
//             self.0.run_once(None).unwrap();
//         }
//     }

//     pub fn gen_message(id: u16, len: usize) -> RawMessage {
//         let writer = MessageWriter::new(id, len);
//         RawMessage::new(writer.sign(&gen_keypair().1))
//     }

//     struct BenchConfig {
//         times: usize,
//         len: usize,
//         tcp_nodelay: bool
//     }

//     fn bench_network(b: &mut Bencher, addrs: [SocketAddr; 2], cfg: BenchConfig) {
//         b.iter(|| {
//             let mut e1 = TestEvents::with_addr(addrs[0], &cfg);
//             let mut e2 = TestEvents::with_addr(addrs[1], &cfg);
//             e1.bind().unwrap();
//             e2.bind().unwrap();

//             let timeout = Duration::seconds(30);
//             let len = cfg.len;
//             let times = cfg.times;
//             let t1 = thread::spawn(move || {
//                 e1.wait_for_connect(&addrs[1]);
//                 for _ in 0..times {
//                     let msg = Events::<u32>::gen_message(0, len);
//                     e1.send_to(&addrs[1], msg);
//                     e1.wait_for_messages(1, timeout).unwrap();
//                 }
//                 e1.process_events(Duration::milliseconds(0));
//                 drop(e1);
//             });
//             let t2 = thread::spawn(move || {
//                 e2.wait_for_connect(&addrs[0]);
//                 for _ in 0..times {
//                     let msg = Events::<u32>::gen_message(1, len);
//                     e2.send_to(&addrs[0], msg);
//                     e2.wait_for_messages(1, timeout).unwrap();
//                 }
//                 e2.process_events(Duration::milliseconds(0));
//                 drop(e2);
//             });
//             t1.join().unwrap();
//             t2.join().unwrap();
//         })
//     }

//     #[cfg(feature = "long_benchmarks")]
//     #[bench]
//     fn bench_msg_short_100(b: &mut Bencher) {
//         let cfg = BenchConfig {
//             tcp_nodelay: false,
//             len: 100,
//             times: 100
//         };
//         let addrs = ["127.0.0.1:9990".parse().unwrap(), "127.0.0.1:9991".parse().unwrap()];
//         bench_network(b, addrs, cfg);
//     }

//     #[cfg(feature = "long_benchmarks")]
//     #[bench]
//     fn bench_msg_short_1000(b: &mut Bencher) {
//         let cfg = BenchConfig {
//             tcp_nodelay: false,
//             len: 100,
//             times: 1000
//         };
//         let addrs = ["127.0.0.1:9992".parse().unwrap(), "127.0.0.1:9993".parse().unwrap()];
//         bench_network(b, addrs, cfg);
//     }

//     #[cfg(feature = "long_benchmarks")]
//     #[bench]
//     fn bench_msg_short_10000(b: &mut Bencher) {
//         let cfg = BenchConfig {
//             tcp_nodelay: false,
//             len: 100,
//             times: 10000
//         };
//         let addrs = ["127.0.0.1:9992".parse().unwrap(), "127.0.0.1:9993".parse().unwrap()];
//         bench_network(b, addrs, cfg);
//     }

//     #[cfg(feature = "long_benchmarks")]
//     #[bench]
//     fn bench_msg_short_100_nodelay(b: &mut Bencher) {
//         let cfg = BenchConfig {
//             tcp_nodelay: true,
//             len: 100,
//             times: 100
//         };
//         let addrs = ["127.0.0.1:9990".parse().unwrap(), "127.0.0.1:9991".parse().unwrap()];
//         bench_network(b, addrs, cfg);
//     }

//     #[cfg(feature = "long_benchmarks")]
//     #[bench]
//     fn bench_msg_short_10000_nodelay(b: &mut Bencher) {
//         let cfg = BenchConfig {
//             tcp_nodelay: true,
//             len: 100,
//             times: 10000
//         };
//         let addrs = ["127.0.0.1:9990".parse().unwrap(), "127.0.0.1:9991".parse().unwrap()];
//         bench_network(b, addrs, cfg);
//     }

//     #[cfg(feature = "long_benchmarks")]
//     #[bench]
//     fn bench_msg_long_10(b: &mut Bencher) {
//         let cfg = BenchConfig {
//             tcp_nodelay: false,
//             len: 100000,
//             times: 10
//         };
//         let addrs = ["127.0.0.1:9994".parse().unwrap(), "127.0.0.1:9995".parse().unwrap()];
//         bench_network(b, addrs, cfg);
//     }

//     #[cfg(feature = "long_benchmarks")]
//     #[bench]
//     fn bench_msg_long_100(b: &mut Bencher) {
//         let cfg = BenchConfig {
//             tcp_nodelay: false,
//             len: 100000,
//             times: 100
//         };
//         let addrs = ["127.0.0.1:9996".parse().unwrap(), "127.0.0.1:9997".parse().unwrap()];
//         bench_network(b, addrs, cfg);
//     }

//     #[cfg(feature = "long_benchmarks")]
//     #[bench]
//     fn bench_msg_long_1000(b: &mut Bencher) {
//         let cfg = BenchConfig {
//             tcp_nodelay: false,
//             len: 100000,
//             times: 1000
//         };
//         let addrs = ["127.0.0.1:9998".parse().unwrap(), "127.0.0.1:9999".parse().unwrap()];
//         bench_network(b, addrs, cfg);
//     }

//     #[cfg(feature = "long_benchmarks")]
//     #[bench]
//     fn bench_msg_long_1000_nodelay(b: &mut Bencher) {
//         let cfg = BenchConfig {
//             tcp_nodelay: true,
//             len: 100000,
//             times: 1000
//         };
//         let addrs = ["127.0.0.1:9998".parse().unwrap(), "127.0.0.1:9999".parse().unwrap()];
//         bench_network(b, addrs, cfg);
//     }
// }