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
    blockchain::InstanceCollection,
    helpers, impl_service_dispatcher,
    node::{ApiSender, ExternalMessage, Node, NodeConfig},
    runtime::rust::{AfterCommitContext, RustArtifactSpec, Service, ServiceFactory},
};
use exonum_derive::service_interface;
use exonum_merkledb::{Database, TemporaryDB};
use futures::{sync::oneshot, Future, IntoFuture};
use tokio::util::FutureExt;
use tokio_core::reactor::Core;

use std::{
    cell::RefCell,
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};

#[service_interface]
trait CommitWatcherInterface {}

#[derive(Debug)]
struct CommitWatcherService(pub RefCell<Option<oneshot::Sender<()>>>);

impl CommitWatcherInterface for CommitWatcherService {}

impl_service_dispatcher!(CommitWatcherService, CommitWatcherInterface);

impl Service for CommitWatcherService {
    fn after_commit(&self, _context: AfterCommitContext) {
        if let Some(oneshot) = self.0.borrow_mut().take() {
            oneshot.send(()).unwrap();
        }
    }
}

impl ServiceFactory for CommitWatcherService {
    fn artifact(&self) -> RustArtifactSpec {
        "after-commit/1.0.0".parse().unwrap()
    }

    fn new_instance(&self) -> Box<dyn Service> {
        Box::new(Self(RefCell::new(self.0.borrow_mut().take())))
    }
}

#[service_interface]
trait StartCheckerInterface {}

#[derive(Debug)]
struct StartCheckerService;

impl StartCheckerInterface for StartCheckerService {}

impl_service_dispatcher!(StartCheckerService, StartCheckerInterface);

impl Service for StartCheckerService {}

#[derive(Debug)]
struct StartCheckerServiceFactory(pub Arc<Mutex<u64>>);

impl ServiceFactory for StartCheckerServiceFactory {
    fn artifact(&self) -> RustArtifactSpec {
        "configure/1.0.0".parse().unwrap()
    }

    fn new_instance(&self) -> Box<dyn Service> {
        *self.0.lock().unwrap() += 1;
        Box::new(StartCheckerService)
    }
}

struct RunHandle {
    node_thread: JoinHandle<()>,
    api_tx: ApiSender,
}

fn run_nodes(count: u16, start_port: u16) -> (Vec<RunHandle>, Vec<oneshot::Receiver<()>>) {
    let mut node_threads = Vec::new();
    let mut commit_rxs = Vec::new();
    for node_cfg in helpers::generate_testnet_config(count, start_port) {
        let (commit_tx, commit_rx) = oneshot::channel();

        let node = Node::new(
            TemporaryDB::new(),
            vec![
                InstanceCollection::new(CommitWatcherService(RefCell::new(Some(commit_tx))))
                    .with_instance(2, "commit-watcher", ()),
            ],
            node_cfg,
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
#[ignore = "TODO: Research why node tests randomly fails. [ECR-2363]"]
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
    let start_node =
        |node_cfg: NodeConfig, db, start_times| {
            let node =
                Node::new(
                    db,
                    vec![
                        InstanceCollection::new(StartCheckerServiceFactory(start_times))
                            .with_instance(4, "startup-checker", ()),
                    ],
                    node_cfg,
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
    assert_eq!(*start_times.lock().unwrap(), 2);
}
