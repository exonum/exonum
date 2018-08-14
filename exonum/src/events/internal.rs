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
    future::{self, Either}, sync::mpsc, Future, Sink, Stream,
};
use tokio_core::reactor::{Handle, Timeout};

use std::{
    io, time::{Duration, SystemTime},
};

use super::{
    error::{into_other, other_error}, InternalEvent, InternalRequest, TimeoutRequest,
};
use futures::future::Executor;

#[derive(Debug)]
pub struct InternalPart {
    pub internal_tx: mpsc::Sender<InternalEvent>,
    pub internal_requests_rx: mpsc::Receiver<InternalRequest>,
}

impl InternalPart {
    pub fn run<E>(
        self,
        handle: Handle,
        verify_executor: E,
    ) -> impl Future<Item = (), Error = io::Error>
    where
        E: Executor<Box<dyn Future<Item = (), Error = ()> + Send>>,
    {
        let internal_tx = self.internal_tx.clone();

        self.internal_requests_rx
            .map_err(|()| other_error("error fetching internal requests"))
            .filter_map(move |request| match request {
                InternalRequest::VerifyTx(tx) => {
                    let internal_tx = internal_tx.clone();

                    let fut = future::lazy(move || {
                        if tx.verify() {
                            let fut = internal_tx
                                .send(InternalEvent::TxVerified(tx.raw().clone()))
                                .map(drop)
                                .map_err(|e| {
                                    panic!(
                                        "error sending verified transaction \
                                         to internal events pipe: {:?}",
                                        e
                                    )
                                });
                            Either::A(fut)
                        } else {
                            Either::B(future::ok(()))
                        }
                    });

                    // FIXME: can errors be piped here?
                    verify_executor
                        .execute(Box::new(fut))
                        .expect("cannot schedule transaction verification");
                    None
                }
                req => Some(req),
            })
            .and_then(move |request| match request {
                InternalRequest::Timeout(TimeoutRequest(time, timeout)) => {
                    let duration = time.duration_since(SystemTime::now())
                        .unwrap_or_else(|_| Duration::from_millis(0));
                    let fut = Timeout::new(duration, &handle)
                        .expect("Unable to create timeout")
                        .map(move |_| InternalEvent::Timeout(timeout));

                    Either::A(fut)
                }

                InternalRequest::JumpToRound(height, round) => {
                    let evt = InternalEvent::JumpToRound(height, round);
                    Either::B(future::ok(evt))
                }

                InternalRequest::Shutdown => {
                    let evt = InternalEvent::Shutdown;
                    Either::B(future::ok(evt))
                }

                InternalRequest::VerifyTx(..) => unreachable!(),
            })
            .forward(self.internal_tx.sink_map_err(into_other))
            .map(drop)
    }
}
