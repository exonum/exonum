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

const TRANSACTIONS_COUNT: usize = 40_000;
const SAMPLE_SIZE: usize = 3;

use criterion::{
    AxisScale, Bencher, Criterion, ParameterizedBenchmark, PlotConfiguration, Throughput,
};
use futures::{stream, Future, Sink};
use num::pow::pow;
use tempdir::TempDir;
use tokio_core::reactor::Core;

use std::{io, sync::Arc, thread};

use exonum::{
    blockchain::{
        Blockchain, ExecutionResult, GenesisConfig, Service, SharedNodeState, Transaction,
        TransactionSet, ValidatorKeys,
    },
    crypto::{self, Hash, PublicKey}, encoding,
    events::{error::other_error, Event, EventHandler, HandlerPart, NetworkEvent},
    messages::{Message, RawTransaction},
    node::{
        ApiSender, Configuration, ConnectList, DefaultSystemState, ListenerConfig, NodeApiConfig,
        NodeChannel, NodeConfig, NodeHandler, ServiceConfig,
    },
    storage::{Database, DbOptions, Fork, RocksDB, Snapshot},
};

pub const SERVICE_ID: u16 = 1;

transactions! {
    pub Transactions {
        const SERVICE_ID = SERVICE_ID;

        struct Blank {
            author: &PublicKey,
            bytes: &[u8]
        }
    }
}

impl Transaction for Blank {
    fn verify(&self) -> bool {
        self.verify_signature(self.author())
    }

    fn execute(&self, _fork: &mut Fork) -> ExecutionResult {
        Ok(())
    }
}

struct DummyService;

impl Service for DummyService {
    fn service_name(&self) -> &str {
        "dummy"
    }

    fn state_hash(&self, _: &Snapshot) -> Vec<Hash> {
        Vec::new()
    }

    fn service_id(&self) -> u16 {
        SERVICE_ID
    }

    /// Implement a method to deserialize transactions coming to the node.
    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, encoding::Error> {
        let tx = Transactions::tx_from_raw(raw)?;
        Ok(tx.into())
    }
}

fn gen_transactions(count: usize, len: usize) -> Vec<Box<dyn Transaction>> {
    let padding = vec![0; len];
    (0..count)
        .map(|_| {
            let (p, s) = crypto::gen_keypair();
            Box::new(Blank::new(&p, &padding, &s)) as Box<dyn Transaction>
        })
        .collect()
}

struct TransactionsHandler {
    inner: Option<NodeHandler>,
    txs_count: usize,
}

impl TransactionsHandler {
    fn inner(&self) -> &NodeHandler {
        self.inner.as_ref().unwrap()
    }
}

impl EventHandler for TransactionsHandler {
    fn handle_event(&mut self, event: Event) {
        let is_transaction = if let Event::Network(NetworkEvent::MessageReceived(..)) = event {
            true
        } else {
            false
        };

        self.inner.as_mut().unwrap().handle_event(event);
        if is_transaction {
            self.txs_count -= 1;
            if self.txs_count == 0 {
                self.inner.take();
            }
        }
    }
}

struct TransactionsBenchmarkRunner {
    handler: TransactionsHandler,
    channel: NodeChannel,
    transactions: Vec<RawTransaction>,
}

impl TransactionsBenchmarkRunner {
    fn new(db: Arc<dyn Database>, transactions: impl IntoIterator<Item = RawTransaction>) -> Self {
        let node_config = Self::node_config();
        let channel = NodeChannel::new(&node_config.mempool.events_pool_capacity);
        let mut blockchain = Blockchain::new(
            db,
            vec![Box::new(DummyService) as Box<dyn Service>],
            node_config.service_public_key,
            node_config.service_secret_key.clone(),
            ApiSender::new(channel.api_requests.0.clone()),
        );
        blockchain.initialize(node_config.genesis.clone()).unwrap();

        let peers = node_config.connect_list.addresses();

        let config = Configuration {
            listener: ListenerConfig {
                consensus_public_key: node_config.consensus_public_key,
                consensus_secret_key: node_config.consensus_secret_key,
                connect_list: ConnectList::from_config(node_config.connect_list),
                address: node_config.listen_address,
            },
            service: ServiceConfig {
                service_public_key: node_config.service_public_key,
                service_secret_key: node_config.service_secret_key,
            },
            mempool: node_config.mempool,
            network: node_config.network,
            peer_discovery: peers,
        };

        let transactions = transactions.into_iter().collect::<Vec<_>>();
        let api_state = SharedNodeState::new(node_config.api.state_update_timeout as u64);
        let system_state = Box::new(DefaultSystemState(node_config.listen_address));
        let handler = TransactionsHandler {
            inner: Some(NodeHandler::new(
                blockchain,
                node_config.external_address,
                channel.node_sender(),
                system_state,
                config,
                api_state,
                None,
            )),
            txs_count: transactions.len(),
        };

        Self {
            handler,
            channel,
            transactions,
        }
    }

