#![feature(test)]

extern crate test;
extern crate exonum;

#[cfg(all(test, feature = "long_benchmarks"))]
#[cfg(test)]
mod tests {
    use test::Bencher;

    use std::net::SocketAddr;

    use exonum::node::EventsPoolCapacity;
    use exonum::events::network::NetworkConfiguration;
    use exonum::events::tests::{TestEvents, TestHandler, connect_message, raw_message};

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

            let c1 = connect_message(addrs[0]);
            let c2 = connect_message(addrs[1]);

            let t1 = test_events(cfg, addrs[0]).spawn(move |e: &mut TestHandler| {
                e.connect_with(addrs[1]);
                assert_eq!(e.wait_for_connect(), c2);
                for _ in 0..times {
                    let msg = raw_message(0, len);
                    e.send_to(addrs[1], msg);
                    e.wait_for_message();
                }
                e.disconnect_with(addrs[1]);
                assert_eq!(e.wait_for_disconnect(), addrs[1]);
            });
            let t2 = test_events(cfg, addrs[1]).spawn(move |e: &mut TestHandler| {
                assert_eq!(e.wait_for_connect(), c1);
                e.connect_with(addrs[0]);
                for _ in 0..times {
                    let msg = raw_message(1, len);
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
        bench_network(b, addrs, &cfg);
    }

    #[bench]
    fn bench_msg_short_1000(b: &mut Bencher) {
        let cfg = BenchConfig {
            tcp_nodelay: false,
            len: 100,
            times: 1000,
        };
        let addrs = ["127.0.0.1:9792".parse().unwrap(), "127.0.0.1:9793".parse().unwrap()];
        bench_network(b, addrs, &cfg);
    }

    #[bench]
    fn bench_msg_short_100_nodelay(b: &mut Bencher) {
        let cfg = BenchConfig {
            tcp_nodelay: true,
            len: 100,
            times: 100,
        };
        let addrs = ["127.0.0.1:4990".parse().unwrap(), "127.0.0.1:4991".parse().unwrap()];
        bench_network(b, addrs, &cfg);
    }

    #[bench]
    fn bench_msg_short_1000_nodelay(b: &mut Bencher) {
        let cfg = BenchConfig {
            tcp_nodelay: true,
            len: 100,
            times: 1000,
        };
        let addrs = ["127.0.0.1:5990".parse().unwrap(), "127.0.0.1:5991".parse().unwrap()];
        bench_network(b, addrs, &cfg);
    }

    #[bench]
    fn bench_msg_long_10(b: &mut Bencher) {
        let cfg = BenchConfig {
            tcp_nodelay: false,
            len: 100_000,
            times: 10,
        };
        let addrs = ["127.0.0.1:9984".parse().unwrap(), "127.0.0.1:9985".parse().unwrap()];
        bench_network(b, addrs, &cfg);
    }

    #[bench]
    fn bench_msg_long_100(b: &mut Bencher) {
        let cfg = BenchConfig {
            tcp_nodelay: false,
            len: 100_000,
            times: 100,
        };
        let addrs = ["127.0.0.1:9946".parse().unwrap(), "127.0.0.1:9947".parse().unwrap()];
        bench_network(b, addrs, &cfg);
    }

    #[bench]
    fn bench_msg_long_10_nodelay(b: &mut Bencher) {
        let cfg = BenchConfig {
            tcp_nodelay: true,
            len: 100_000,
            times: 10,
        };
        let addrs = ["127.0.0.1:9198".parse().unwrap(), "127.0.0.1:9199".parse().unwrap()];
        bench_network(b, addrs, &cfg);
    }

    #[bench]
    fn bench_msg_long_100_nodelay(b: &mut Bencher) {
        let cfg = BenchConfig {
            tcp_nodelay: true,
            len: 100_000,
            times: 100,
        };
        let addrs = ["127.0.0.1:9198".parse().unwrap(), "127.0.0.1:9199".parse().unwrap()];
        bench_network(b, addrs, &cfg);
    }
}
