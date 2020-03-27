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
    merkledb::TemporaryDB,
    runtime::{ExecutionContext, ExecutionError, InstanceId},
};
use exonum_derive::*;
use exonum_rust_runtime::{AfterCommitContext, RustRuntime, Service, ServiceFactory};
use futures::{channel::mpsc, prelude::*};
use tokio::task::JoinHandle;

use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::{Arc, Mutex},
};

use exonum_node::{
    generate_testnet_config, proposer::SkipEmptyBlocks, Node, NodeBuilder, ShutdownHandle,
};

#[derive(Debug)]
pub struct RunHandle {
    pub blockchain: Blockchain,
    node_task: JoinHandle<()>,
    shutdown_handle: ShutdownHandle,
}

impl RunHandle {
    pub fn new(node: Node) -> Self {
        let blockchain = node.blockchain().to_owned();
        let shutdown_handle = node.shutdown_handle();
        let node_task = node.run().unwrap_or_else(|err| panic!("{}", err));
        Self {
            blockchain,
            shutdown_handle,
            node_task: tokio::spawn(node_task),
        }
    }

    /// Returns the shutdown handle for this node.
    pub fn shutdown_handle(&self) -> ShutdownHandle {
        self.shutdown_handle.clone()
    }

    /// Waits for the node to shut down without terminating it.
    pub async fn run(self) {
        self.node_task.await.unwrap()
    }

    /// Terminates the node and waits for it to shut down.
    pub async fn join(self) {
        self.shutdown_handle.shutdown().await.unwrap();
        self.node_task.await.unwrap();
    }
}

#[exonum_interface(auto_ids)]
pub trait DummyInterface<Ctx> {
    type Output;
    fn timestamp(&self, context: Ctx, _value: u64) -> Self::Output;
}

#[derive(Debug, Clone, ServiceDispatcher, ServiceFactory)]
#[service_dispatcher(implements("DummyInterface"))]
#[service_factory(
    artifact_name = "after-commit",
    artifact_version = "1.0.0",
    proto_sources = "exonum::proto::schema",
    service_constructor = "CommitWatcherService::new_instance"
)]
pub struct CommitWatcherService(mpsc::UnboundedSender<()>);

impl CommitWatcherService {
    pub const ID: InstanceId = 2;

    fn new_instance(&self) -> Box<dyn Service> {
        Box::new(self.clone())
    }
}

impl Service for CommitWatcherService {
    fn after_commit(&self, _context: AfterCommitContext<'_>) {
        self.0.unbounded_send(()).ok();
    }
}

impl DummyInterface<ExecutionContext<'_>> for CommitWatcherService {
    type Output = Result<(), ExecutionError>;

    fn timestamp(&self, _context: ExecutionContext<'_>, _value: u64) -> Self::Output {
        Ok(())
    }
}

#[derive(Debug, ServiceDispatcher)]
struct StartCheckerService;

impl Service for StartCheckerService {}

#[derive(Debug, ServiceFactory)]
#[service_factory(
    artifact_name = "configure",
    artifact_version = "1.0.2",
    proto_sources = "exonum::proto::schema",
    service_constructor = "StartCheckerServiceFactory::new_instance"
)]
pub struct StartCheckerServiceFactory(pub Arc<Mutex<u64>>);

impl StartCheckerServiceFactory {
    fn new_instance(&self) -> Box<dyn Service> {
        *self.0.lock().unwrap() += 1;
        Box::new(StartCheckerService)
    }
}

#[derive(Clone, Copy, Default)]
pub struct Options {
    pub slow_blocks: bool,
    pub skip_empty_blocks: bool,
    pub http_start_port: Option<u16>,
    pub disable_signals: bool,
}

pub fn run_nodes(
    count: u16,
    start_port: u16,
    options: Options,
) -> (Vec<RunHandle>, Vec<mpsc::UnboundedReceiver<()>>) {
    let mut node_handles = Vec::new();
    let mut commit_rxs = Vec::new();

    let it = generate_testnet_config(count, start_port)
        .into_iter()
        .enumerate();
    for (i, (mut node_cfg, node_keys)) in it {
        let (commit_tx, commit_rx) = mpsc::unbounded();
        if options.slow_blocks {
            node_cfg.consensus.first_round_timeout = 20_000;
            node_cfg.consensus.min_propose_timeout = 10_000;
            node_cfg.consensus.max_propose_timeout = 10_000;
        }
        if let Some(start_port) = options.http_start_port {
            let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), start_port + i as u16);
            node_cfg.api.public_api_address = Some(addr);
        }

        let service = CommitWatcherService(commit_tx);
        let artifact = service.artifact_id();
        let instance = artifact
            .clone()
            .into_default_instance(CommitWatcherService::ID, "commit-watcher");
        let genesis_cfg = GenesisConfigBuilder::with_consensus_config(node_cfg.consensus.clone())
            .with_artifact(artifact)
            .with_instance(instance)
            .build();

        let db = TemporaryDB::new();
        let mut node_builder = NodeBuilder::new(db, node_cfg, node_keys)
            .with_genesis_config(genesis_cfg)
            .with_runtime_fn(|channel| {
                RustRuntime::builder()
                    .with_factory(service)
                    .build(channel.endpoints_sender())
            });
        if options.skip_empty_blocks {
            node_builder = node_builder.with_block_proposer(SkipEmptyBlocks);
        }
        if options.disable_signals {
            node_builder = node_builder.disable_signals();
        }

        let node = node_builder.build();
        node_handles.push(RunHandle::new(node));
        commit_rxs.push(commit_rx);
    }

    (node_handles, commit_rxs)
}
