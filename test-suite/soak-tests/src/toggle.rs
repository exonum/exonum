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

//! Runs a network with a service that is frequently switched on / off and that generates
//! transactions in `after_commit` hook.

use exonum::{
    blockchain::config::GenesisConfigBuilder,
    crypto::KeyPair,
    helpers::Height,
    merkledb::{Database, ObjectHash, TemporaryDB},
    runtime::SnapshotExt,
};
use exonum_node::{generate_testnet_config, Node, NodeBuilder, ShutdownHandle};
use exonum_rust_runtime::{DefaultInstance, RustRuntime, ServiceFactory};
use futures::Future;
use structopt::StructOpt;

use std::{sync::Arc, thread, time::Duration};

use crate::services::{MainService, MainServiceInterface, TogglingSupervisor};

mod services;

#[derive(Debug)]
struct RunHandle {
    node_thread: thread::JoinHandle<()>,
    service_keys: KeyPair,
    shutdown_handle: ShutdownHandle,
}

impl RunHandle {
    fn new(node: Node, service_keys: KeyPair) -> Self {
        let shutdown_handle = node.shutdown_handle();
        Self {
            node_thread: thread::spawn(|| node.run().unwrap()),
            shutdown_handle,
            service_keys,
        }
    }

    fn join(self) -> KeyPair {
        self.shutdown_handle
            .shutdown()
            .wait()
            .expect("Cannot shut down node");
        self.node_thread
            .join()
            .expect("Node panicked during shutdown");
        self.service_keys
    }
}

fn run_nodes(count: u16, start_port: u16) -> (Vec<RunHandle>, Arc<TemporaryDB>) {
    let mut node_threads = Vec::with_capacity(count as usize);
    let inspected_db = Arc::new(TemporaryDB::new());

    let configs = generate_testnet_config(count, start_port);
    for (i, (node_cfg, node_keys)) in configs.into_iter().enumerate() {
        let genesis_cfg = GenesisConfigBuilder::with_consensus_config(node_cfg.consensus.clone())
            .with_artifact(MainService.artifact_id())
            .with_instance(MainService.default_instance())
            .with_artifact(TogglingSupervisor.artifact_id())
            .with_instance(TogglingSupervisor.default_instance())
            .build();

        let db = if i == 0 {
            Arc::clone(&inspected_db)
        } else {
            Arc::new(TemporaryDB::new())
        };

        let service_keys = node_keys.service.clone();
        let node = NodeBuilder::new(db as Arc<dyn Database>, node_cfg, node_keys)
            .with_genesis_config(genesis_cfg)
            .with_runtime_fn(|channel| {
                RustRuntime::builder()
                    .with_factory(MainService)
                    .with_factory(TogglingSupervisor)
                    .build(channel.endpoints_sender())
            })
            .build();

        node_threads.push(RunHandle::new(node, service_keys));
    }
    (node_threads, inspected_db)
}

fn get_height(db: &TemporaryDB) -> Height {
    db.snapshot().for_core().height()
}

/// Runs a network with a service that is frequently switched on / off and that generates
/// transactions in `after_commit` hook.
#[derive(Debug, StructOpt)]
#[structopt(name = "toggle", set_term_width = 80)]
struct Args {
    /// Number of nodes in the network.
    #[structopt(name = "nodes", default_value = "4")]
    node_count: u16,

    /// Blockchain height to reach. If not specified, the test will run infinitely.
    #[structopt(name = "max-height", long, short = "H")]
    max_height: Option<u64>,
}

fn main() {
    exonum::crypto::init();
    exonum::helpers::init_logger().ok();

    let args = Args::from_args();
    println!("Running test with {:?}", args);

    let (nodes, db) = run_nodes(args.node_count, 2_000);
    loop {
        let height = get_height(&db);
        println!("Blockchain height: {:?}", height);
        if args.max_height.map_or(false, |max| height >= Height(max)) {
            break;
        }
        thread::sleep(Duration::from_secs(1));
    }

    let snapshot = db.snapshot();
    let core_schema = snapshot.for_core();
    let transactions = core_schema.transactions();
    let height = core_schema.height();

    let keys: Vec<_> = nodes.into_iter().map(RunHandle::join).collect();
    let mut committed_timestamps = 0;
    for (node_i, keys) in keys.into_iter().enumerate() {
        let timestamps = (1..=height.0)
            .filter_map(|i| match i % 5 {
                0 | 4 => Some(Height(i)),
                _ => None,
            })
            .map(|i| (i, keys.timestamp(MainService::INSTANCE_ID, i)));

        for (i, timestamp) in timestamps {
            if transactions.contains(&timestamp.object_hash()) {
                committed_timestamps += 1;
            } else {
                println!(
                    "Did not commit transaction for node {} at height {:?}",
                    node_i, i
                );
            }
        }
    }

    let committed_txs = core_schema.transactions_len();
    println!("Total committed transactions: {}", committed_txs);
    assert_eq!(
        committed_timestamps, committed_txs,
        "There are unknown transactions on the blockchain"
    );
}
