// Copyright 2020 The Exonum Team
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

use anyhow::format_err;
use exonum::{crypto::KeyPair, helpers::Height, merkledb::ObjectHash, runtime::SnapshotExt};
use exonum_node::FlushPoolStrategy;
use exonum_rust_runtime::{
    spec::{Deploy, Spec},
    DefaultInstance,
};
use futures::Future;
use structopt::StructOpt;

use std::{
    collections::HashMap,
    fmt, thread,
    time::{Duration, Instant},
};

use exonum_soak_tests::{
    run_nodes,
    services::{MainConfig, MainService, MainServiceInterface},
};

/// Runs a network with a service and sends transactions to it, measuring how fast
/// transactions appear in the mempool and are confirmed.
#[derive(Debug, StructOpt)]
#[structopt(name = "toggle", set_term_width = 80)]
struct Args {
    /// Number of nodes in the network.
    #[structopt(name = "nodes", default_value = "4")]
    node_count: u16,

    /// Blockchain height to reach. If not specified, the test will run infinitely.
    #[structopt(name = "tx-count", long, short = "T")]
    tx_count: Option<u64>,

    /// Pool flushing strategy serialized in a TOML-like format (e.g., `never`, `immediate`
    /// or `timeout=20`).
    #[structopt(name = "flush", long, short = "f", parse(try_from_str = parse_strategy))]
    flush_strategy: Option<FlushPoolStrategy>,

    /// Intensity of the test, in transactions per second. Sensible values are up to several
    /// hundred tps.
    #[structopt(name = "tps", long, short = "t", default_value = "10")]
    tps: usize,
}

fn parse_strategy(s: &str) -> anyhow::Result<FlushPoolStrategy> {
    match s.trim() {
        "never" => Ok(FlushPoolStrategy::Never),
        "immediate" => Ok(FlushPoolStrategy::Immediate),
        s if s.starts_with("timeout=") => {
            // 8 is the length of "timeout=".
            let timeout: u64 = s[8..].parse()?;
            Ok(FlushPoolStrategy::Timeout { timeout })
        }
        _ => Err(format_err!("Invalid pool flushing strategy")),
    }
}

#[derive(Default)]
struct TimingStats {
    total_duration: Duration,
    max_duration: Duration,
    samples: usize,
}

impl fmt::Display for TimingStats {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.samples == 0 {
            formatter.write_str("(no samples)")
        } else {
            let avg_duration = self.total_duration.mul_f64(1.0 / (self.samples as f64));
            write!(
                formatter,
                "avg: {} ms, max: {} ms",
                avg_duration.as_millis(),
                self.max_duration.as_millis()
            )
        }
    }
}

impl TimingStats {
    fn push(&mut self, dur: Duration) {
        if self.max_duration < dur {
            self.max_duration = dur;
        }
        self.total_duration += dur;
        self.samples += 1;
    }
}

// FIXME: refactor using the `tokio` runtime once #1804 is merged. (ECR-4320)
fn main() {
    exonum::crypto::init();
    exonum::helpers::init_logger().ok();

    let args = Args::from_args();
    println!("Running test with {:?}", args);

    let config = MainConfig {
        generate_tx_in_after_commit: false,
    };
    let main_service = Spec::new(MainService).with_instance(
        MainService::INSTANCE_ID,
        MainService::INSTANCE_NAME,
        config,
    );

    let flush_strategy = args.flush_strategy.unwrap_or_default();
    let nodes = run_nodes(
        args.node_count,
        2_000,
        |node_cfg| {
            node_cfg.mempool.flush_pool_strategy = flush_strategy.clone();
        },
        |genesis, rt| main_service.clone().deploy(genesis, rt),
    );

    let keys = KeyPair::random();
    let delay = Duration::from_secs(1).mul_f64(1.0 / args.tps as f64);

    let mut txs_not_in_pool = HashMap::new();
    let mut times_to_pool = TimingStats::default();
    let mut txs_in_pool = HashMap::new();
    let mut times_to_commit = TimingStats::default();
    let mut prev_report_time = Instant::now();

    loop {
        let height = nodes[0].blockchain().last_block().height;
        if height > Height(0) {
            break;
        }
        thread::sleep(Duration::from_millis(200));
    }
    log::info!("Started sending transactions");

    for i in 0..args.tx_count.unwrap_or_else(u64::max_value) {
        let tx = keys.timestamp(MainService::INSTANCE_ID, Height(i));
        let tx_hash = tx.object_hash();
        let now = Instant::now();
        txs_not_in_pool.insert(tx_hash, now);

        nodes[0]
            .blockchain()
            .sender()
            .broadcast_transaction(tx)
            .wait()
            .unwrap();

        let snapshot = nodes.last().unwrap().blockchain().snapshot();
        let snapshot = snapshot.for_core();
        let tx_pool = snapshot.transactions_pool();
        let tx_locations = snapshot.transactions_locations();

        txs_not_in_pool.retain(|tx_hash, start| {
            if tx_locations.contains(tx_hash) {
                log::trace!("Transaction {} is immediately committed", tx_hash);
                times_to_pool.push(now - *start);
                times_to_commit.push(now - *start);
                false
            } else if tx_pool.contains(tx_hash) {
                log::trace!("Transaction {} appeared in pool", tx_hash);
                txs_in_pool.insert(*tx_hash, *start);
                times_to_pool.push(now - *start);
                false
            } else {
                true
            }
        });

        txs_in_pool.retain(|tx_hash, start| {
            if tx_locations.contains(tx_hash) {
                log::trace!("Transaction {} is committed", tx_hash);
                times_to_commit.push(now - *start);
                false
            } else {
                true
            }
        });

        if now - prev_report_time >= Duration::from_secs(1) {
            prev_report_time = now;
            println!(
                "Transactions: {} total, {} in pool, {} committed",
                i + 1,
                txs_in_pool.len(),
                times_to_commit.samples
            );
            println!("Time to pool: {}", times_to_pool);
            println!("Time to commit: {}", times_to_commit);
        }

        thread::sleep(delay);
    }

    for node in nodes {
        node.join();
    }

    println!("\nOverall results:");
    println!("Time to pool: {}", times_to_pool);
    println!("Time to commit: {}", times_to_commit);
}
