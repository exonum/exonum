// Copyright 2018 The Exonum Team
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
extern crate exonum;
extern crate futures;
extern crate serde_json;
extern crate tokio_timer;

use futures::Future;
use futures::sync::oneshot;
use tokio_timer::Timer;
use serde_json::Value;

use std::thread::{self, JoinHandle};
use std::time::Duration;
use std::sync::{Arc, Mutex};

use exonum::blockchain::{Service, ServiceContext, Transaction};
use exonum::encoding::Error as EncodingError;
use exonum::messages::RawTransaction;
use exonum::node::{ApiSender, ExternalMessage, Node};
use exonum::storage::{Database, Fork, MemoryDB, Snapshot};
use exonum::helpers;
use exonum::crypto::Hash;

struct CommitWatcherService(pub Mutex<Option<oneshot::Sender<()>>>);

impl Service for CommitWatcherService {
    fn service_id(&self) -> u16 {
        255
    }

    fn service_name(&self) -> &str {
        "commit_watcher"
    }

    fn state_hash(&self, _: &Snapshot) -> Vec<Hash> {
        Vec::new()
    }

    fn tx_from_raw(&self, _raw: RawTransaction) -> Result<Box<Transaction>, EncodingError> {
        unreachable!("An unknown transaction received");
    }

    fn handle_commit(&self, _context: &ServiceContext) {
        if let Some(oneshot) = self.0.lock().unwrap().take() {
            oneshot.send(()).unwrap();
        }
    }
}

struct InitializeCheckerService(pub Arc<Mutex<u64>>);

impl Service for InitializeCheckerService {
    fn service_id(&self) -> u16 {
        256
    }

    fn service_name(&self) -> &str {
        "initialize_checker"
    }

    fn state_hash(&self, _: &Snapshot) -> Vec<Hash> {
        Vec::new()
    }

    fn tx_from_raw(&self, _raw: RawTransaction) -> Result<Box<Transaction>, EncodingError> {
        unreachable!("An unknown transaction received");
    }

    fn initialize(&self, _fork: &mut Fork) -> Value {
        *self.0.lock().unwrap() += 1;
        Value::Null
    }
}

struct RunHandle {
    node_thread: JoinHandle<()>,
    api_tx: ApiSender,
}

fn run_nodes(count: u8, start_port: u16) -> (Vec<RunHandle>, Vec<oneshot::Receiver<()>>) {
    let mut node_threads = Vec::new();
    let mut commit_rxs = Vec::new();
    for node_cfg in helpers::generate_testnet_config(count, start_port) {
        let (commit_tx, commit_rx) = oneshot::channel();
        let service = Box::new(CommitWatcherService(Mutex::new(Some(commit_tx))));
        let node = Node::new(MemoryDB::new(), vec![service], node_cfg);
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

    let timer = Timer::default();
    let duration = Duration::from_secs(60);
    for rx in commit_rxs {
        let rx = timer.timeout(rx.map_err(drop), duration);
        rx.wait().unwrap();
    }

    for handle in nodes {
        handle
            .api_tx
            .send_external_message(ExternalMessage::Shutdown)
            .unwrap();
        handle.node_thread.join().unwrap();
    }
}

// See ECR-907 for the details.
#[test]
#[ignore]
fn test_node_shutdown_twice() {
    let (nodes, commit_rxs) = run_nodes(1, 16_400);

    let timer = Timer::default();
    let duration = Duration::from_secs(60);
    for rx in commit_rxs {
        let rx = timer.timeout(rx.map_err(drop), duration);
        rx.wait().unwrap();
    }

    for handle in nodes {
        handle
            .api_tx
            .send_external_message(ExternalMessage::Shutdown)
            .unwrap();
        handle
            .api_tx
            .send_external_message(ExternalMessage::Shutdown)
            .unwrap();
        handle.node_thread.join().unwrap();
    }
}

#[test]
fn test_node_restart_regression() {
    let start_node = |node_cfg, db, init_times| {
        let service = Box::new(InitializeCheckerService(init_times));
        let node = Node::new(db, vec![service], node_cfg);
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

    let db = Arc::from(Box::new(MemoryDB::new()) as Box<Database>) as Arc<Database>;
    let node_cfg = helpers::generate_testnet_config(1, 3600)[0].clone();

    let init_times = Arc::new(Mutex::new(0));
    // First launch
    start_node(node_cfg.clone(), db.clone(), Arc::clone(&init_times));
    // Second launch
    start_node(node_cfg, db, Arc::clone(&init_times));
    assert_eq!(*init_times.lock().unwrap(), 1);
}
