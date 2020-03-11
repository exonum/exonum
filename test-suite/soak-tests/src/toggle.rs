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
use futures::{
    channel::oneshot,
    future::{self, Either},
    FutureExt, TryFutureExt,
};
use reqwest::Client;
use structopt::StructOpt;
use tokio::{task::JoinHandle, time::delay_for};

use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};

use crate::services::{MainService, MainServiceInterface, TogglingSupervisor};

mod services;

#[derive(Debug)]
struct RunHandle {
    node_task: JoinHandle<()>,
    service_keys: KeyPair,
    shutdown_handle: ShutdownHandle,
}

impl RunHandle {
    fn new(node: Node, service_keys: KeyPair) -> Self {
        let shutdown_handle = node.shutdown_handle();
        let node_task = node.run().unwrap_or_else(|e| panic!("{}", e));
        Self {
            node_task: tokio::spawn(node_task),
            shutdown_handle,
            service_keys,
        }
    }

    async fn join(self) -> KeyPair {
        self.shutdown_handle
            .shutdown()
            .await
            .expect("Cannot shut down node");
        self.node_task.await.expect("Node panicked during shutdown");
        self.service_keys
    }
}

fn run_nodes(count: u16, start_port: u16) -> (Vec<RunHandle>, Arc<TemporaryDB>) {
    let mut node_handles = Vec::with_capacity(count as usize);
    let inspected_db = Arc::new(TemporaryDB::new());

    let configs = generate_testnet_config(count, start_port);
    for (i, (mut node_cfg, node_keys)) in configs.into_iter().enumerate() {
        if i == 0 {
            let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 8_080);
            node_cfg.api.public_api_address = Some(addr);
        }

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

        node_handles.push(RunHandle::new(node, service_keys));
    }
    (node_handles, inspected_db)
}

fn get_height(db: &TemporaryDB) -> Height {
    db.snapshot().for_core().height()
}

#[derive(Debug)]
struct ApiStats {
    ok_answers: usize,
    erroneous_answers: usize,
}

/// Periodically probes the node HTTP API, recording the number of times the response was
/// successful and erroneous. `cancel_rx` is used to signal probe termination.
async fn probe_api(url: &str, mut cancel_rx: oneshot::Receiver<()>) -> ApiStats {
    const API_TIMEOUT: Duration = Duration::from_millis(50);

    async fn send_request(client: &Client, url: &str) -> Result<(), failure::Error> {
        let response: String = client
            .get(url)
            .timeout(API_TIMEOUT)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        assert_eq!(response, "pong");
        Ok(())
    }

    let mut stats = ApiStats {
        ok_answers: 0,
        erroneous_answers: 0,
    };

    loop {
        let selected = future::select(&mut cancel_rx, delay_for(API_TIMEOUT)).await;
        if let Either::Left(_) = selected {
            // We've received the cancellation signal; we're done.
            break;
        }

        // NB: reusing `Client` among requests makes it cache responses, which is the last thing
        // we want in this test.
        if let Err(e) = send_request(&Client::new(), url).await {
            log::info!("Call to node API resulted in an error: {}", e);
            stats.erroneous_answers += 1;
        } else {
            log::trace!("Successfully called node API");
            stats.ok_answers += 1;
        }
    }
    stats
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

#[tokio::main]
async fn main() {
    exonum::crypto::init();
    exonum::helpers::init_logger().ok();

    let args = Args::from_args();
    println!("Running test with {:?}", args);

    let (supervisor_api_tx, supervisor_api_rx) = oneshot::channel();
    let supervisor_stats_task = probe_api(
        "http://127.0.0.1:8080/api/services/supervisor/ping",
        supervisor_api_rx,
    );
    let (supervisor_stats_task, supervisor_stats) = supervisor_stats_task.remote_handle();
    tokio::spawn(supervisor_stats_task);

    let (main_api_tx, main_api_rx) = oneshot::channel();
    let main_stats_task = probe_api("http://127.0.0.1:8080/api/services/main/ping", main_api_rx);
    let (main_stats_task, main_stats) = main_stats_task.remote_handle();
    tokio::spawn(main_stats_task);

    let (nodes, db) = run_nodes(args.node_count, 2_000);
    loop {
        let height = get_height(&db);
        println!("Blockchain height: {:?}", height);
        if args.max_height.map_or(false, |max| height >= Height(max)) {
            break;
        }
        delay_for(Duration::from_secs(1)).await;
    }

    let snapshot = db.snapshot();
    let core_schema = snapshot.for_core();
    let transactions = core_schema.transactions();
    let transactions_pool = core_schema.transactions_pool();
    let height = core_schema.height();

    let keys = future::join_all(nodes.into_iter().map(RunHandle::join)).await;
    let mut committed_timestamps = 0;
    for (node_i, keys) in keys.into_iter().enumerate() {
        let timestamps = (1..=height.0)
            .filter_map(|i| match i % 5 {
                0 | 4 => Some(Height(i)),
                _ => None,
            })
            .map(|i| (i, keys.timestamp(MainService::INSTANCE_ID, i)));

        for (i, timestamp) in timestamps {
            let tx_hash = timestamp.object_hash();
            if transactions.contains(&tx_hash) && !transactions_pool.contains(&tx_hash) {
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

    drop(supervisor_api_tx);
    drop(main_api_tx);
    println!("Supervisor availability: {:#?}", supervisor_stats.await);
    println!("Main service availability: {:#?}", main_stats.await);
}
