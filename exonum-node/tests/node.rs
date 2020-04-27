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

//! High-level tests for the Exonum node.

use exonum::{
    blockchain::config::GenesisConfigBuilder,
    crypto::KeyPair,
    helpers::{Height, Round},
    merkledb::{Database, ObjectHash, TemporaryDB},
    runtime::SnapshotExt,
};
use exonum_rust_runtime::{RustRuntime, ServiceFactory};
use futures::prelude::*;
use tokio::time::{delay_for, timeout};

use std::{
    net::{Ipv4Addr, SocketAddr, TcpListener},
    sync::{Arc, Mutex},
    time::Duration,
};

use exonum_node::{generate_testnet_config, NodeBuilder, NodeConfig};

pub mod common;
use crate::common::{
    run_nodes, CommitWatcherService, DummyInterface, Options, RunHandle, StartCheckerServiceFactory,
};

#[tokio::test]
async fn nodes_commit_blocks() {
    const TIMEOUT: Duration = Duration::from_secs(10);

    let (nodes, commit_rxs) = run_nodes(4, 16_300, Options::default());
    let commit_notifications = commit_rxs.into_iter().map(|mut rx| async move {
        if timeout(TIMEOUT, rx.next()).await.is_err() {
            panic!("Timed out");
        }
    });
    future::join_all(commit_notifications).await;

    // Check that nodes do not skip the first round of the first block.
    let snapshot = nodes[0].blockchain.snapshot();
    let block_proof = snapshot.for_core().block_and_precommits(Height(1)).unwrap();
    assert!(block_proof
        .precommits
        .iter()
        .all(|precommit| precommit.payload().round == Round(1)));

    future::join_all(nodes.into_iter().map(RunHandle::join)).await;
}

#[tokio::test]
async fn node_frees_sockets_on_shutdown() {
    let options = Options {
        http_start_port: Some(16_351),
        ..Options::default()
    };
    let (mut nodes, mut commit_rxs) = run_nodes(1, 16_350, options);
    let node = nodes.pop().unwrap();
    let mut commit_rx = commit_rxs.pop().unwrap();
    commit_rx.next().await;
    node.join().await;

    delay_for(Duration::from_millis(100)).await;

    // The sockets used by the node should be freed now.
    TcpListener::bind(SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 16_350)).unwrap();
    TcpListener::bind(SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 16_351)).unwrap();
}

#[tokio::test]
async fn nodes_flush_transactions_to_storage_before_commit() {
    // `slow_blocks: true` argument makes it so that nodes should not create a single block
    // during the test.
    let options = Options {
        slow_blocks: true,
        ..Options::default()
    };
    let (nodes, _) = run_nodes(4, 16_400, options);
    delay_for(Duration::from_secs(5)).await;

    // Send some transactions over `blockchain`s.
    let keys = KeyPair::random();
    let (tx_hashes, broadcasts): (Vec<_>, Vec<_>) = (0_u64..10)
        .map(|i| {
            let tx = keys.timestamp(CommitWatcherService::ID, i);
            let tx_hash = tx.object_hash();
            let node_i = i as usize % nodes.len();
            let broadcast = nodes[node_i].blockchain.sender().broadcast_transaction(tx);
            (tx_hash, broadcast)
        })
        .unzip();
    future::try_join_all(broadcasts).await.unwrap();

    // Nodes need order of 100ms to create a column family for the tx pool in the debug mode,
    // so we sleep here to make it happen for all nodes.
    delay_for(Duration::from_millis(500)).await;

    // All transactions should be persisted on all nodes now.
    for node in &nodes {
        let snapshot = node.blockchain.snapshot();
        let snapshot = snapshot.for_core();
        assert_eq!(snapshot.height(), Height(0));
        let tx_pool = snapshot.transactions_pool();
        for tx_hash in &tx_hashes {
            assert!(tx_pool.contains(tx_hash));
        }
    }

    future::join_all(nodes.into_iter().map(RunHandle::join)).await;
}

#[tokio::test]
async fn nodes_commit_blocks_with_custom_proposal_logic() {
    const TIMEOUT: Duration = Duration::from_secs(10);

    let options = Options {
        skip_empty_blocks: true,
        ..Options::default()
    };
    let (nodes, mut commit_rxs) = run_nodes(4, 16_500, options);

    // Send a transaction to the node and wait for it to be committed.
    let tx = KeyPair::random().timestamp(CommitWatcherService::ID, 0);
    let tx_hash = tx.object_hash();
    let send_tx = nodes[0].blockchain.sender().broadcast_transaction(tx);
    send_tx.await.unwrap();

    let commit_notifications = commit_rxs
        .iter_mut()
        .map(|rx| async move { timeout(TIMEOUT, rx.next()).await });
    future::try_join_all(commit_notifications).await.unwrap();

    let snapshot = nodes[1].blockchain.snapshot();
    let schema = snapshot.for_core();
    assert!(schema.transactions().contains(&tx_hash));
    assert!(schema.transactions_locations().contains(&tx_hash));

    // Check that no new blocks are being approved when there are no transactions.
    delay_for(TIMEOUT / 2).await;
    for commit_rx in &mut commit_rxs {
        assert!(commit_rx.next().now_or_never().is_none());
    }

    let other_tx = KeyPair::random().timestamp(CommitWatcherService::ID, 0);
    let other_send_tx = nodes[0].blockchain.sender().broadcast_transaction(other_tx);
    other_send_tx.await.unwrap();

    let new_commit_notifications = commit_rxs
        .iter_mut()
        .map(|rx| async move { timeout(TIMEOUT, rx.next()).await });
    future::try_join_all(new_commit_notifications)
        .await
        .unwrap();

    let last_block = nodes[0].blockchain.last_block();
    assert_eq!(last_block.height, Height(2));
    assert!(last_block.epoch().unwrap() > Height(5));

    future::join_all(nodes.into_iter().map(RunHandle::join)).await;
}

#[tokio::test]
async fn node_restart_regression() {
    let start_node = |node_cfg: NodeConfig, node_keys, db, start_times| {
        let service = StartCheckerServiceFactory(start_times);
        let artifact = service.artifact_id();
        let genesis_config =
            GenesisConfigBuilder::with_consensus_config(node_cfg.consensus.clone())
                .with_artifact(artifact.clone())
                .with_instance(artifact.into_default_instance(4, "startup-checker"))
                .build();

        let node = NodeBuilder::new(db, node_cfg, node_keys)
            .with_genesis_config(genesis_config)
            .with_runtime_fn(|channel| {
                RustRuntime::builder()
                    .with_factory(service)
                    .build(channel.endpoints_sender())
            })
            .build();
        RunHandle::new(node).join()
    };

    let db = Arc::new(TemporaryDB::new()) as Arc<dyn Database>;
    let (node_cfg, node_keys) = generate_testnet_config(1, 3_600).pop().unwrap();

    let start_times = Arc::new(Mutex::new(0));
    // First launch
    start_node(
        node_cfg.clone(),
        node_keys.clone(),
        Arc::clone(&db),
        Arc::clone(&start_times),
    )
    .await;
    // Second launch
    start_node(node_cfg, node_keys, db, Arc::clone(&start_times)).await;

    // The service is created two times on instantiation (for `start_adding_service`
    // and `commit_service` methods), and then once on each new node startup.
    assert_eq!(*start_times.lock().unwrap(), 3);
}
