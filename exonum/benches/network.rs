#![feature(test)]

extern crate test;
extern crate exonum;
extern crate time;

use std::{thread};
use std::net::SocketAddr;

use time::Duration;
use test::Bencher;

use exonum::events::{Events, Reactor, EventsConfiguration, Event, Timeout};
use exonum::events::{Network, NetworkConfiguration};
use exonum::messages::{MessageWriter, RawMessage};
use exonum::crypto::gen_keypair;


trait EventsBench {
    fn with_addr(addr: SocketAddr) -> Events;
    fn wait_for_msg(&mut self) -> Option<RawMessage>;
    fn gen_message(id: u16, len: usize) -> RawMessage;
    fn wait_for_messages(&mut self, mut count: usize, timeout: Duration) -> Result<(), String>;
}

impl EventsBench for Events {
    fn with_addr(addr: SocketAddr) -> Events {
        let network = Network::with_config(NetworkConfiguration {
            listen_address: addr,
            max_incoming_connections: 128,
            max_outgoing_connections: 128
        });
        Events::with_config(EventsConfiguration::new(), network).unwrap()
    }

    fn wait_for_msg(&mut self) -> Option<RawMessage> {
        let time = self.get_time() + Duration::milliseconds(10000);
        self.add_timeout(Timeout::Status, time);
        loop {
            match self.poll() {
                Event::Incoming(msg) => return Some(msg),
                Event::Timeout(_) => return None,
                Event::Error(_) => return None,
                _ => {}
            }
        }
    }

    fn wait_for_messages(&mut self, mut count: usize, timeout: Duration) -> Result<(), String> {
        let time = self.get_time() + timeout;
        self.add_timeout(Timeout::Status, time);
        loop {
            match self.poll() {
                Event::Incoming(_) => { count = count - 1; }
                Event::Timeout(_) => return Err(format!("Timeout exceeded, {} messages is not received", count)),
                Event::Error(_) => return Err(format!("An error occured, {} messages is not received", count)),
                _ => {}
            }
            if count == 0 {
                return Ok(())
            }
        }
    }

    fn gen_message(id: u16, len: usize) -> RawMessage {
        let writer = MessageWriter::new(id, len);
        RawMessage::new(writer.sign(&gen_keypair().1))
    }
}

fn bench_network(b: &mut Bencher, addrs: [SocketAddr; 2], times: usize, len: usize) {
    b.iter(|| {
        let mut e1 = Events::with_addr(addrs[0]);
        let mut e2 = Events::with_addr(addrs[1]);
        e1.bind().unwrap();
        e2.bind().unwrap();

        let timeout = Duration::seconds(120);
        let t1 = thread::spawn(move || {
                for _ in 0..times {
                    let msg = Events::gen_message(0, len);
                    e1.send_to(&addrs[1], msg).unwrap();
                }
                e1.wait_for_messages(times, timeout).unwrap();
        });
        let t2 = thread::spawn(move || {
                for _ in 0..times {
                    let msg = Events::gen_message(1, len);
                    e2.send_to(&addrs[0], msg).unwrap();
                }
                e2.wait_for_messages(times, timeout).unwrap();
        });
        t1.join().unwrap();
        t2.join().unwrap();
    })
}

#[bench]
fn bench_msg_short_100(b: &mut Bencher) {
    let addrs = [
        "127.0.0.1:9990".parse().unwrap(),
        "127.0.0.1:9991".parse().unwrap()
    ];
    bench_network(b, addrs, 100, 100);
}

#[bench]
fn bench_msg_short_1000(b: &mut Bencher) {
    let addrs = [
        "127.0.0.1:9992".parse().unwrap(),
        "127.0.0.1:9993".parse().unwrap()
    ];
    bench_network(b, addrs, 1000, 100);
}

#[bench]
fn bench_msg_long_10(b: &mut Bencher) {
    let addrs = [
        "127.0.0.1:9994".parse().unwrap(),
        "127.0.0.1:9995".parse().unwrap()
    ];
    bench_network(b, addrs, 10, 100000);
}
