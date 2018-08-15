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

use std::time::{Duration, SystemTime};

use super::{InternalEvent, InternalRequest, TimeoutRequest};
use blockchain::Transaction;

#[derive(Debug)]
pub struct InternalPart {
    pub internal_tx: mpsc::Sender<InternalEvent>,
    pub internal_requests_rx: mpsc::Receiver<InternalRequest>,
}

impl InternalPart {
    // If the receiver for internal events is gone, we panic, as we cannot
    // continue our work (e.g., timely responding to timeouts).
    fn send_event(
        event: impl Future<Item = InternalEvent, Error = ()>,
        sender: mpsc::Sender<InternalEvent>,
    ) -> impl Future<Item = (), Error = ()> {
        event.and_then(|evt| {
            sender
                .send(evt)
                .map(drop)
                .map_err(|_| panic!("cannot send internal event"))
        })
    }

    fn verify_transaction(
        tx: Box<dyn Transaction>,
        internal_tx: mpsc::Sender<InternalEvent>,
    ) -> impl Future<Item = (), Error = ()> {
        future::lazy(move || {
            if tx.verify() {
                let event = future::ok(InternalEvent::TxVerified(tx.raw().clone()));
                Either::A(Self::send_event(event, internal_tx))
            } else {
                Either::B(future::ok(()))
            }
        })
    }

    pub fn run<E>(self, handle: Handle, verify_executor: E) -> impl Future<Item = (), Error = ()>
    where
        E: Executor<Box<dyn Future<Item = (), Error = ()> + Send>>,
    {
        let internal_tx = self.internal_tx;

        self.internal_requests_rx
            .map(move |request| {
                let event = match request {
                    InternalRequest::VerifyTx(tx) => {
                        let fut = Self::verify_transaction(tx, internal_tx.clone());
                        verify_executor
                            .execute(Box::new(fut))
                            .expect("cannot schedule transaction verification");
                        return;
                    }

                    InternalRequest::Timeout(TimeoutRequest(time, timeout)) => {
                        let duration = time.duration_since(SystemTime::now())
                            .unwrap_or_else(|_| Duration::from_millis(0));

                        let fut = Timeout::new(duration, &handle)
                            .expect("Unable to create timeout")
                            .map(|()| InternalEvent::Timeout(timeout))
                            .map_err(|e| panic!("Cannot execute timeout: {:?}", e));

                        Either::A(fut)
                    }

                    InternalRequest::JumpToRound(height, round) => {
                        let event = InternalEvent::JumpToRound(height, round);
                        Either::B(future::ok(event))
                    }

                    InternalRequest::Shutdown => {
                        let event = InternalEvent::Shutdown;
                        Either::B(future::ok(event))
                    }
                };

                let send_event = Self::send_event(event, internal_tx.clone());
                handle.spawn(send_event);
            })
            .for_each(Ok)
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
