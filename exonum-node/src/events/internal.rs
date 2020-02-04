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

use exonum::{merkledb::BinaryValue, messages::SignedMessage};
use futures::{
    future::{self, Either, Executor},
    sync::mpsc,
    Future, Sink, Stream,
};
use tokio_core::reactor::{Handle, Timeout};

use std::time::{Duration, SystemTime};

use super::{InternalEvent, InternalRequest, TimeoutRequest};
use crate::messages::{ExonumMessage, Message};

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

    fn verify_message(
        raw: Vec<u8>,
        internal_tx: mpsc::Sender<InternalEvent>,
    ) -> impl Future<Item = (), Error = ()> {
        future::lazy(|| {
            SignedMessage::from_bytes(raw.into())
                .and_then(SignedMessage::into_verified::<ExonumMessage>)
                .map(Message::from)
        })
        .map_err(drop)
        .and_then(|msg| {
            let event = future::ok(InternalEvent::message_verified(msg));
            Self::send_event(event, internal_tx)
        })
    }

    /// Represents a task that processes Internal Requests and produces Internal Events.
    /// `handle` is used to schedule additional tasks within this task.
    /// `verify_executor` is where transaction verification task is executed.
    pub fn run<E>(self, handle: Handle, verify_executor: E) -> impl Future<Item = (), Error = ()>
    where
        E: Executor<Box<dyn Future<Item = (), Error = ()> + Send>>,
    {
        let internal_tx = self.internal_tx;

        self.internal_requests_rx
            .map(move |request| {
                let event = match request {
                    InternalRequest::VerifyMessage(tx) => {
                        let fut = Self::verify_message(tx, internal_tx.clone());
                        verify_executor
                            .execute(Box::new(fut))
                            .expect("cannot schedule message verification");
                        return;
                    }

                    InternalRequest::Timeout(TimeoutRequest(time, timeout)) => {
                        let duration = time
                            .duration_since(SystemTime::now())
                            .unwrap_or_else(|_| Duration::from_millis(0));

                        let fut = Timeout::new(duration, &handle)
                            .expect("Unable to create timeout")
                            .map(|()| InternalEvent::timeout(timeout))
                            .map_err(|e| panic!("Cannot execute timeout: {:?}", e));

                        Either::A(fut)
                    }

                    InternalRequest::JumpToRound(height, round) => {
                        let event = InternalEvent::jump_to_round(height, round);
                        Either::B(future::ok(event))
                    }

                    InternalRequest::Shutdown => {
                        let event = InternalEvent::shutdown();
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
    use exonum::{
        crypto::{Hash, KeyPair, Signature},
        helpers::Height,
        messages::Verified,
    };
    use pretty_assertions::assert_eq;
    use tokio_core::reactor::Core;

    use std::thread;

    use super::*;
    use crate::messages::Status;

    fn verify_message(msg: Vec<u8>) -> Option<InternalEvent> {
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

        let request = InternalRequest::VerifyMessage(msg);
        internal_requests_tx.wait().send(request).unwrap();
        thread.join().unwrap()
    }

    fn get_signed_message() -> SignedMessage {
        let keys = KeyPair::random();
        Verified::from_value(
            Status::new(Height(0), Hash::zero(), 0),
            keys.public_key(),
            keys.secret_key(),
        )
        .into_raw()
    }

    #[test]
    fn verify_msg() {
        let tx = get_signed_message();

        let expected_event =
            InternalEvent::message_verified(Message::from_signed(tx.clone()).unwrap());
        let event = verify_message(tx.into_bytes());
        assert_eq!(event, Some(expected_event));
    }

    #[test]
    fn verify_incorrect_msg() {
        let mut tx = get_signed_message();
        tx.signature = Signature::zero();

        let event = verify_message(tx.into_bytes());
        assert_eq!(event, None);
    }
}
