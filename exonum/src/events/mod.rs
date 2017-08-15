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

use tokio_core::reactor::Handle;

use std::io;
use std::net::SocketAddr;
use std::time::SystemTime;

use messages::{RawMessage};
use tokio::network::NetworkEvent;

// #[cfg(test)]
// mod tests;

pub type Milliseconds = u64;

pub trait EventHandler {
    type Timeout: Send;
    type ApplicationEvent: Send;

    fn handle_network_event(&mut self, event: NetworkEvent);
    fn handle_timeout(&mut self, timeout: Self::Timeout);
    fn handle_application_event(&mut self, event: Self::ApplicationEvent);
}

pub trait Channel: Sync + Send + Clone {
    type ApplicationEvent: Send;
    type Timeout: Send;

    fn get_time(&self) -> SystemTime;
    fn address(&self) -> SocketAddr;

    fn post_event(&self, handle: Handle, msg: Self::ApplicationEvent) -> Result<(), io::Error>;
    fn send_to(&mut self, handle: Handle, address: SocketAddr, message: RawMessage);
    fn add_timeout(&mut self, handle: Handle, timeout: Self::Timeout, time: SystemTime);
}
