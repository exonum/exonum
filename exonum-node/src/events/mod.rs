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

#![allow(missing_debug_implementations, missing_docs)]

pub use self::internal::InternalPart;
pub use self::network::{NetworkEvent, NetworkPart, NetworkRequest};

pub mod codec;
pub mod error;
pub mod internal;
pub mod network;
pub mod noise;

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

use crate::{events::error::LogError, messages::Message, ExternalMessage, NodeTimeout};

#[cfg(test)]
mod tests;

#[derive(Debug)]
pub struct SyncSender<T>(mpsc::Sender<T>);

impl<T: Send + 'static> SyncSender<T> {
    pub fn new(inner: mpsc::Sender<T>) -> Self {
        Self(inner)
    }

    // Since sandbox tests execute outside the `tokio` context, we detect this and block
    // the future if necessary.
    #[cfg(test)]
    pub fn send(&mut self, message: T) {
        use futures::executor::block_on;
        use tokio::runtime::Handle;

        if let Ok(handle) = Handle::try_current() {
            let mut sender = self.0.clone();
            handle.spawn(async move {
                sender.send(message).await.log_error();
            });
        } else {
            block_on(self.0.send(message)).log_error();
        }
    }

    // Outside tests, `send()` is always called from an async context.
    #[cfg(not(test))]
    pub fn send(&mut self, message: T) {
        let mut sender = self.0.clone();
        tokio::spawn(async move {
            sender.send(message).await.log_error();
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
        match self.0 {
            InternalEventInner::MessageVerified(_) => true,
            _ => false,
        }
    }

    pub(crate) fn timeout(timeout: NodeTimeout) -> Self {
        Self(InternalEventInner::Timeout(timeout))
    }
}

#[derive(Debug, PartialEq)]
pub(crate) enum InternalEventInner {
    /// Round update event.
    JumpToRound(Height, Round),
    /// Timeout event.
    Timeout(NodeTimeout),
    /// Message has been successfully verified.
    /// Message is boxed here so that enum variants have similar size.
    MessageVerified(Box<Message>),
}

#[derive(Debug)]
/// Asynchronous requests for internal actions.
pub enum InternalRequest {
    Timeout(TimeoutRequest),
    JumpToRound(Height, Round),
    /// Async request to verify a message in the thread pool.
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

#[derive(Debug)]
pub enum Event {
    Network(NetworkEvent),
    Transaction(Verified<AnyTx>),
    Api(ExternalMessage),
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

pub trait EventHandler {
    fn handle_event(&mut self, event: Event) -> EventOutcome;
}

#[derive(Debug)]
pub struct HandlerPart<H: EventHandler> {
    pub handler: H,
    pub internal_rx: mpsc::Receiver<InternalEvent>,
    pub network_rx: mpsc::Receiver<NetworkEvent>,
    pub transactions_rx: mpsc::Receiver<Verified<AnyTx>>,
    pub api_rx: mpsc::Receiver<ExternalMessage>,
}

impl<H: EventHandler + 'static + Send> HandlerPart<H> {
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

impl Into<InternalRequest> for TimeoutRequest {
    fn into(self) -> InternalRequest {
        InternalRequest::Timeout(self)
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

impl Into<Event> for NetworkEvent {
    fn into(self) -> Event {
        Event::Network(self)
    }
}

impl Into<Event> for NodeTimeout {
    fn into(self) -> Event {
        Event::Internal(InternalEvent::timeout(self))
    }
}

impl Into<Event> for Verified<AnyTx> {
    fn into(self) -> Event {
        Event::Transaction(self)
    }
}

impl Into<Event> for ExternalMessage {
    fn into(self) -> Event {
        Event::Api(self)
    }
}

impl Into<Event> for InternalEvent {
    fn into(self) -> Event {
        Event::Internal(self)
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
