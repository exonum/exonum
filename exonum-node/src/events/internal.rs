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
use futures::{channel::mpsc, prelude::*};
use tokio::{task, time::delay_for};

use std::time::{Duration, SystemTime};

use super::{InternalEvent, InternalRequest, TimeoutRequest};
use crate::messages::{ExonumMessage, Message};

#[derive(Debug)]
pub struct InternalPart {
    pub internal_tx: mpsc::Sender<InternalEvent>,
    pub internal_requests_rx: mpsc::Receiver<InternalRequest>,
}

impl InternalPart {
    async fn send_event(mut sender: mpsc::Sender<InternalEvent>, event: InternalEvent) {
        // We don't make a fuss if the event receiver hanged up; this happens if the node
        // is being terminated.
        sender.send(event).await.ok();
    }

    async fn verify_message(raw: Vec<u8>, internal_tx: mpsc::Sender<InternalEvent>) {
        let task = task::spawn_blocking(|| {
            SignedMessage::from_bytes(raw.into())
                .and_then(SignedMessage::into_verified::<ExonumMessage>)
                .map(Message::from)
        });
        if let Ok(Ok(msg)) = task.await {
            let event = InternalEvent::message_verified(msg);
            Self::send_event(internal_tx, event).await;
        }
    }

    /// Represents a task that processes internal requests and produces internal events.
    /// `handle` is used to schedule additional tasks within this task.
    /// `verify_executor` is where transaction verification tasks are executed.
    pub async fn run(mut self) {
        while let Some(request) = self.internal_requests_rx.next().await {
            // Check if the receiver of internal events has hanged up. If so, terminate
            // event processing immediately since the generated events will be dropped anyway.
            if self.internal_tx.is_closed() {
                return;
            }
            let internal_tx = self.internal_tx.clone();

            match request {
                InternalRequest::VerifyMessage(raw) => {
                    tokio::spawn(Self::verify_message(raw, internal_tx));
                }

                InternalRequest::Timeout(TimeoutRequest(time, timeout)) => {
                    let duration = time
                        .duration_since(SystemTime::now())
                        .unwrap_or_else(|_| Duration::from_millis(0));

                    tokio::spawn(async move {
                        delay_for(duration).await;
                        Self::send_event(internal_tx, InternalEvent::timeout(timeout)).await;
                    });
                }

                InternalRequest::JumpToRound(height, round) => {
                    let event = InternalEvent::jump_to_round(height, round);
                    tokio::spawn(Self::send_event(internal_tx, event));
                }
            }
        }
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

    use super::*;
    use crate::messages::Status;

    async fn verify_message(msg: Vec<u8>) -> Option<InternalEvent> {
        let (internal_tx, mut internal_rx) = mpsc::channel(16);
        let (mut internal_requests_tx, internal_requests_rx) = mpsc::channel(16);

        let internal_part = InternalPart {
            internal_tx,
            internal_requests_rx,
        };
        tokio::spawn(internal_part.run());

        let request = InternalRequest::VerifyMessage(msg);
        internal_requests_tx.send(request).await.unwrap();
        drop(internal_requests_tx); // force the `internal_part` to stop
        internal_rx.next().await
    }

    fn get_signed_message() -> SignedMessage {
        let keys = KeyPair::random();
        Verified::from_value(
            Status::new(Height(0), Height(0), Hash::zero(), 0),
            keys.public_key(),
            keys.secret_key(),
        )
        .into_raw()
    }

    #[tokio::test]
    async fn verify_msg() {
        let tx = get_signed_message();
        let expected_event =
            InternalEvent::message_verified(Message::from_signed(tx.clone()).unwrap());
        let event = verify_message(tx.into_bytes()).await;
        assert_eq!(event, Some(expected_event));
    }

    #[tokio::test]
    async fn verify_incorrect_msg() {
        let mut tx = get_signed_message();
        tx.signature = Signature::zero();
        let event = verify_message(tx.into_bytes()).await;
        assert_eq!(event, None);
    }
}
