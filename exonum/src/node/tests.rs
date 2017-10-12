use std::thread::{self, JoinHandle};
use std::time::Duration;

use futures::{Future, Sink, Stream};
use futures::sync::mpsc;
use tokio_timer::Timer;

use blockchain::{Blockchain, Service, ServiceContext, Transaction};
use encoding::Error as EncodingError;
use messages::RawTransaction;
use node::Node;
use storage::MemoryDB;
use helpers;

struct CommitWatcherService(pub mpsc::Sender<()>);

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
        self.0.clone().send(()).wait().unwrap();
    }
}

fn run_nodes(count: u8) -> (Vec<JoinHandle<()>>, Vec<mpsc::Receiver<()>>) {
    let mut node_threads = Vec::new();
    let mut commit_rxs = Vec::new();
    for node_cfg in helpers::generate_testnet_config(count, 16300) {
        let (commit_tx, commit_rx) = mpsc::channel(1);
        let service = Box::new(CommitWatcherService(commit_tx));
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
    let _ = helpers::init_logger();

    let (_, commit_rxs) = run_nodes(2);

    let timer = Timer::default();
    let duration = Duration::from_secs(60);
    for rx in commit_rxs {
        let rx = timer.timeout_stream(rx, duration);
        rx.wait().next().unwrap().unwrap();
    }
}
