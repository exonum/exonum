// Copyright 2018 The Exonum Team
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

//! Consensus and other messages and related utilities.
use std::fmt::{self, Debug};
use std::ops::Deref;

use failure::Error;

pub use authorisation::SignedMessage;
pub use protocol::ProtocolMessage;

mod raw;
mod protocol;
mod authorisation;
mod helpers;

/// Version of the protocol. Different versions are incompatible.
pub const PROTOCOL_MAJOR_VERSION: u8 = 1;
// FIXME: Use config value.
pub const MAX_MESSAGE_SIZE: usize  = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawTransaction {
    service_id: u16,
    payload: Vec<u8>,
}

impl fmt::Debug for RawTransaction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Transaction")
            .field("service_id", &self.service_id)
            .field("payload_len", &self.payload.len())
            .finish()
    }
}

/// Wrappers around pair of serialized message, and its binary form
#[derive(Clone, Debug, Eq, PartialEq)]
pub (crate) struct Message<T: ProtocolMessage> {
    payload: T,
    message: SignedMessage,
}

impl<T: ProtocolMessage> Message<T> {
    fn deserialize(message: SignedMessage) -> Result<Self, Error>
    where T: BinarryForm
    {
        let payload = <T as BinaryForm>::deserialize(&message.payload)?;
        Ok(Message {
            payload,
            message
        })
    }

    fn map<U, F>(self, func: F) -> Result<Message<U>, Error>
        where U: ProtocolMessage,
              F: Fn(T)-> U
    {
        let payload = func(self.payload);
        let message = self.message;
        if payload != message {
            bail("Type {} is not a part of exonum protocol", payload)
        }
        Ok(Message {
            payload,
            message,
        })
    }
}

impl<T> Deref for Message<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.payload
    }
}
