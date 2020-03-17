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

// cspell:ignore noprobe

use exonum::{helpers::Height, merkledb::ObjectHash, runtime::SnapshotExt};
use exonum_rust_runtime::{
    spec::{Deploy, Spec},
    DefaultInstance,
};
use futures::{
    channel::oneshot,
    future::{self, Either, RemoteHandle},
    FutureExt,
};
use reqwest::Client;
use structopt::StructOpt;
use tokio::time::delay_for;

use std::{fmt, time::Duration};

use exonum_soak_tests::{
    services::{MainConfig, MainService, MainServiceInterface, TogglingSupervisor},
    NetworkBuilder, RunHandle,
};

struct ApiStats {
    ok_answers: usize,
    erroneous_answers: usize,
}

impl fmt::Debug for ApiStats {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug_struct = formatter.debug_struct("ApiStats");
        debug_struct
            .field("ok_answers", &self.ok_answers)
            .field("erroneous_answers", &self.erroneous_answers);

        let all_answers = self.ok_answers + self.erroneous_answers;
        if all_answers > 0 {
            let ok_fraction = self.ok_answers as f64 / all_answers as f64;
            debug_struct.field("ok_fraction", &ok_fraction);
        }
        debug_struct.finish()
    }
}

/// Periodically probes the node HTTP API, recording the number of times the response was
/// successful and erroneous. `cancel_rx` is used to signal probe termination.
async fn probe_api(url: &str, mut cancel_rx: oneshot::Receiver<()>) -> ApiStats {
    const API_TIMEOUT: Duration = Duration::from_millis(50);

    async fn send_request(client: &Client, url: &str) -> anyhow::Result<()> {
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

#[derive(Debug)]
struct ApiProbe {
    cancel_tx: oneshot::Sender<()>,
    stats: RemoteHandle<ApiStats>,
}

impl ApiProbe {
    fn new(url: &'static str) -> Self {
        let (cancel_tx, cancel_rx) = oneshot::channel();
        let (stats_task, stats) = probe_api(url, cancel_rx).remote_handle();
        tokio::spawn(stats_task);
        Self { cancel_tx, stats }
    }

    async fn get(self) -> ApiStats {
        drop(self.cancel_tx);
        self.stats.await
    }
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

    /// Disable HTTP API probing.
    #[structopt(name = "noprobe", long)]
    no_probe: bool,
}

#[tokio::main]
async fn main() {
    exonum::crypto::init();
    exonum::helpers::init_logger().ok();

    let args = Args::from_args();
    println!("Running test with {:?}", args);

    let config = MainConfig {
        generate_tx_in_after_commit: true,
    };
    let main_service = Spec::new(MainService).with_instance(
        MainService::INSTANCE_ID,
        MainService::INSTANCE_NAME,
        config,
    );
    let supervisor = Spec::new(TogglingSupervisor).with_default_instance();

    let mut set_api = false;
    let nodes = NetworkBuilder::new(args.node_count, 2_000)
        .modify_config(|node_cfg| {
            // Enable public HTTP server for a single node.
            if !set_api {
                node_cfg.api.public_api_address = Some("127.0.0.1:8080".parse().unwrap());
                set_api = true;
            }

            // Slightly enlarge intervals between blocks.
            node_cfg.consensus.first_round_timeout *= 2;
            node_cfg.consensus.min_propose_timeout *= 2;
            node_cfg.consensus.max_propose_timeout *= 2;
        })
        .init_node(|genesis, rt| {
            supervisor.clone().deploy(genesis, rt);
            main_service.clone().deploy(genesis, rt);
        })
        .build();

    let probes = if args.no_probe {
        None
    } else {
        let supervisor_probe = ApiProbe::new("http://127.0.0.1:8080/api/services/supervisor/ping");
        let main_probe = ApiProbe::new("http://127.0.0.1:8080/api/services/main/ping");
        Some((supervisor_probe, main_probe))
    };

    loop {
        let height = nodes[0].blockchain().last_block().height;
        println!("Blockchain height: {:?}", height);
        if args.max_height.map_or(false, |max| height >= Height(max)) {
            break;
        }
        delay_for(Duration::from_secs(1)).await;
    }

    let snapshot = nodes[0].blockchain().snapshot();
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

    if let Some((supervisor_probe, main_probe)) = probes {
        println!(
            "Supervisor availability: {:#?}",
            supervisor_probe.get().await
        );
        println!("Main service availability: {:#?}", main_probe.get().await);
    }
}
