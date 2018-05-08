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

use futures::{self, sync::mpsc, Future, Sink, Stream, future};
use tokio_core::reactor::{Handle, Timeout};
use tokio_threadpool::Builder as ThreadPoolBuilder;

use std::{
    io, time::{Duration, SystemTime},
};

use super::{
    error::{into_other, other_error}, to_box, InternalEvent, InternalRequest, TimeoutRequest,
};
use blockchain::Transaction;
use crypto::Hash;

#[derive(Debug)]
pub struct InternalPart {
    pub internal_tx: mpsc::Sender<InternalEvent>,
    pub internal_requests_rx: mpsc::Receiver<InternalRequest>,
}

impl InternalPart {
    pub fn run(self, handle: Handle) -> Box<dyn Future<Item = (), Error = io::Error>> {
        // Default number of threads = number of cores.
        let thread_pool = ThreadPoolBuilder::new().build();

        // Buffer in a channel wouldn't do anything except clutter the memory.
        let (pool_tx, pool_rx) = mpsc::channel::<Box<Transaction>>(0);
        let internal_tx = self.internal_tx.clone();
        thread_pool.spawn(futures::lazy(move || {
            pool_rx.for_each(move |tx| {
                if tx.verify() {
                    internal_tx
                        .clone()
                        .send(InternalEvent::TxVerified(tx))
                        .wait()
                        .map_err(|_| panic!("Cannot send tx to thread pool."));
                }
                Ok(())
            })
        }));

        let mut txs_in_verification = HashSet::<Hash>::new();

        let internal_tx = self.internal_tx.clone();
        let fut = self.internal_requests_rx
            .for_each(move |request| {
                let pool_tx = pool_tx.clone();
                let event = match request {
                    InternalRequest::Timeout(TimeoutRequest(time, timeout)) => {
                        let duration = time.duration_since(SystemTime::now())
                            .unwrap_or_else(|_| Duration::from_millis(0));
                        let internal_tx = internal_tx.clone();
                        let fut = Timeout::new(duration, &handle)
                            .expect("Unable to create timeout")
                            .and_then(move |_| {
                                internal_tx
                                    .clone()
                                    .send(InternalEvent::Timeout(timeout))
                                    .map(drop)
                                    .map_err(into_other)
                            })
                            .map_err(|_| panic!("Can't timeout"));
                        to_box(fut)
                    }
                    InternalRequest::JumpToRound(height, round) => {
                        let internal_tx = internal_tx.clone();
                        let f = futures::lazy(move || {
                            internal_tx
                                .send(InternalEvent::JumpToRound(height, round))
                                .map(drop)
                                .map_err(into_other)
                        }).map_err(|_| panic!("Can't execute jump to round"));
                        to_box(f)
                    }
                    InternalRequest::Shutdown => {
                        let internal_tx = internal_tx.clone();
                        let f = futures::lazy(move || {
                            internal_tx
                                .send(InternalEvent::Shutdown)
                                .map(drop)
                                .map_err(into_other)
                        }).map_err(|_| panic!("Can't execute shutdown"));
                        to_box(f)
                    }
                    InternalRequest::VerifyTx(tx) => {
                        if !txs_in_verification.insert(tx.raw().hash()) {
                            let f = future::ok::<(), ()>(());
                            to_box(f)
                        } else {
                            let f = futures::lazy(move || {
                                pool_tx.send(tx).map(drop).map_err(into_other)
                            }).map_err(|_| {
                                panic!("Can't send tx for verification to the thread pool")
                            });
                            to_box(f)
                        }
                    }
                };

                handle.spawn(event);
                Ok(())
            })
            .map_err(|_| other_error("Can't handle timeout request"));
        to_box(fut)
    }
}
