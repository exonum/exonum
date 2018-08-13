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

use futures::{
    self, future::{self, Either}, sync::mpsc, Future, Sink, Stream,
};
use tokio_core::reactor::{Handle, Timeout};
use tokio_threadpool::Builder as ThreadPoolBuilder;

use std::{
    io, time::{Duration, SystemTime},
};

use super::{
    error::{into_other, other_error}, to_box, InternalEvent, InternalRequest, TimeoutRequest,
};

#[derive(Debug)]
pub struct InternalPart {
    pub internal_tx: mpsc::Sender<InternalEvent>,
    pub internal_requests_rx: mpsc::Receiver<InternalRequest>,
    pub thread_pool_size: Option<u8>,
}

impl InternalPart {
    pub fn run(self, handle: Handle) -> Box<dyn Future<Item = (), Error = io::Error>> {
        let thread_pool = if let Some(size) = self.thread_pool_size {
            ThreadPoolBuilder::new().pool_size(size.into()).build()
        } else {
            ThreadPoolBuilder::new().build()
        };
        let internal_tx = self.internal_tx.clone();

        let fut = self.internal_requests_rx
            .for_each(move |request| {
                match request {
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

                        handle.spawn(fut);
                    }

                    InternalRequest::JumpToRound(height, round) => {
                        let internal_tx = internal_tx.clone();
                        let f = futures::lazy(move || {
                            internal_tx
                                .send(InternalEvent::JumpToRound(height, round))
                                .map(drop)
                                .map_err(into_other)
                        }).map_err(|_| panic!("Can't execute jump to round"));

                        handle.spawn(f);
                    }

                    InternalRequest::Shutdown => {
                        let internal_tx = internal_tx.clone();
                        let f = futures::lazy(move || {
                            internal_tx
                                .send(InternalEvent::Shutdown)
                                .map(drop)
                                .map_err(into_other)
                        }).map_err(|_| panic!("Can't execute shutdown"));

                        handle.spawn(f);
                    }

                    InternalRequest::VerifyTx(tx) => {
                        let internal_tx = internal_tx.clone();

                        thread_pool
                            .sender()
                            .spawn(future::lazy(move || {
                                if tx.verify() {
                                    let evt = InternalEvent::TxVerified(tx.raw().clone());
                                    Either::A(
                                        internal_tx
                                            .send(evt)
                                            .map(drop)
                                            .map_err(|e| panic!("{:?}", e)),
                                    )
                                } else {
                                    Either::B(future::empty())
                                }
                            }))
                            .unwrap();
                    }
                }

                Ok(())
            })
            .map_err(|_| other_error("Can't handle timeout request"));
        to_box(fut)
    }
}
