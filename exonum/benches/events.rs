#![feature(test)]

extern crate exonum;
extern crate test;

#[cfg(all(test, feature = "long_benchmarks"))]
mod tests {
    use test::Bencher;
    use std::thread;
    use std::net::SocketAddr;
    use std::time::Duration;

    use exonum::events::Reactor;
    use exonum::events::tests::{gen_message, TestEvents, BenchConfig};

    fn bench_network(b: &mut Bencher, addrs: [SocketAddr; 2], cfg: &BenchConfig) {
        b.iter(|| {
            let mut e1 = TestEvents::with_cfg(cfg, addrs[0]);
            let mut e2 = TestEvents::with_cfg(cfg, addrs[1]);
            e1.0.bind().unwrap();
            e2.0.bind().unwrap();

            let timeout = Duration::from_secs(30);
            let len = cfg.len;
            let times = cfg.times;
            let t1 = thread::spawn(move || {
                e1.wait_for_connect(&addrs[1]).unwrap();
                for _ in 0..times {
                    let msg = gen_message(0, len);
                    e1.send_to(&addrs[1], msg);
                    e1.wait_for_messages(1, timeout).unwrap();
                }
                e1.wait_for_disconnect(Duration::from_millis(1000)).unwrap();
            });
            let t2 = thread::spawn(move || {
                e2.wait_for_connect(&addrs[0]).unwrap();
                for _ in 0..times {
                    let msg = gen_message(1, len);
                    e2.send_to(&addrs[0], msg);
                    e2.wait_for_messages(1, timeout).unwrap();
                }
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
    fn bench_msg_short_10000(b: &mut Bencher) {
        let cfg = BenchConfig {
            tcp_nodelay: false,
            len: 100,
            times: 10_000,
        };
        let addrs = ["127.0.0.1:9982".parse().unwrap(), "127.0.0.1:9983".parse().unwrap()];
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
    fn bench_msg_short_10000_nodelay(b: &mut Bencher) {
        let cfg = BenchConfig {
            tcp_nodelay: true,
            len: 100,
            times: 10_000,
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
