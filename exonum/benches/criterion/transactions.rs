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

const TRANSACTIONS_COUNT: usize = 1_000;
const SAMPLE_SIZE: usize = 20;

use criterion::{
    AxisScale, Bencher, Criterion, ParameterizedBenchmark, PlotConfiguration, Throughput,
};
use futures::{stream, sync::mpsc::Sender, sync::oneshot, Future, Sink};
use tokio_core::reactor::Core;

use std::{
    sync::{Arc, RwLock}, thread::{self, JoinHandle},
};

use exonum::events::InternalRequest;
use exonum::node::EventsPoolCapacity;
use exonum::node::ExternalMessage;
use exonum::{
    blockchain::{ExecutionResult, Transaction}, crypto::{self, PublicKey},
    events::{Event, EventHandler, HandlerPart, InternalEvent, InternalPart, NetworkEvent},
    messages::Message, node::NodeChannel, storage::Fork,
};
use tokio_threadpool::Builder as ThreadPoolBuilder;

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

    fn execute(&self, _: &mut Fork) -> ExecutionResult {
        Ok(())
    }
}

fn gen_transactions(count: usize, padding_len: usize) -> Vec<Box<Blank>> {
    let padding = vec![0; padding_len];

    (0..count)
        .map(|_| {
            let (p, s) = crypto::gen_keypair();
            Box::new(Blank::new(&p, &padding, &s))
        })
        .collect()
}

struct TransactionsHandler {
    txs_count: usize,
    expected_count: usize,
    finish_signal: Option<oneshot::Sender<()>>,
}

impl TransactionsHandler {
    fn new(expected_count: usize) -> (Self, oneshot::Receiver<()>) {
        let channel = oneshot::channel();

        let handler = TransactionsHandler {
            txs_count: 0,
            expected_count,
            finish_signal: Some(channel.0),
        };
        (handler, channel.1)
    }

    fn is_finished(&self) -> bool {
        self.finish_signal.is_none()
    }
}

impl EventHandler for TransactionsHandler {
    fn handle_event(&mut self, event: Event) {
        if let Event::Internal(InternalEvent::TxVerified(_)) = event {
            assert!(!self.is_finished(), "unexpected `TxVerified`");

            self.txs_count += 1;

            if self.txs_count == self.expected_count {
                self.finish_signal
                    .take()
                    .unwrap()
                    .send(())
                    .expect("cannot send finish signal");
            }
        }
    }
}

#[derive(Clone)]
struct TransactionsHandlerRef {
    // We need to reset the handler from the main thread and then access it from the
    // handler thread, hence the use of `Arc<RwLock<_>>`.
    inner: Arc<RwLock<TransactionsHandler>>,
}

impl TransactionsHandlerRef {
    fn new() -> Self {
        let (handler, _) = TransactionsHandler::new(0);
        TransactionsHandlerRef {
            inner: Arc::new(RwLock::new(handler)),
        }
    }

    fn reset(&self, expected_count: usize) -> oneshot::Receiver<()> {
        let (handler, finish_signal) = TransactionsHandler::new(expected_count);
        *self.inner.write().unwrap() = handler;
        finish_signal
    }
}

impl EventHandler for TransactionsHandlerRef {
    fn handle_event(&mut self, event: Event) {
        self.inner.write().unwrap().handle_event(event);
    }
}

struct TransactionVerifier {
    tx_sender: Option<Sender<InternalRequest>>,
    tx_handler: TransactionsHandlerRef,
    network_thread: JoinHandle<()>,
    handler_thread: JoinHandle<()>,
    api_sender: Option<Sender<ExternalMessage>>,
    network_sender: Option<Sender<NetworkEvent>>,
}

impl TransactionVerifier {
    fn new() -> Self {
        let channel = NodeChannel::new(&EventsPoolCapacity::default());
        let handler = TransactionsHandlerRef::new();

        let handler_part = HandlerPart {
            handler: handler.clone(),
            internal_rx: channel.internal_events.1,
            network_rx: channel.network_events.1,
            api_rx: channel.api_requests.1,
        };

        let handler_thread = thread::spawn(move || {
            let mut core = Core::new().unwrap();
            core.run(handler_part.run()).unwrap();
        });

        let internal_part = InternalPart {
            internal_tx: channel.internal_events.0,
            internal_requests_rx: channel.internal_requests.1,
        };

        let network_thread = thread::spawn(move || {
            let mut core = Core::new().unwrap();
            let handle = core.handle();

            let thread_pool = ThreadPoolBuilder::new().build();
            let verify_handle = thread_pool.sender().clone();

            core.run(internal_part.run(handle, verify_handle)).unwrap();
        });

        TransactionVerifier {
            handler_thread,
            network_thread,
            tx_sender: Some(channel.internal_requests.0.clone()),
            tx_handler: handler,
            api_sender: Some(channel.api_requests.0),
            network_sender: Some(channel.network_events.0),
        }
    }

    fn send_all<'a>(
        &self,
        transactions: &'a [Box<Blank>],
    ) -> impl Future<Item = (), Error = ()> + 'a {
        let tx_sender = self.tx_sender.as_ref().unwrap().clone();
        let finish_signal = self.tx_handler.reset(transactions.len());

        tx_sender
            .send_all(stream::iter_ok(
                transactions
                    .iter()
                    .cloned()
                    .map(|tx| InternalRequest::VerifyTx(tx as Box<dyn Transaction>)),
            ))
            .map(drop)
            .map_err(drop)
            .and_then(|()| finish_signal.map_err(drop))
    }

    /// Stops the transaction verifier.
    fn join(mut self) {
        self.tx_sender = None;
        self.network_thread.join().unwrap();

        self.api_sender = None;
        self.network_sender = None;
        self.handler_thread.join().unwrap();
    }
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

    let verifier = TransactionVerifier::new();
    let mut core = Core::new().unwrap();

    b.iter(|| {
        core.run(verifier.send_all(&transactions)).unwrap();
    });
    verifier.join();
}

pub fn bench_verify_transactions(c: &mut Criterion) {
    crypto::init();

    let parameters = (7..12).map(|i| 1 << i).collect::<Vec<_>>();

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
