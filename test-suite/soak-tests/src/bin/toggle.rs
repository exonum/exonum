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

use exonum::{helpers::Height, merkledb::ObjectHash, runtime::SnapshotExt};
use exonum_rust_runtime::{
    spec::{Deploy, Spec},
    DefaultInstance,
};
use structopt::StructOpt;

use std::{thread, time::Duration};

use exonum_soak_tests::{
    run_nodes,
    services::{MainConfig, MainService, MainServiceInterface, TogglingSupervisor},
    RunHandle,
};

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

    let config = MainConfig {
        generate_tx_in_after_commit: true,
    };
    let main_service = Spec::new(MainService).with_instance(
        MainService::INSTANCE_ID,
        MainService::INSTANCE_NAME,
        config,
    );
    let supervisor = Spec::new(TogglingSupervisor).with_default_instance();
    let nodes = run_nodes(args.node_count, 2_000, |genesis, rt| {
        main_service.clone().deploy(genesis, rt);
        supervisor.clone().deploy(genesis, rt);
    });

    loop {
        let height = nodes[0].blockchain().last_block().height;
        println!("Blockchain height: {:?}", height);
        if args.max_height.map_or(false, |max| height >= Height(max)) {
            break;
        }
        thread::sleep(Duration::from_secs(1));
    }

    let snapshot = nodes[0].blockchain().snapshot();
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
