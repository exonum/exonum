// Copyright 2017 The Exonum Team
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
extern crate tokio_timer;

use std::thread::{self, JoinHandle};
use std::time::Duration;
use std::sync::Mutex;

use futures::Future;
use futures::sync::oneshot;
use tokio_timer::Timer;
use exonum::blockchain::{Service, ServiceContext, Transaction};
use exonum::encoding::Error as EncodingError;
use exonum::messages::RawTransaction;
use exonum::node::{Node, ApiSender, ExternalMessage};
use exonum::storage::{MemoryDB, Snapshot};
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

fn run_nodes(count: u8) -> (Vec<(JoinHandle<()>, ApiSender)>, Vec<oneshot::Receiver<()>>) {
    let mut node_threads = Vec::new();
    let mut commit_rxs = Vec::new();
    for node_cfg in helpers::generate_testnet_config(count, 16_300) {
        let (commit_tx, commit_rx) = oneshot::channel();
        let service = Box::new(CommitWatcherService(Mutex::new(Some(commit_tx))));
        let node = Node::new(Box::new(MemoryDB::new()), vec![service], node_cfg);
        let api_tx = node.channel();
        let node_thread = thread::spawn(move || { node.run().unwrap(); });
        node_threads.push((node_thread, api_tx));
        commit_rxs.push(commit_rx);
    }
    (node_threads, commit_rxs)
}

#[test]
fn test_node_run() {
    let (nodes, commit_rxs) = run_nodes(4);

    let timer = Timer::default();
    let duration = Duration::from_secs(60);
    for rx in commit_rxs {
        let rx = timer.timeout(rx.map_err(drop), duration);
        rx.wait().unwrap();
    }

    for (handle, api_tx) in nodes {
        api_tx
            .send_external_message(ExternalMessage::Shutdown)
            .unwrap();
        handle.join().unwrap();
    }
}
