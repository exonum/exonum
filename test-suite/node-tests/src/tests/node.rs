// Copyright 2019 The Exonum Team
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

// This is a regression test for exonum node.

use exonum::{
    blockchain::config::GenesisConfigBuilder,
    helpers,
    node::{ExternalMessage, Node, NodeConfig},
    runtime::{
        rust::{AfterCommitContext, Service, ServiceFactory},
        RuntimeInstance,
    },
};
use exonum_derive::{ServiceDispatcher, ServiceFactory};
use exonum_merkledb::{Database, TemporaryDB};
use futures::{sync::mpsc, Future, Stream};
use tokio::util::FutureExt;
use tokio_core::reactor::Core;

use std::{
    sync::{Arc, Mutex},
    thread::{self},
    time::Duration,
};

use crate::RunHandle;

#[derive(Debug, Clone, ServiceDispatcher, ServiceFactory)]
#[service_factory(
    artifact_name = "after-commit",
    artifact_version = "1.0.0",
    proto_sources = "exonum::proto::schema",
    service_constructor = "CommitWatcherService::new_instance"
)]
struct CommitWatcherService(mpsc::UnboundedSender<()>);

impl CommitWatcherService {
    fn new_instance(&self) -> Box<dyn Service> {
        Box::new(self.clone())
    }
}

impl Service for CommitWatcherService {
    fn after_commit(&self, _context: AfterCommitContext<'_>) {
        self.0.unbounded_send(()).ok();
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
struct StartCheckerServiceFactory(pub Arc<Mutex<u64>>);

impl StartCheckerServiceFactory {
    fn new_instance(&self) -> Box<dyn Service> {
        *self.0.lock().unwrap() += 1;
        Box::new(StartCheckerService)
    }
}

fn run_nodes(count: u16, start_port: u16) -> (Vec<RunHandle>, Vec<mpsc::UnboundedReceiver<()>>) {
    let mut node_threads = Vec::new();
    let mut commit_rxs = Vec::new();
    for node_cfg in helpers::generate_testnet_config(count, start_port) {
        let (commit_tx, commit_rx) = mpsc::unbounded();

        let external_runtimes: Vec<RuntimeInstance> = vec![];
        let service = CommitWatcherService(commit_tx);
        let artifact = service.artifact_id();
        let genesis_config =
            GenesisConfigBuilder::with_consensus_config(node_cfg.consensus.clone())
                .with_artifact(artifact.clone())
                .with_instance(artifact.into_default_instance(2, "commit-watcher"))
                .build();
        let services = vec![service.into()];

        let node = Node::new(
            TemporaryDB::new(),
            external_runtimes,
            services,
            node_cfg,
            genesis_config,
            None,
        );

        let api_tx = node.channel();
        node_threads.push(RunHandle {
            node_thread: thread::spawn(move || {
                node.run().unwrap();
            }),
            api_tx,
        });
        commit_rxs.push(commit_rx);
    }
    (node_threads, commit_rxs)
}

#[test]
fn test_node_run() {
    let (nodes, commit_rxs) = run_nodes(4, 16_300);

    let mut core = Core::new().unwrap();
    let duration = Duration::from_secs(60);
    for rx in commit_rxs {
        let future = rx.into_future().timeout(duration).map_err(drop);
        core.run(future).expect("failed commit");
    }

    for handle in nodes {
        handle
            .api_tx
            .send_external_message(ExternalMessage::Shutdown)
            .unwrap();
        handle.node_thread.join().unwrap();
    }
}

#[test]
fn test_node_restart_regression() {
    let start_node = |node_cfg: NodeConfig, db, start_times| {
        let external_runtimes: Vec<RuntimeInstance> = vec![];
        let service = StartCheckerServiceFactory(start_times);
        let artifact = service.artifact_id();
        let genesis_config =
            GenesisConfigBuilder::with_consensus_config(node_cfg.consensus.clone())
                .with_artifact(artifact.clone())
                .with_instance(artifact.into_default_instance(4, "startup-checker"))
                .build();
        let services = vec![service.into()];

        let node = Node::new(
            db,
            external_runtimes,
            services,
            node_cfg,
            genesis_config,
            None,
        );
        let api_tx = node.channel();
        let node_thread = thread::spawn(move || {
            node.run().unwrap();
        });
        // Wait for shutdown
        api_tx
            .send_external_message(ExternalMessage::Shutdown)
            .unwrap();
        node_thread.join().unwrap();
    };

    let db = Arc::from(TemporaryDB::new()) as Arc<dyn Database>;
    let node_cfg = helpers::generate_testnet_config(1, 3600)[0].clone();

    let start_times = Arc::new(Mutex::new(0));
    // First launch
    start_node(node_cfg.clone(), db.clone(), Arc::clone(&start_times));
    // Second launch
    start_node(node_cfg, db, Arc::clone(&start_times));

    // The service is created two times on instantiation (for `start_adding_service`
    // and `commit_service` methods), and then once on each new node startup.
    assert_eq!(*start_times.lock().unwrap(), 3);
}
