// Copyright 2019 The Exonum Team
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

//! Handling messages received from P2P node network.
//!
//! Every message passes through three phases:
//!
//!   * `Vec<u8>`: raw bytes as received from the network
//!   * `SignedMessage`: integrity and signature of the message has been verified
//!   * `Message`: the message has been completely parsed and has correct structure
//!
//! Graphical representation of the message processing flow:
//!
//! ```text
//! +---------+             +---------------+                  +----------+
//! | Vec<u8> |--(verify)-->| SignedMessage |--(deserialize)-->| Message |-->(handle)
//! +---------+     |       +---------------+        |         +----------+
//!                 |                                |
//!                 V                                V
//!              (drop)                           (drop)
//! ```

use failure::Error;
use hex::{FromHex, ToHex};
use serde::{
    de::{self, Deserialize, Deserializer},
    ser::{Serialize, Serializer},
};

use std::{borrow::Cow, fmt, ops::Deref};

use crate::crypto::{CryptoHash, Hash, PublicKey};
use crate::storage::StorageValue;

pub(crate) use self::{authorization::SignedMessage, helpers::HexStringRepresentation};
pub use self::{
    helpers::{to_hex_string, BinaryForm},
    protocol::*,
};

mod authorization;
mod helpers;
mod protocol;
#[cfg(test)]
mod tests;

/// Concrete raw transaction transaction inside `TransactionSet`.
/// This type is used inside `#[derive(TransactionSet)]`
/// to return raw transaction payload as part of service transaction set.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ServiceTransaction {
    transaction_id: u16,
    payload: Vec<u8>,
}

impl ServiceTransaction {
    /// Creates `ServiceTransaction` from unchecked raw data.
    pub fn from_raw_unchecked(transaction_id: u16, payload: Vec<u8>) -> Self {
        ServiceTransaction {
            transaction_id,
            payload,
        }
    }

    /// Converts `ServiceTransaction` back to raw data.
    pub fn into_raw_parts(self) -> (u16, Vec<u8>) {
        (self.transaction_id, self.payload)
    }
}

impl fmt::Debug for ServiceTransaction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Transaction")
            .field("message_id", &self.transaction_id)
            .field("payload_len", &self.payload.len())
            .finish()
    }
}

/// Wraps a `Payload` together with the corresponding `SignedMessage`.
///
/// Usually one wants to work with fully parsed messages (i.e., `Payload`). However, occasionally
/// we need to retransmit the message into the network or save its serialized form. We could
/// serialize the `Payload` back, but Protobuf does not have a canonical form so the resulting
/// payload may have different binary representation (thus invalidating the message signature).
///
/// So we use `Signed` to keep the original byte buffer around with the parsed `Payload`.
#[derive(Clone, Debug, PartialEq, Eq, Ord, PartialOrd)]
pub struct Signed<T> {
    // TODO: inner T duplicate data in SignedMessage, we can use owning_ref,
    // if our serialization format allows us (ECR-2315).
    payload: T,
    message: SignedMessage,
}

impl<T> Signed<T> {
    /// Creates a new instance of the message.
    pub(in crate::messages) fn new(payload: T, message: SignedMessage) -> Signed<T> {
        Signed { payload, message }
    }
}

impl<T: ProtocolMessage> CryptoHash for Signed<T> {
    fn hash(&self) -> Hash {
        self.message.hash()
    }
}

impl<T> Signed<T> {
    /// Returns public key of the message creator.
    pub fn author(&self) -> PublicKey {
        *self.message.key()
    }

    /// Returns reference to the payload.
    pub fn payload(&self) -> &T {
        &self.payload
    }

    /// Returns reference to the signed message.
    pub fn signed_message(&self) -> &SignedMessage {
        &self.message
    }

    /// Returns reference to the signed message.
    #[cfg(test)]
    pub fn signed_message_mut(&mut self) -> &mut SignedMessage {
        &mut self.message
    }
}

impl<T> ToHex for Signed<T> {
    fn write_hex<W: fmt::Write>(&self, w: &mut W) -> fmt::Result {
        self.message.write_hex(w)
    }

    fn write_hex_upper<W: fmt::Write>(&self, w: &mut W) -> fmt::Result {
        self.message.write_hex_upper(w)
    }
}

impl<X: ProtocolMessage> FromHex for Signed<X> {
    type Error = Error;

    fn from_hex<T: AsRef<[u8]>>(v: T) -> Result<Self, Error> {
        let bytes = Vec::<u8>::from_hex(v)?;
        let protocol = Message::deserialize(SignedMessage::decode(&bytes)?)?;
        ProtocolMessage::try_from(protocol)
            .map_err(|_| format_err!("Couldn't deserialize message."))
    }
}

impl<T: ProtocolMessage> AsRef<SignedMessage> for Signed<T> {
    fn as_ref(&self) -> &SignedMessage {
        &self.message
    }
}

impl<T: ProtocolMessage> AsRef<T> for Signed<T> {
    fn as_ref(&self) -> &T {
        &self.payload
    }
}

impl<T> From<Signed<T>> for SignedMessage {
    fn from(message: Signed<T>) -> Self {
        message.message
    }
}

impl<T> Deref for Signed<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.payload
    }
}

impl<T: ProtocolMessage> StorageValue for Signed<T> {
    fn into_bytes(self) -> Vec<u8> {
        self.encode().unwrap()
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        let message = SignedMessage::from_bytes(value);
        let msg = Message::deserialize(message).unwrap();
        T::try_from(msg).unwrap()
    }
}

impl<T: ProtocolMessage> BinaryForm for Signed<T> {
    fn encode(&self) -> Result<Vec<u8>, failure::Error> {
        self.message.encode()
    }

    fn decode(buffer: &[u8]) -> Result<Self, failure::Error> {
        let message = SignedMessage::decode(buffer)?;
        let msg = Message::deserialize(message)?;
        T::try_from(msg).map_err(|_| format_err!("Wrong message type"))
    }
}

impl<T> Serialize for Signed<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        HexStringRepresentation::serialize(&self.message, serializer)
    }
}

impl<'de, T> Deserialize<'de> for Signed<T>
where
    T: ProtocolMessage,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let signed_message: SignedMessage = HexStringRepresentation::deserialize(deserializer)?;
        Message::deserialize(signed_message)
            .map_err(|e| de::Error::custom(format!("Unable to deserialize signed message: {}", e)))
            .and_then(|msg| {
                T::try_from(msg).map_err(|e| {
                    de::Error::custom(format!(
                        "Unable to decode signed message into payload: {:?}",
                        e
                    ))
                })
            })
    }
}