    fn run(self) -> io::Result<()> {
        // Creates node handler part.
        let tx_sender = self.channel.network_events.0.clone();
        let handler_part = HandlerPart {
            handler: self.handler,
            internal_rx: self.channel.internal_events.1,
            network_rx: self.channel.network_events.1,
            api_rx: self.channel.api_requests.1,
        };
        // Emulates transactions from the network.
        let socket_addr = handler_part.handler.inner().system_state.listen_address();
        let transactions = self.transactions
            .into_iter()
            .map(|raw| NetworkEvent::MessageReceived(socket_addr, raw))
            .collect::<Vec<_>>();
        let network_thread = thread::spawn(move || {
            Core::new()?
                .run(
                    tx_sender
                        .send_all(stream::iter_ok(transactions))
                        .map(drop)
                        .map_err(drop),
                )
                .map_err(|_| other_error("An error in the `Network` thread occurred"))
        });
        // Drops unused channels.
        drop(self.channel.api_requests.0);
        // Runs handler part.
        let mut core = Core::new()?;
        core.run(handler_part.run())
            .map_err(|_| other_error("An error in the `Handler` thread occurred"))?;
        network_thread.join().unwrap()
    }

    fn node_config() -> NodeConfig {
        let (consensus_public_key, consensus_secret_key) = crypto::gen_keypair();
        let (service_public_key, service_secret_key) = crypto::gen_keypair();

        let validator_keys = ValidatorKeys {
            consensus_key: consensus_public_key,
            service_key: service_public_key,
        };
        let genesis = GenesisConfig::new(vec![validator_keys].into_iter());
        let peer_address = "0.0.0.0:2000".parse().unwrap();

        NodeConfig {
            listen_address: peer_address,
            external_address: peer_address,
            service_public_key,
            service_secret_key,
            consensus_public_key,
            consensus_secret_key,
            genesis,
            network: Default::default(),
            connect_list: Default::default(),
            api: NodeApiConfig::default(),
            mempool: Default::default(),
            services_configs: Default::default(),
            database: Default::default(),
        }
    }
}

fn create_rocksdb(tempdir: &TempDir) -> Arc<dyn Database> {
    let options = DbOptions::default();
    let db = RocksDB::open(tempdir.path(), &options).unwrap();
    Arc::new(db)
}

fn bench_verify_transactions_simple(b: &mut Bencher, &size: &usize) {
    let transactions = gen_transactions(TRANSACTIONS_COUNT, size);
    b.iter(|| {
        for transaction in &transactions {
            transaction.verify();
        }
    })
}

fn bench_verify_transactions_event_loop(b: &mut Bencher, &size: &usize) {
    let transactions = gen_transactions(TRANSACTIONS_COUNT, size);
    b.iter_with_large_setup(
        || {
            let dir = TempDir::new("exonum").unwrap();
            let runner = TransactionsBenchmarkRunner::new(
                create_rocksdb(&dir),
                transactions
                    .iter()
                    .map(|transaction| transaction.raw().clone()),
            );
            (runner, dir)
        },
        |(runner, _dir)| {
            runner.run().unwrap();
        },
    )
}

pub fn bench_verify_transactions(c: &mut Criterion) {
    crypto::init();

    let parameters = (7..12).map(|i| pow(2, i)).collect::<Vec<_>>();
    c.bench(
        "transactions/simple",
        ParameterizedBenchmark::new("size", bench_verify_transactions_simple, parameters.clone())
            .throughput(|_| Throughput::Elements(TRANSACTIONS_COUNT as u32))
            .plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic))
            .sample_size(SAMPLE_SIZE),
    );
    c.bench(
        "transactions/event_loop",
        ParameterizedBenchmark::new(
            "size",
            bench_verify_transactions_event_loop,
            parameters.clone(),
        ).throughput(|_| Throughput::Elements(TRANSACTIONS_COUNT as u32))
            .plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic))
            .sample_size(SAMPLE_SIZE),
    );
}
