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

use exonum::{
    blockchain::TransactionCache,
    crypto::Hash,
    helpers::Height,
    merkledb::{access::AccessExt, BinaryValue, Snapshot},
    runtime::SnapshotExt,
};
use exonum_node::pool::{ManagePool, Pool, ProposeParams, ProposeTemplate};
use exonum_rust_runtime::{
    spec::{Deploy, Spec},
    DefaultInstance,
};
use structopt::StructOpt;
use tokio::time::sleep;

use std::{borrow::Cow, iter, time::Duration};

use exonum_soak_tests::{
    send_transactions,
    services::{MainConfig, MainService},
    NetworkBuilder,
};

/// Custom pool manager that filters transactions so that only the newest transaction
/// is proposed and retained in the pool. (The transaction actuality is determined via
/// an auto-incrementing integer field.)
#[derive(Debug, Clone)]
struct NewestTimestamp;

impl NewestTimestamp {
    /// Returns hash of the newest transactions and the vector with the older transactions.
    fn find_newest_stamp(pool: &Pool<'_>) -> (Option<Hash>, Vec<Hash>) {
        let mut other_txs = vec![];
        let mut max_index = None;

        for (tx_hash, tx) in pool.transactions() {
            let index = u64::from_bytes(Cow::Borrowed(&tx.payload().arguments)).unwrap();
            if max_index.map_or(true, |(max, _)| max < index) {
                if let Some((_, old_tx_hash)) = max_index {
                    other_txs.push(old_tx_hash);
                }
                max_index = Some((index, tx_hash));
            } else {
                other_txs.push(tx_hash);
            }
        }

        (max_index.map(|(_, tx_hash)| tx_hash), other_txs)
    }
}

impl ManagePool for NewestTimestamp {
    // Selects the newest timestamp.
    fn propose_block(&mut self, pool: Pool<'_>, _params: ProposeParams<'_>) -> ProposeTemplate {
        let (max_tx_hash, _) = Self::find_newest_stamp(&pool);
        if let Some(tx_hash) = max_tx_hash {
            ProposeTemplate::ordinary(iter::once(tx_hash))
        } else {
            ProposeTemplate::Skip
        }
    }

    // Removes all timestamps except for the newest one.
    fn remove_transactions(&mut self, pool: Pool<'_>, _snapshot: &dyn Snapshot) -> Vec<Hash> {
        let (_, removed_txs) = Self::find_newest_stamp(&pool);
        removed_txs
    }
}

/// Runs a network with custom `Propose` creation logic and filtering: only blocks with the
/// latest timestamp are proposed by the nodes, and all other transactions are filtered
/// on block commit.
///
/// Although the newest transaction may differ from node to node, the consensus algorithm
/// handles this situation just fine; if a node proposes a block with an "outdated" transaction,
/// other nodes will query it and restore the transaction themselves.
#[derive(Debug, StructOpt)]
#[structopt(name = "tx_filtering", set_term_width = 80)]
struct Args {
    /// Number of nodes in the network.
    #[structopt(name = "nodes", default_value = "4")]
    node_count: u16,

    /// Blockchain height to reach. If not specified, the test will run infinitely.
    #[structopt(name = "max-height", long, short = "H")]
    max_height: Option<u64>,

    /// Interval between sending a transaction to a node, measured in milliseconds.
    #[structopt(name = "tx-interval", long, short = "t", default_value = "20")]
    tx_interval: u64,
}

#[tokio::main]
async fn main() {
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

    let nodes = NetworkBuilder::new(args.node_count, 2_000)
        .init_node(|genesis, rt| main_service.clone().deploy(genesis, rt))
        .with_pool_manager(NewestTimestamp)
        .build();

    let sender = nodes[0].blockchain().sender().to_owned();
    let interval = Duration::from_millis(args.tx_interval);
    tokio::spawn(send_transactions(sender, interval));

    loop {
        let last_block = nodes[0].blockchain().last_block();
        println!(
            "Blockchain height: {:?}, txs in last block: {}",
            last_block.height, last_block.tx_count
        );

        let snapshot = nodes[0].blockchain().snapshot();
        let tx_len = snapshot.for_core().transactions_len();
        let latest_stamp = snapshot
            .for_service(MainService::INSTANCE_ID)
            .unwrap()
            .get_entry::<_, Height>("latest_stamp")
            .get()
            .unwrap_or(Height(0));
        println!(
            "Total committed transactions: {} (latest stamp = {})",
            tx_len, latest_stamp
        );

        if args
            .max_height
            .map_or(false, |max| last_block.height >= Height(max))
        {
            break;
        }
        sleep(Duration::from_millis(500)).await;
    }
}
