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

use test::Bencher;

use std::{net::SocketAddr, thread};

use events::{
    network::NetworkConfiguration, tests::{connect_message, raw_message, TestEvents},
};
use node::EventsPoolCapacity;

struct BenchConfig {
    times: usize,
    len: usize,
    tcp_nodelay: bool,
}

fn test_events(cfg: &BenchConfig, listen_address: SocketAddr) -> TestEvents {
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

fn bench_network(b: &mut Bencher, addrs: [SocketAddr; 2], cfg: &BenchConfig) {
    b.iter(|| {
        let times = cfg.times;
        let len = cfg.len;
        let first = addrs[0];
        let second = addrs[1];

        let c1 = connect_message(first);
        let c2 = connect_message(second);

        let mut t1 = test_events(cfg, first).spawn();
        let mut t2 = test_events(cfg, second).spawn();

        t1.connect_with(second);
        t2.connect_with(first);
        assert_eq!(t1.wait_for_connect(), c2);
        assert_eq!(t2.wait_for_connect(), c1);

        let t1 = thread::spawn(move || {
            for _ in 0..times {
                let msg = raw_message(0, len);
                t1.send_to(second, msg);
                t1.wait_for_message();
            }
            t1
        });

        let t2 = thread::spawn(move || {
            for _ in 0..times {
                let msg = raw_message(1, len);
                t2.send_to(first, msg);
                t2.wait_for_message();
            }
            t2
        });

        let mut t1 = t1.join().unwrap();
        let mut t2 = t2.join().unwrap();

        t1.disconnect_with(second);
        t2.disconnect_with(first);

        assert_eq!(t1.wait_for_disconnect(), second);
        assert_eq!(t2.wait_for_disconnect(), first);

        drop(t1);
        drop(t2);
    })
}

#[bench]
fn bench_msg_short_100(b: &mut Bencher) {
    let cfg = BenchConfig {
        tcp_nodelay: false,
        len: 100,
        times: 100,
    };
    let addrs = [
        "127.0.0.1:6990".parse().unwrap(),
        "127.0.0.1:6991".parse().unwrap(),
    ];
    bench_network(b, addrs, &cfg);
}

#[bench]
fn bench_msg_short_1000(b: &mut Bencher) {
    let cfg = BenchConfig {
        tcp_nodelay: false,
        len: 1000,
        times: 1000,
    };
    let addrs = [
        "127.0.0.1:9792".parse().unwrap(),
        "127.0.0.1:9793".parse().unwrap(),
    ];
    bench_network(b, addrs, &cfg);
}

#[bench]
fn bench_msg_short_10_000(b: &mut Bencher) {
    let cfg = BenchConfig {
        tcp_nodelay: false,
        len: 1000,
        times: 10_000,
    };
    let addrs = [
        "127.0.0.1:9792".parse().unwrap(),
        "127.0.0.1:9793".parse().unwrap(),
    ];
    bench_network(b, addrs, &cfg);
}

#[bench]
fn bench_msg_short_100_nodelay(b: &mut Bencher) {
    let cfg = BenchConfig {
        tcp_nodelay: true,
        len: 100,
        times: 100,
    };
    let addrs = [
        "127.0.0.1:4990".parse().unwrap(),
        "127.0.0.1:4991".parse().unwrap(),
    ];
    bench_network(b, addrs, &cfg);
}

#[bench]
fn bench_msg_short_1000_nodelay(b: &mut Bencher) {
    let cfg = BenchConfig {
        tcp_nodelay: true,
        len: 100,
        times: 1000,
    };
    let addrs = [
        "127.0.0.1:5990".parse().unwrap(),
        "127.0.0.1:5991".parse().unwrap(),
    ];
    bench_network(b, addrs, &cfg);
}

#[bench]
fn bench_msg_short_10_000_nodelay(b: &mut Bencher) {
    let cfg = BenchConfig {
        tcp_nodelay: true,
        len: 100,
        times: 10_000,
    };
    let addrs = [
        "127.0.0.1:5990".parse().unwrap(),
        "127.0.0.1:5991".parse().unwrap(),
    ];
    bench_network(b, addrs, &cfg);
}

#[bench]
fn bench_msg_long_10(b: &mut Bencher) {
    let cfg = BenchConfig {
        tcp_nodelay: false,
        len: 100_000,
        times: 10,
    };
    let addrs = [
        "127.0.0.1:9984".parse().unwrap(),
        "127.0.0.1:9985".parse().unwrap(),
    ];
    bench_network(b, addrs, &cfg);
}

#[bench]
fn bench_msg_long_100(b: &mut Bencher) {
    let cfg = BenchConfig {
        tcp_nodelay: false,
        len: 100_000,
        times: 100,
    };
    let addrs = [
        "127.0.0.1:9946".parse().unwrap(),
        "127.0.0.1:9947".parse().unwrap(),
    ];
    bench_network(b, addrs, &cfg);
}

#[bench]
fn bench_msg_long_10_nodelay(b: &mut Bencher) {
    let cfg = BenchConfig {
        tcp_nodelay: true,
        len: 100_000,
        times: 10,
    };
    let addrs = [
        "127.0.0.1:9198".parse().unwrap(),
        "127.0.0.1:9199".parse().unwrap(),
    ];
    bench_network(b, addrs, &cfg);
}

#[bench]
fn bench_msg_long_100_nodelay(b: &mut Bencher) {
    let cfg = BenchConfig {
        tcp_nodelay: true,
        len: 100_000,
        times: 100,
    };
    let addrs = [
        "127.0.0.1:9198".parse().unwrap(),
        "127.0.0.1:9199".parse().unwrap(),
    ];
    bench_network(b, addrs, &cfg);
}
