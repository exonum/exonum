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

use exonum::blockchain::{Blockchain, Service, ServiceContext, Transaction};
use exonum::encoding::Error as EncodingError;
use exonum::messages::RawTransaction;
use exonum::node::Node;
use exonum::storage::MemoryDB;
use exonum::helpers;

struct CommitWatcherService(pub Mutex<Option<oneshot::Sender<()>>>);

impl Service for CommitWatcherService {
    fn service_id(&self) -> u16 {
        255
    }

    fn service_name(&self) -> &'static str {
        "commit_watcher"
    }

    fn tx_from_raw(&self, _raw: RawTransaction) -> Result<Box<Transaction>, EncodingError> {
        unreachable!("An unknown transaction received");
    }

    fn handle_commit(&self, _context: &mut ServiceContext) {
        if let Some(oneshot) = self.0.lock().unwrap().take() {
            oneshot.send(()).unwrap();
        }
    }
}

fn run_nodes(count: u8) -> (Vec<JoinHandle<()>>, Vec<oneshot::Receiver<()>>) {
    let mut node_threads = Vec::new();
    let mut commit_rxs = Vec::new();
    for node_cfg in helpers::generate_testnet_config(count, 16_300) {
        let (commit_tx, commit_rx) = oneshot::channel();
        let service = Box::new(CommitWatcherService(Mutex::new(Some(commit_tx))));
        let blockchain = Blockchain::new(Box::new(MemoryDB::new()), vec![service]);
        let node_thread = thread::spawn(move || {
            let node = Node::new(blockchain, node_cfg);
            node.run().unwrap();
        });
        node_threads.push(node_thread);
        commit_rxs.push(commit_rx);
    }
    (node_threads, commit_rxs)
}

#[test]
fn test_node_run() {
    let (_, commit_rxs) = run_nodes(4);

    let timer = Timer::default();
    let duration = Duration::from_secs(60);
    for rx in commit_rxs {
        let rx = timer.timeout(rx.map_err(drop), duration);
        rx.wait().unwrap();
    }
}
