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
    future::{self, Either, Executor}, sync::mpsc, Future, Sink, Stream,
};
use tokio_core::reactor::{Handle, Timeout};

use std::{
    io, time::{Duration, SystemTime},
};

use super::{
    error::{into_other, other_error}, InternalEvent, InternalRequest, TimeoutRequest,
};
use blockchain::Transaction;

#[derive(Debug)]
pub struct InternalPart {
    pub internal_tx: mpsc::Sender<InternalEvent>,
    pub internal_requests_rx: mpsc::Receiver<InternalRequest>,
}

impl InternalPart {
    fn verify_transaction(
        tx: Box<dyn Transaction>,
        internal_tx: mpsc::Sender<InternalEvent>,
    ) -> impl Future<Item = (), Error = ()> {
        future::lazy(move || {
            if tx.verify() {
                let send_event = internal_tx
                    .send(InternalEvent::TxVerified(tx.raw().clone()))
                    .map(drop)
                    .map_err(|e| {
                        panic!(
                            "error sending verified transaction \
                             to internal events pipe: {:?}",
                            e
                        );
                    });
                Either::A(send_event)
            } else {
                Either::B(future::ok(()))
            }
        })
    }

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
                    let fut = Self::verify_transaction(tx, internal_tx.clone());
                    // TODO: can errors be piped here?
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

#[cfg(test)]
mod tests {
    use tokio_core::reactor::Core;

    use std::thread;

    use super::*;
    use blockchain::ExecutionResult;
    use crypto::{gen_keypair, PublicKey, Signature};
    use messages::Message;
    use storage::Fork;

    transactions! {
        Transactions {
            const SERVICE_ID = 255;

            struct Tx {
                sender: &PublicKey,
                data: &str,
            }
        }
    }

    impl Transaction for Tx {
        fn verify(&self) -> bool {
            self.verify_signature(self.sender())
        }

        fn execute(&self, _: &mut Fork) -> ExecutionResult {
            Ok(())
        }
    }

    fn verify_transaction<T: Transaction>(tx: T) -> Option<InternalEvent> {
        let (internal_tx, internal_rx) = mpsc::channel(16);
        let (internal_requests_tx, internal_requests_rx) = mpsc::channel(16);

        let internal_part = InternalPart {
            internal_tx,
            internal_requests_rx,
        };

        let thread = thread::spawn(|| {
            let mut core = Core::new().unwrap();
            let handle = core.handle();
            let verifier = core.handle();

            let task = internal_part
                .run(handle, verifier)
                .map_err(drop)
                .and_then(|()| internal_rx.into_future().map_err(drop))
                .map(|(event, _)| event);
            core.run(task).unwrap()
        });

        let request = InternalRequest::VerifyTx(tx.into());
        internal_requests_tx.wait().send(request).unwrap();
        thread.join().unwrap()
    }

    #[test]
    fn verify_tx() {
        let (pk, sk) = gen_keypair();
        let tx = Tx::new(&pk, "foo", &sk);

        let expected_event = InternalEvent::TxVerified(tx.raw().clone());
        let event = verify_transaction(tx);
        assert_eq!(event, Some(expected_event));
    }

    #[test]
    fn verify_incorrect_tx() {
        let (pk, _) = gen_keypair();
        let tx = Tx::new_with_signature(&pk, "foo", &Signature::zero());

        let event = verify_transaction(tx);
        assert_eq!(event, None);
    }
}
