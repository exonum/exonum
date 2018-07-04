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
use std::fmt;
use std::ops::Deref;

use failure::Error;

use crypto::{hash, Hash, PublicKey, SecretKey};

pub use self::authorisation::SignedMessage;
pub use self::helpers::BinaryForm;
pub use self::protocol::*;
pub(crate) use self::raw::UncheckedBuffer;

mod authorisation;
mod helpers;
mod protocol;
mod raw;
#[cfg(test)]
mod test;

/// Version of the protocol. Different versions are incompatible.
pub const PROTOCOL_MAJOR_VERSION: u8 = 1;

#[derive(Clone, PartialEq, Serialize, Deserialize)]
/// Transaction raw buffer.
/// This struct used to transfer transaction in network.
pub struct RawTransaction {
    service_id: u16,
    payload: Vec<u8>,
}

impl RawTransaction {
    /// Creates new instance of RawTransaction.
    pub(crate) fn new(service_id: u16, payload: Vec<u8>) -> RawTransaction {
        RawTransaction {
            service_id,
            payload,
        }
    }
    /// Returns user defined data that should be used for deserialization.
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
    /// Returns service_id specified for current transaction.
    pub fn service_id(&self) -> u16 {
        self.service_id
    }
    /// Verify buffer and return safe message wrapper for it.
    pub(crate) fn verify_transaction(
        buffer: UncheckedBuffer,
    ) -> Result<Message<RawTransaction>, ::failure::Error> {
        let signed = SignedMessage::verify_buffer(buffer)?;
        signed.into_message().map_into::<RawTransaction>()
    }
}

impl fmt::Debug for RawTransaction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Transaction")
            .field("service_id", &self.service_id)
            .field("payload_len", &self.payload.len())
            .finish()
    }
}

/// Wrappers around pair of deserialized message, and its binary form.
// TODO: Rewrite using owning_ref
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Message<T = Protocol>
where
    T: ProtocolMessage,
{
    payload: T,
    message: SignedMessage,
}

impl Message<Protocol> {
    /// Makes new instance of map from existing,
    /// trying convert internal payload to provided type U.
    pub fn map_into<U: ProtocolMessage>(self) -> Result<Message<U>, Error> {
        let (payload, message) = self.into_parts();
        Message::from_parts(U::try_from_protocol(payload)?, message)
    }
}

impl<T: ProtocolMessage> Message<T> {
    /// Creates new instance of message
    pub(crate) fn new(payload: T, author: PublicKey, secret_key: &SecretKey) -> Message<T> {
        let message =
            SignedMessage::new(payload.clone(), author, secret_key).expect("Serialization error");
        Message { payload, message }
    }

    /// Makes new instance of map from existing,
    /// trying convert internal payload to provided type,
    /// with helper converting method.
    pub fn map<U, F>(self, func: F) -> Result<Message<U>, Error>
    where
        U: ProtocolMessage,
        F: Fn(T) -> U,
    {
        let (payload, message) = self.into_parts();
        Message::from_parts(func(payload), message)
    }

    /// Split message into payload and signed raw parts
    pub(crate) fn into_parts(self) -> (T, SignedMessage) {
        (self.payload, self.message)
    }

    /// Trying to convert pair of payload and signed raw message into safe message wrapper.
    pub(crate) fn from_parts(payload: T, message: SignedMessage) -> Result<Message<T>, Error> {
        if payload != message.authorised_message.protocol {
            bail!("Type {:?} is not a part of exonum protocol", payload)
        }
        Ok(Message { payload, message })
    }
    /// Returns hahs of full message
    pub fn hash(&self) -> Hash {
        hash(&self.message.to_vec())
    }

    /// Returns hex representation of binary message form
    pub fn to_hex_string(&self) -> String {
        self.message.to_hex_string()
    }

    /// Downgrade `Message<T>` into root `Message<Protocol>`
    pub fn downgrade(self) -> Message<Protocol> {
        Message {
            payload: self.message.authorised_message.protocol.clone(),
            message: self.message,
        }
    }

    /// Returns public key of message creator.
    pub fn author(&self) -> &PublicKey {
        &self.message.authorised_message.author
    }
}

impl<T: ProtocolMessage> AsRef<SignedMessage> for Message<T> {
    fn as_ref(&self) -> &SignedMessage {
        &self.message
    }
}

impl<T: ProtocolMessage> AsRef<T> for Message<T> {
    fn as_ref(&self) -> &T {
        &self.payload
    }
}

impl<T: ProtocolMessage> Into<SignedMessage> for Message<T> {
    fn into(self) -> SignedMessage {
        self.message
    }
}

impl<T: ProtocolMessage> Deref for Message<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.payload
    }
}
