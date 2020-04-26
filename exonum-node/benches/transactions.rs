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

// Used in bench functions for convenience: we want to be able to pass these functions
// to `ParameterizedBenchmark::new()`.
#![allow(clippy::trivially_copy_pass_by_ref)]

const MESSAGES_COUNT: u64 = 1_000;
const SAMPLE_SIZE: usize = 20;

use criterion::{
    criterion_group, criterion_main, AxisScale, Bencher, Criterion, ParameterizedBenchmark,
    PlotConfiguration, Throughput,
};
use exonum::{
    crypto,
    merkledb::BinaryValue,
    messages::Verified,
    runtime::{AnyTx, CallInfo},
};
use futures::{
    channel::{mpsc, oneshot},
    stream, SinkExt,
};
use tokio::{runtime::Runtime, task::JoinHandle};

use std::sync::{Arc, RwLock};

use exonum_node::{
    EventsPoolCapacity, ExternalMessage, NodeChannel,
    _bench_types::{
        Event, EventHandler, EventOutcome, HandlerPart, InternalPart, InternalRequest,
        NetworkEvent, PeerMessage,
    },
};

struct MessagesHandler {
    txs_count: usize,
    expected_count: usize,
    finish_signal: Option<oneshot::Sender<()>>,
}

impl MessagesHandler {
    fn new(expected_count: usize) -> (Self, oneshot::Receiver<()>) {
        let channel = oneshot::channel();

        let handler = MessagesHandler {
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

impl EventHandler for MessagesHandler {
    fn handle_event(&mut self, event: Event) -> EventOutcome {
        if let Event::Internal(event) = event {
            if event.is_message_verified() {
                assert!(!self.is_finished(), "unexpected `MessageVerified`");

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
        EventOutcome::Ok
    }
}

fn gen_messages(count: u64, tx_size: usize) -> Vec<Vec<u8>> {
    let keypair = crypto::KeyPair::random();
    (0..count)
        .map(|_| {
            let tx = AnyTx::new(CallInfo::new(0, 0), vec![0; tx_size]).sign_with_keypair(&keypair);
            tx.into_bytes()
        })
        .collect()
}

#[derive(Clone)]
struct MessagesHandlerRef {
    // We need to reset the handler from the main thread and then access it from the
    // handler thread, hence the use of `Arc<RwLock<_>>`.
    inner: Arc<RwLock<MessagesHandler>>,
}

impl MessagesHandlerRef {
    fn new() -> Self {
        let (handler, _) = MessagesHandler::new(0);
        MessagesHandlerRef {
            inner: Arc::new(RwLock::new(handler)),
        }
    }

    fn reset(&self, expected_count: usize) -> oneshot::Receiver<()> {
        let (handler, finish_signal) = MessagesHandler::new(expected_count);
        *self.inner.write().unwrap() = handler;
        finish_signal
    }
}

impl EventHandler for MessagesHandlerRef {
    fn handle_event(&mut self, event: Event) -> EventOutcome {
        self.inner.write().unwrap().handle_event(event);
        EventOutcome::Ok
    }
}

struct MessageVerifier {
    tx_sender: Option<mpsc::Sender<InternalRequest>>,
    tx_handler: MessagesHandlerRef,
    network_task: JoinHandle<()>,
    handler_task: JoinHandle<()>,
    // We retain sender references in order to not shut down the event loop prematurely.
    external_tx_sender: Option<mpsc::Sender<Verified<AnyTx>>>,
    api_sender: Option<mpsc::Sender<ExternalMessage>>,
    network_sender: Option<mpsc::Sender<NetworkEvent>>,
}

impl MessageVerifier {
    fn new(rt: &Runtime) -> Self {
        let channel = NodeChannel::new(&EventsPoolCapacity::default());
        let handler = MessagesHandlerRef::new();

        let handler_part = HandlerPart {
            handler: handler.clone(),
            internal_rx: channel.internal_events.1,
            network_rx: channel.network_events.1,
            transactions_rx: channel.transactions.1,
            api_rx: channel.api_requests.1,
        };
        let handler_task = rt.spawn(handler_part.run());

        let internal_part = InternalPart {
            internal_tx: channel.internal_events.0,
            internal_requests_rx: channel.internal_requests.1,
        };
        let network_task = rt.spawn(internal_part.run());

        MessageVerifier {
            handler_task,
            network_task,
            tx_sender: Some(channel.internal_requests.0.clone()),
            tx_handler: handler,
            external_tx_sender: Some(channel.transactions.0),
            api_sender: Some(channel.api_requests.0),
            network_sender: Some(channel.network_events.0),
        }
    }

    async fn send_all(&mut self, messages: Vec<Vec<u8>>) {
        let finish_signal = self.tx_handler.reset(messages.len());
        let tx_sender = self.tx_sender.as_mut().unwrap();
        let messages = messages
            .into_iter()
            .map(|raw| Ok(InternalRequest::VerifyMessage(raw)));
        tx_sender
            .send_all(&mut stream::iter(messages))
            .await
            .unwrap();
        finish_signal.await.unwrap();
    }

    /// Stops the transaction verifier.
    async fn join(mut self) {
        self.tx_sender = None;
        self.network_task.await.unwrap();

        self.external_tx_sender = None;
        self.api_sender = None;
        self.network_sender = None;
        self.handler_task.await.unwrap();
    }
}

fn bench_verify_messages_simple(b: &mut Bencher<'_>, &size: &usize) {
    let messages = gen_messages(MESSAGES_COUNT, size);
    b.iter_with_setup(
        || messages.clone(),
        |messages| {
            for message in messages {
                PeerMessage::from_raw_buffer(message).unwrap();
            }
        },
    )
}

fn bench_verify_messages_event_loop(b: &mut Bencher<'_>, &size: &usize) {
    let messages = gen_messages(MESSAGES_COUNT, size);
    let mut rt = Runtime::new().unwrap();
    let mut verifier = MessageVerifier::new(&rt);

    b.iter_with_setup(
        || messages.clone(),
        |messages| {
            rt.block_on(verifier.send_all(messages));
        },
    );
    rt.block_on(verifier.join());
}

fn bench_verify_transactions(c: &mut Criterion) {
    crypto::init();

    let parameters = (7..12).map(|i| 1 << i).collect::<Vec<_>>();

    c.bench(
        "transactions/simple",
        ParameterizedBenchmark::new("size", bench_verify_messages_simple, parameters.clone())
            .throughput(|_| Throughput::Elements(MESSAGES_COUNT))
            .plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic))
            .sample_size(SAMPLE_SIZE),
    );
    c.bench(
        "transactions/event_loop",
        ParameterizedBenchmark::new("size", bench_verify_messages_event_loop, parameters)
            .throughput(|_| Throughput::Elements(MESSAGES_COUNT))
            .plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic))
            .sample_size(SAMPLE_SIZE),
    );
}

criterion_group!(benches, bench_verify_transactions);
criterion_main!(benches);
