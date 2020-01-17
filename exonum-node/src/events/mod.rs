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
use futures::{
    sink::Wait,
    sync::mpsc::{self, Sender},
    Async, Future, Poll, Stream,
};

use std::{cmp::Ordering, time::SystemTime};

use crate::{messages::Message, ExternalMessage, NodeTimeout};

#[cfg(test)]
mod tests;

pub type SyncSender<T> = Wait<Sender<T>>;

/// This kind of events is used to schedule execution in next event-loop ticks
/// Usable to make flat logic and remove recursions.
#[derive(Debug, PartialEq)]
pub struct InternalEvent(pub(crate) InternalEventInner);

impl InternalEvent {
    pub fn jump_to_round(height: Height, round: Round) -> Self {
        InternalEvent(InternalEventInner::JumpToRound(height, round))
    }

    pub fn message_verified(message: Message) -> Self {
        InternalEvent(InternalEventInner::MessageVerified(Box::new(message)))
    }

    pub fn is_message_verified(&self) -> bool {
        match self {
            InternalEvent(InternalEventInner::MessageVerified(_)) => true,
            _ => false,
        }
    }

    pub(crate) fn timeout(timeout: NodeTimeout) -> Self {
        InternalEvent(InternalEventInner::Timeout(timeout))
    }

    pub(crate) fn shutdown() -> Self {
        InternalEvent(InternalEventInner::Shutdown)
    }
}

#[derive(Debug, PartialEq)]
pub(crate) enum InternalEventInner {
    /// Round update event.
    JumpToRound(Height, Round),
    /// Timeout event.
    Timeout(NodeTimeout),
    /// Shutdown the node.
    Shutdown,
    /// Message has been successfully verified.
    /// Message is boxed here so that enum variants have similar size.
    MessageVerified(Box<Message>),
}

#[derive(Debug)]
/// Asynchronous requests for internal actions.
pub enum InternalRequest {
    Timeout(TimeoutRequest),
    JumpToRound(Height, Round),
    Shutdown,
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

pub trait EventHandler {
    fn handle_event(&mut self, event: Event);
}

#[derive(Debug)]
pub struct HandlerPart<H: EventHandler> {
    pub handler: H,
    pub internal_rx: mpsc::Receiver<InternalEvent>,
    pub network_rx: mpsc::Receiver<NetworkEvent>,
    pub transactions_rx: mpsc::Receiver<Verified<AnyTx>>,
    pub api_rx: mpsc::Receiver<ExternalMessage>,
}

impl<H: EventHandler + 'static> HandlerPart<H> {
    pub fn run(self) -> Box<dyn Future<Item = (), Error = ()>> {
        let mut handler = self.handler;
        let aggregator = EventsAggregator::new(
            self.internal_rx,
            self.network_rx,
            self.transactions_rx,
            self.api_rx,
        );
        let fut = aggregator.for_each(move |event| {
            handler.handle_event(event);
            Ok(())
        });

        to_box(fut)
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
    S1: Stream,
    S2: Stream,
    S3: Stream,
    S4: Stream,
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
    S1: Stream<Item = InternalEvent>,
    S2: Stream<Item = NetworkEvent, Error = S1::Error>,
    S3: Stream<Item = Verified<AnyTx>, Error = S1::Error>,
    S4: Stream<Item = ExternalMessage, Error = S1::Error>,
{
    type Item = Event;
    type Error = S1::Error;

    fn poll(&mut self) -> Poll<Option<Event>, Self::Error> {
        if self.done {
            Ok(Async::Ready(None))
        } else {
            match self.internal.poll()? {
                Async::Ready(None)
                | Async::Ready(Some(InternalEvent(InternalEventInner::Shutdown))) => {
                    self.done = true;
                    return Ok(Async::Ready(None));
                }
                Async::Ready(Some(item)) => {
                    return Ok(Async::Ready(Some(Event::Internal(item))));
                }
                Async::NotReady => {}
            };

            match self.network.poll()? {
                Async::Ready(Some(item)) => {
                    return Ok(Async::Ready(Some(Event::Network(item))));
                }
                Async::Ready(None) => {
                    self.done = true;
                    return Ok(Async::Ready(None));
                }
                Async::NotReady => {}
            };

            match self.transactions.poll()? {
                Async::Ready(Some(item)) => {
                    return Ok(Async::Ready(Some(Event::Transaction(item))));
                }
                Async::Ready(None) => {
                    self.done = true;
                    return Ok(Async::Ready(None));
                }
                Async::NotReady => {}
            };

            match self.api.poll()? {
                Async::Ready(None) => {
                    self.done = true;
                    return Ok(Async::Ready(None));
                }
                Async::Ready(Some(item)) => {
                    return Ok(Async::Ready(Some(Event::Api(item))));
                }
                Async::NotReady => {}
            };

            Ok(Async::NotReady)
        }
    }
}

fn to_box<F: Future + 'static>(f: F) -> Box<dyn Future<Item = (), Error = F::Error>> {
    Box::new(f.map(drop))
}
