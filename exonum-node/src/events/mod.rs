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

pub use self::{
    internal::InternalPart,
    network::{ConnectedPeerAddr, NetworkEvent, NetworkPart, NetworkRequest},
    noise::HandshakeParams,
};

mod codec;
mod internal;
mod network;
mod noise;

use exonum::{
    helpers::{Height, Round},
    messages::{AnyTx, Verified},
};
use futures::{channel::mpsc, prelude::*};

use std::{
    cmp::Ordering,
    pin::Pin,
    task::{Context, Poll},
    time::SystemTime,
};

use crate::{messages::Message, ExternalMessage, NodeTimeout};

#[cfg(test)]
mod tests;

#[derive(Debug)]
pub struct SyncSender<T> {
    inner: mpsc::Sender<T>,
    message_kind: &'static str,
}

impl<T: Send + 'static> SyncSender<T> {
    const ERROR_MSG_SUFFIX: &'static str =
        "This is the expected behavior if the node is being shut down, \
         but could warrant an investigation otherwise.";

    pub fn new(inner: mpsc::Sender<T>, message_kind: &'static str) -> Self {
        Self {
            inner,
            message_kind,
        }
    }

    // Since sandbox tests execute outside the `tokio` context, we detect this and block
    // the future if necessary.
    #[cfg(test)]
    pub fn send(&mut self, message: T) {
        use futures::executor::block_on;
        use tokio::runtime::Handle;

        if let Ok(handle) = Handle::try_current() {
            let mut sender = self.inner.clone();
            let message_kind = self.message_kind;
            handle.spawn(async move {
                if sender.send(message).await.is_err() {
                    log::warn!(
                        "Cannot send a {}; processing has shut down. {}",
                        message_kind,
                        Self::ERROR_MSG_SUFFIX
                    );
                }
            });
        } else if block_on(self.inner.send(message)).is_err() {
            log::warn!(
                "Cannot send a {}; processing has shut down. {}",
                self.message_kind,
                Self::ERROR_MSG_SUFFIX
            );
        }
    }

    // Outside tests, `send()` is always called from an async context.
    #[cfg(not(test))]
    pub fn send(&mut self, message: T) {
        let mut sender = self.inner.clone();
        let message_kind = self.message_kind;
        tokio::spawn(async move {
            if sender.send(message).await.is_err() {
                log::warn!(
                    "Cannot send a {}; processing has shut down. {}",
                    message_kind,
                    Self::ERROR_MSG_SUFFIX
                );
            }
        });
    }
}

/// This kind of events is used to schedule execution in next event-loop ticks
/// Usable to make flat logic and remove recursions.
#[derive(Debug, PartialEq)]
pub struct InternalEvent(pub(crate) InternalEventInner);

impl InternalEvent {
    pub fn jump_to_round(height: Height, round: Round) -> Self {
        Self(InternalEventInner::JumpToRound(height, round))
    }

    pub fn message_verified(message: Message) -> Self {
        Self(InternalEventInner::MessageVerified(Box::new(message)))
    }

    pub fn is_message_verified(&self) -> bool {
        matches!(self.0, InternalEventInner::MessageVerified(_))
    }

    pub(crate) fn timeout(timeout: NodeTimeout) -> Self {
        Self(InternalEventInner::Timeout(timeout))
    }
}

#[derive(Debug, PartialEq)]
pub enum InternalEventInner {
    /// Round update event.
    JumpToRound(Height, Round),
    /// Timeout event.
    Timeout(NodeTimeout),
    /// Message has been successfully verified.
    /// Message is boxed here so that enum variants have similar size.
    MessageVerified(Box<Message>),
}

/// Asynchronous requests for internal actions.
#[derive(Debug)]
pub enum InternalRequest {
    /// Send an event on the specified timeout.
    Timeout(TimeoutRequest),
    /// Jump to the specified round.
    JumpToRound(Height, Round),
    /// Verify a message in the thread pool.
    VerifyMessage(Vec<u8>),
}

#[derive(Debug, PartialEq, Eq)]
pub struct TimeoutRequest(pub(crate) SystemTime, pub(crate) NodeTimeout);

impl TimeoutRequest {
    /// Gets the timeout of the request.
    pub fn time(&self) -> SystemTime {
        self.0
    }

    /// Converts this request into an event.
    pub fn event(self) -> Event {
        self.1.into()
    }
}

/// Events processed by the node.
#[derive(Debug)]
pub enum Event {
    /// Event related to network logic.
    Network(NetworkEvent),
    /// Event carrying a verified transaction.
    Transaction(Verified<AnyTx>),
    /// External control message (e.g., node shutdown).
    Api(ExternalMessage),
    /// Internally generated event.
    Internal(InternalEvent),
}

/// Denotes the execution status of an event.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EventOutcome {
    /// Event processing may continue normally.
    Ok,
    /// The event loop should terminate immediately.
    Terminated,
}

