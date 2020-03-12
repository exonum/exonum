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
    blockchain::{config::GenesisConfigBuilder, Blockchain},
    crypto::KeyPair,
    merkledb::{Database, TemporaryDB},
};
use exonum_node::{generate_testnet_config, Node, NodeBuilder, NodeConfig, ShutdownHandle};
use exonum_rust_runtime::{RustRuntime, RustRuntimeBuilder};
use futures::TryFutureExt;
use tokio::task::JoinHandle;

use std::sync::Arc;

pub mod services;

#[derive(Debug)]
pub struct RunHandle {
    node_task: JoinHandle<()>,
    blockchain: Blockchain,
    shutdown_handle: ShutdownHandle,
}

impl RunHandle {
    pub fn new(node: Node) -> Self {
        let blockchain = node.blockchain().to_owned();
        let shutdown_handle = node.shutdown_handle();
        let node_task = node.run().unwrap_or_else(|e| panic!("{}", e));
        Self {
            node_task: tokio::spawn(node_task),
            blockchain,
            shutdown_handle,
        }
    }

    pub fn blockchain(&self) -> &Blockchain {
        &self.blockchain
    }

    pub async fn join(self) -> KeyPair {
        self.shutdown_handle
            .shutdown()
            .await
            .expect("Cannot shut down node");
        self.node_task.await.expect("Node panicked during shutdown");
        self.blockchain.service_keypair().to_owned()
    }
}

pub fn run_nodes(
    count: u16,
    start_port: u16,
    mut modify_cfg: impl FnMut(&mut NodeConfig),
    mut init_node: impl FnMut(&mut GenesisConfigBuilder, &mut RustRuntimeBuilder),
) -> Vec<RunHandle> {
    let mut node_threads = Vec::with_capacity(count as usize);

    let configs = generate_testnet_config(count, start_port);
    for (mut node_cfg, node_keys) in configs {
        modify_cfg(&mut node_cfg);
        let mut genesis_cfg =
            GenesisConfigBuilder::with_consensus_config(node_cfg.consensus.clone());
        let mut rt = RustRuntime::builder();
        init_node(&mut genesis_cfg, &mut rt);

        let db = Arc::new(TemporaryDB::new()) as Arc<dyn Database>;
        let node = NodeBuilder::new(db, node_cfg, node_keys)
            .with_genesis_config(genesis_cfg.build())
            .with_runtime_fn(|channel| rt.build(channel.endpoints_sender()))
            .build();

        node_threads.push(RunHandle::new(node));
    }
    node_threads
}
