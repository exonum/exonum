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

use std::net::SocketAddr;
use std::time::Duration;

use messages::{RawMessage};

// #[cfg(test)]
// mod tests;

pub type Milliseconds = u64;

pub trait Channel {
    type ApplicationEvent: Send;
    type Timeout: Send;

    fn send_to(&self, handle: Handle, address: SocketAddr, message: RawMessage);
    fn add_timeout(&self, handle: Handle, timeout: Self::Timeout, time: Duration);
}
