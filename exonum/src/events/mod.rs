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

#[cfg(any(test, feature = "network_benchmarks"))]
pub mod tests;
pub mod codec;
pub mod error;
pub mod network;

use futures::{Stream, Poll, Async};
use futures::stream::Fuse;

use std::time::SystemTime;

use node::{NodeTimeout, ExternalMessage};

pub use self::network::{NetworkEvent, NetworkRequest};

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

impl PartialOrd for TimeoutRequest {
    fn partial_cmp(&self, other: &Self) -> Option<::std::cmp::Ordering> {
        Some((&self.0, &self.1).cmp(&(&other.0, &other.1)).reverse())
    }
}

impl Ord for TimeoutRequest {
    fn cmp(&self, other: &Self) -> ::std::cmp::Ordering {
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

#[derive(Debug)]
pub struct EventsAggregator<S1, S2, S3>
where
    S1: Stream,
    S2: Stream,
    S3: Stream,
{
    timeout: Fuse<S1>,
    network: Fuse<S2>,
    api: Fuse<S3>,
}

impl<S1, S2, S3> EventsAggregator<S1, S2, S3>
where
    S1: Stream,
    S2: Stream,
    S3: Stream,
{
    pub fn new(timeout: S1, network: S2, api: S3) -> EventsAggregator<S1, S2, S3> {
        EventsAggregator {
            network: network.fuse(),
            timeout: timeout.fuse(),
            api: api.fuse(),
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
        let mut stream_finished = false;
        // Check timeout events
        match self.timeout.poll()? {
            Async::Ready(Some(item)) => return Ok(Async::Ready(Some(Event::Timeout(item)))),
            // Just finish stream
            Async::Ready(None) => stream_finished = true,
            Async::NotReady => {}
        };
        // Check network events
        match self.network.poll()? {
            Async::Ready(Some(item)) => return Ok(Async::Ready(Some(Event::Network(item)))),
            // Just finish stream
            Async::Ready(None) => stream_finished = true,
            Async::NotReady => {}
        };
        // Check api events
        match self.api.poll()? {
            Async::Ready(Some(item)) => return Ok(Async::Ready(Some(Event::Api(item)))),
            // Just finish stream
            Async::Ready(None) => stream_finished = true,
            Async::NotReady => {}
        };

        Ok(if stream_finished {
            Async::Ready(None)
        } else {
            Async::NotReady
        })
    }
}
