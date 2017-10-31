// Copyright 2017 The Exonum Team
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

#[cfg(any(test, feature = "long_benchmarks"))]
pub mod tests;
pub mod codec;
pub mod error;
pub mod network;
pub mod timeouts;

use futures::{Future, Async, Poll, Stream};
use futures::sync::mpsc;

use std::time::SystemTime;
use std::cmp::Ordering;

use node::{ExternalMessage, NodeTimeout};

pub use self::network::{NetworkEvent, NetworkRequest, NetworkPart, NetworkConfiguration};
pub use self::timeouts::TimeoutsPart;


#[derive(Debug)]
pub enum Event {
    Network(NetworkEvent),
    Timeout(NodeTimeout),
    Api(ExternalMessage),
}

pub trait EventHandler {
    fn handle_event(&mut self, event: Event);
}

#[derive(Debug, PartialEq, Eq)]
pub struct TimeoutRequest(pub SystemTime, pub NodeTimeout);

#[derive(Debug)]
pub struct HandlerPart<H: EventHandler> {
    pub handler: H,
    pub timeout_rx: mpsc::Receiver<NodeTimeout>,
    pub network_rx: mpsc::Receiver<NetworkEvent>,
    pub api_rx: mpsc::Receiver<ExternalMessage>,
}

impl<H: EventHandler + 'static> HandlerPart<H> {
    pub fn run(self) -> Box<Future<Item = (), Error = ()>> {
        let mut handler = self.handler;

        let fut = EventsAggregator::new(self.timeout_rx, self.network_rx, self.api_rx)
            .for_each(move |event| {
                handler.handle_event(event);
                Ok(())
            });

        tobox(fut)
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
        Event::Timeout(self)
    }
}

impl Into<Event> for ExternalMessage {
    fn into(self) -> Event {
        Event::Api(self)
    }
}

/// Receives timeout, network and api events and invokes `handle_event` method of handler.
/// If one of these streams closes, the aggregator stream completes immediately.
#[derive(Debug)]
pub struct EventsAggregator<S1, S2, S3>
where
    S1: Stream,
    S2: Stream,
    S3: Stream,
{
    done: bool,
    timeout: S1,
    network: S2,
    api: S3,
}

impl<S1, S2, S3> EventsAggregator<S1, S2, S3>
where
    S1: Stream,
    S2: Stream,
    S3: Stream,
{
    pub fn new(timeout: S1, network: S2, api: S3) -> EventsAggregator<S1, S2, S3> {
        EventsAggregator {
            done: false,
            network: network,
            timeout: timeout,
            api: api,
        }
    }
}

impl<S1, S2, S3> Stream for EventsAggregator<S1, S2, S3>
where
    S1: Stream<Item = NodeTimeout>,
    S2: Stream<
        Item = NetworkEvent,
        Error = S1::Error,
    >,
    S3: Stream<
        Item = ExternalMessage,
        Error = S1::Error,
    >,
{
    type Item = Event;
    type Error = S1::Error;

    fn poll(&mut self) -> Poll<Option<Event>, Self::Error> {
        if self.done {
            Ok(Async::Ready(None))
        } else {
            match self.timeout.poll()? {
                Async::Ready(Some(item)) => {
                    return Ok(Async::Ready(Some(Event::Timeout(item))));
                }
                Async::Ready(None) => {
                    self.done = true;
                    return Ok(Async::Ready(None));
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
            match self.api.poll()? {
                Async::Ready(Some(item)) => {
                    return Ok(Async::Ready(Some(Event::Api(item))));
                }
                Async::Ready(None) => {
                    self.done = true;
                    return Ok(Async::Ready(None));
                }
                Async::NotReady => {}
            };

            Ok(Async::NotReady)
        }
    }
}


fn tobox<F: Future + 'static>(f: F) -> Box<Future<Item = (), Error = F::Error>> {
    Box::new(f.map(drop))
}
