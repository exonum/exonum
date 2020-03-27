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
use exonum_node::{
    generate_testnet_config, proposer::ProposeBlock, Node, NodeBuilder, NodeConfig, ShutdownHandle,
};
use exonum_rust_runtime::{RustRuntime, RustRuntimeBuilder};
use futures::TryFutureExt;
use tokio::task::JoinHandle;

use std::{fmt, sync::Arc};

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

type ProposerGen = Box<dyn Fn() -> Box<dyn ProposeBlock>>;

pub struct NetworkBuilder<'a> {
    count: u16,
    start_port: u16,
    modify_cfg: Option<Box<dyn FnMut(&mut NodeConfig) + 'a>>,
    init_node: Option<Box<dyn FnMut(&mut GenesisConfigBuilder, &mut RustRuntimeBuilder) + 'a>>,
    block_proposer: Option<ProposerGen>,
}

impl fmt::Debug for NetworkBuilder<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NetworkBuilder")
            .field("count", &self.count)
            .field("start_port", &self.start_port)
            .finish()
    }
}

impl<'a> NetworkBuilder<'a> {
    /// Starts building an Exonum network.
    pub fn new(count: u16, start_port: u16) -> Self {
        Self {
            count,
            start_port,
            modify_cfg: None,
            init_node: None,
            block_proposer: None,
        }
    }

    /// Allows to modify node configs before the nodes are started.
    pub fn modify_config<F>(mut self, modify_cfg: F) -> Self
    where
        F: FnMut(&mut NodeConfig) + 'a,
    {
        self.modify_cfg = Some(Box::new(modify_cfg));
        self
    }

    /// Customizes services on the nodes.
    pub fn init_node<F>(mut self, init_node: F) -> Self
    where
        F: FnMut(&mut GenesisConfigBuilder, &mut RustRuntimeBuilder) + 'a,
    {
        self.init_node = Some(Box::new(init_node));
        self
    }

    /// Customizes block proposal logic.
    pub fn with_block_proposer<T>(mut self, proposer: T) -> Self
    where
        T: ProposeBlock + Clone + 'static,
    {
        let f = move || Box::new(proposer.clone()) as Box<dyn ProposeBlock>;
        self.block_proposer = Some(Box::new(f));
        self
    }

    /// Builds the network and returns handles for all nodes.
    pub fn build(mut self) -> Vec<RunHandle> {
        let mut node_handles = Vec::with_capacity(self.count as usize);

        let configs = generate_testnet_config(self.count, self.start_port);
        for (mut node_cfg, node_keys) in configs {
            if let Some(modify_cfg) = self.modify_cfg.as_mut() {
                modify_cfg(&mut node_cfg);
            }
            let mut genesis_cfg =
                GenesisConfigBuilder::with_consensus_config(node_cfg.consensus.clone());
            let mut rt = RustRuntime::builder();
            if let Some(init_node) = self.init_node.as_mut() {
                init_node(&mut genesis_cfg, &mut rt);
            }

            let db = Arc::new(TemporaryDB::new()) as Arc<dyn Database>;
            let mut node_builder = NodeBuilder::new(db, node_cfg, node_keys)
                .with_genesis_config(genesis_cfg.build())
                .with_runtime_fn(|channel| rt.build(channel.endpoints_sender()));

            if let Some(ref gen_proposer) = self.block_proposer {
                node_builder = node_builder.with_block_proposer(gen_proposer());
            }
            node_handles.push(RunHandle::new(node_builder.build()));
        }
        node_handles
    }
}
