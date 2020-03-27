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

//! Runs a network with custom `Propose` creation logic: blocks are not created when
//! there are no transactions.

use exonum::{
    blockchain::{ApiSender, Blockchain},
    crypto::KeyPair,
    helpers::Height,
    runtime::SnapshotExt,
};
use exonum_node::proposer::SkipEmptyBlocks;
use exonum_rust_runtime::{
    spec::{Deploy, Spec},
    DefaultInstance,
};
use structopt::StructOpt;
use tokio::time::delay_for;

use std::time::Duration;

use exonum_soak_tests::{
    services::{MainConfig, MainService, MainServiceInterface},
    NetworkBuilder,
};

fn get_epoch(blockchain: &Blockchain) -> Height {
    let snapshot = blockchain.snapshot();
    let schema = snapshot.for_core();
    let last_block = schema.block_skip().unwrap_or_else(|| schema.last_block());
    last_block
        .epoch()
        .expect("No epoch recorded in a saved block")
}

/// Runs a network with custom `Propose` creation logic: blocks are not created when
/// there are no transactions.
#[derive(Debug, StructOpt)]
struct Args {
    /// Number of nodes in the network.
    #[structopt(name = "nodes", default_value = "4")]
    node_count: u16,

    /// Blockchain height to reach. If not specified, the test will run infinitely.
    #[structopt(name = "max-height", long, short = "H")]
    max_height: Option<u64>,

    /// Interval between sending a transaction to a random node, measured in milliseconds.
    #[structopt(name = "tx-interval", long, short = "t", default_value = "2000")]
    tx_interval: u64,
}

async fn send_transactions(sender: ApiSender, interval: Duration) {
    let mut counter = Height(0);
    let keys = KeyPair::random();
    loop {
        let tx = keys.timestamp(MainService::INSTANCE_ID, counter);
        log::trace!("Sending transaction #{}", counter.0 + 1);
        if sender.broadcast_transaction(tx).await.is_err() {
            return;
        }
        counter.increment();
        delay_for(interval).await;
    }
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
        .with_block_proposer(SkipEmptyBlocks)
        .build();

    let sender = nodes[0].blockchain().sender().to_owned();
    let interval = Duration::from_millis(args.tx_interval);
    tokio::spawn(send_transactions(sender, interval));

    loop {
        let height = nodes[0].blockchain().last_block().height;
        let epoch = get_epoch(nodes[0].blockchain());
        println!("Blockchain height: {:?} / epoch: {:?}", height, epoch);
        if args.max_height.map_or(false, |max| height >= Height(max)) {
            break;
        }
        delay_for(Duration::from_millis(500)).await;
    }
}
