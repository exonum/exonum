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

pub mod codec;
pub mod error;
pub mod handler;
pub mod network;

use node::{NodeTimeout, ExternalMessage};

pub use self::handler::{NodeSender, NodeReceiver, NodeChannel};
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