/// Trait encapsulating event processing logic.
pub trait EventHandler {
    /// Handles a single event.
    ///
    /// # Return value
    ///
    /// The implementation should return `EventOutcome::Terminated` iff the event loop
    /// should terminate right now.
    fn handle_event(&mut self, event: Event) -> EventOutcome;
}

/// Event handler for an Exonum node.
#[derive(Debug)]
pub struct HandlerPart<H: EventHandler> {
    /// Handler logic.
    pub handler: H,
    /// Receiver of internal events.
    pub internal_rx: mpsc::Receiver<InternalEvent>,
    /// Receiver of network events.
    pub network_rx: mpsc::Receiver<NetworkEvent>,
    /// Receiver of verified transactions.
    pub transactions_rx: mpsc::Receiver<Verified<AnyTx>>,
    /// Receiver of external control commands.
    pub api_rx: mpsc::Receiver<ExternalMessage>,
}

impl<H: EventHandler + 'static + Send> HandlerPart<H> {
    /// Processes events until `handler` signals that the event loop should be terminated.
    pub async fn run(self) {
        let mut handler = self.handler;
        let mut aggregator = EventsAggregator::new(
            self.internal_rx,
            self.network_rx,
            self.transactions_rx,
            self.api_rx,
        );

        while let Some(event) = aggregator.next().await {
            if handler.handle_event(event) == EventOutcome::Terminated {
                break;
            }
        }
    }
}

impl From<TimeoutRequest> for InternalRequest {
    fn from(request: TimeoutRequest) -> Self {
        Self::Timeout(request)
    }
}

impl PartialOrd for TimeoutRequest {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TimeoutRequest {
    fn cmp(&self, other: &Self) -> Ordering {
        (&self.0, &self.1).cmp(&(&other.0, &other.1)).reverse()
    }
}

impl From<NetworkEvent> for Event {
    fn from(event: NetworkEvent) -> Self {
        Self::Network(event)
    }
}

impl From<NodeTimeout> for Event {
    fn from(timeout: NodeTimeout) -> Self {
        Self::Internal(InternalEvent::timeout(timeout))
    }
}

impl From<Verified<AnyTx>> for Event {
    fn from(tx: Verified<AnyTx>) -> Self {
        Self::Transaction(tx)
    }
}

impl From<ExternalMessage> for Event {
    fn from(msg: ExternalMessage) -> Self {
        Self::Api(msg)
    }
}

impl From<InternalEvent> for Event {
    fn from(event: InternalEvent) -> Self {
        Self::Internal(event)
    }
}

/// Receives timeout, network and api events and invokes `handle_event` method of handler.
/// If one of these streams closes, the aggregator stream completes immediately.
#[derive(Debug)]
pub struct EventsAggregator<S1, S2, S3, S4> {
    done: bool,
    internal: S1,
    network: S2,
    transactions: S3,
    api: S4,
}

impl<S1, S2, S3, S4> EventsAggregator<S1, S2, S3, S4>
where
    S1: Stream + Unpin,
    S2: Stream + Unpin,
    S3: Stream + Unpin,
    S4: Stream + Unpin,
{
    pub fn new(internal: S1, network: S2, transactions: S3, api: S4) -> Self {
        Self {
            done: false,
            network,
            internal,
            transactions,
            api,
        }
    }
}

impl<S1, S2, S3, S4> Stream for EventsAggregator<S1, S2, S3, S4>
where
    S1: Stream<Item = InternalEvent> + Unpin,
    S2: Stream<Item = NetworkEvent> + Unpin,
    S3: Stream<Item = Verified<AnyTx>> + Unpin,
    S4: Stream<Item = ExternalMessage> + Unpin,
{
    type Item = Event;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.done {
            return Poll::Ready(None);
        }

        match self.internal.poll_next_unpin(cx) {
            Poll::Ready(None) => {
                self.done = true;
                return Poll::Ready(None);
            }
            Poll::Ready(Some(item)) => {
                return Poll::Ready(Some(Event::Internal(item)));
            }
            Poll::Pending => {}
        }

        match self.network.poll_next_unpin(cx) {
            Poll::Ready(Some(item)) => {
                return Poll::Ready(Some(Event::Network(item)));
            }
            Poll::Ready(None) => {
                self.done = true;
                return Poll::Ready(None);
            }
            Poll::Pending => {}
        }

        match self.transactions.poll_next_unpin(cx) {
            Poll::Ready(Some(item)) => {
                return Poll::Ready(Some(Event::Transaction(item)));
            }
            Poll::Ready(None) => {
                self.done = true;
                return Poll::Ready(None);
            }
            Poll::Pending => {}
        }

        match self.api.poll_next_unpin(cx) {
            Poll::Ready(None) => {
                self.done = true;
                return Poll::Ready(None);
            }
            Poll::Ready(Some(item)) => {
                return Poll::Ready(Some(Event::Api(item)));
            }
            Poll::Pending => {}
        }

        Poll::Pending
    }
}
