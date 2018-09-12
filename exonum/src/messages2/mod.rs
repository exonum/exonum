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

//! Handling messages received from P2P node network.
//!
//! Every message passes through three phases:
//!
//!   * `Vec<u8>`: raw bytes as received from the network
//!   * `SignedMessage`: integrity and signature of the message has been verified
//!   * `Protocol`: the message has been completely parsed and has correct structure
//!
//! Graphical representation of the message processing flow:
//!
//! ```text
//! +---------+             +---------------+                  +----------+
//! | Vec<u8> |--(verify)-->| SignedMessage |--(deserialize)-->| Protocol |-->(handle)
//! +---------+     |       +---------------+        |         +----------+
//!                 |                                |
//!                 V                                V
//!              (drop)                           (drop)
//! ```

use byteorder::{ByteOrder, LittleEndian};
use failure::Error;
use hex::{FromHex, ToHex};

use std::{borrow::Cow, cmp::PartialEq, fmt, mem, ops::Deref};

use crypto::{hash, CryptoHash, Hash, PublicKey};
use encoding;
use storage::StorageValue;

pub(crate) use self::{authorization::SignedMessage, helpers::HexStringRepresentation};
pub use self::{helpers::BinaryForm, protocol::*};

#[macro_use]
mod compatibility;
mod authorization;
mod helpers;
mod protocol;
#[cfg(test)]
mod tests;

/// Version of the protocol. Different versions are incompatible.
pub const PROTOCOL_MAJOR_VERSION: u8 = 1;

/// Transaction raw buffer.
/// This struct is used to transfer transactions in network.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RawTransaction {
    service_id: u16,
    service_transaction: ServiceTransaction,
}

/// Concrete raw transaction transaction inside `TransactionSet`.
/// This type used inner inside `transactions!`
/// to return raw transaction payload as part of service transaction set.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
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

impl RawTransaction {
    /// Creates a new instance of RawTransaction.
    pub(in messages) fn new(
        service_id: u16,
        service_transaction: ServiceTransaction,
    ) -> RawTransaction {
        RawTransaction {
            service_id,
            service_transaction,
        }
    }

    /// Returns the user defined data that should be used for deserialization.
    pub fn service_transaction(self) -> ServiceTransaction {
        self.service_transaction
    }

    /// Returns `service_id` specified for current transaction.
    pub fn service_id(&self) -> u16 {
        self.service_id
    }
}

impl BinaryForm for RawTransaction {
    fn serialize(&self) -> Result<Vec<u8>, encoding::Error> {
        let mut buffer = vec![0; mem::size_of::<u16>()];
        LittleEndian::write_u16(&mut buffer[0..2], self.service_id);
        let value = self.service_transaction.serialize()?;
        buffer.extend_from_slice(&value);
        Ok(buffer)
    }

    /// Converts a serialized byte array into a transaction.
    fn deserialize(buffer: &[u8]) -> Result<Self, encoding::Error> {
        if buffer.len() < mem::size_of::<u16>() {
            Err("Buffer too short in RawTransaction deserialization.")?
        }
        let service_id = LittleEndian::read_u16(&buffer[0..2]);
        let service_transaction = ServiceTransaction::deserialize(&buffer[2..])?;
        Ok(RawTransaction {
            service_id,
            service_transaction,
        })
    }
}

impl BinaryForm for ServiceTransaction {
    fn serialize(&self) -> Result<Vec<u8>, encoding::Error> {
        let mut buffer = vec![0; mem::size_of::<u16>()];
        LittleEndian::write_u16(&mut buffer[0..2], self.transaction_id);
        buffer.extend_from_slice(&self.payload);
        Ok(buffer)
    }

    fn deserialize(buffer: &[u8]) -> Result<Self, encoding::Error> {
        if buffer.len() < mem::size_of::<u16>() {
            Err("Buffer too short in ServiceTransaction deserialization.")?
        }
        let transaction_id = LittleEndian::read_u16(&buffer[0..2]);
        let payload = buffer[2..].to_vec();
        Ok(ServiceTransaction {
            transaction_id,
            payload,
        })
    }
}

/// Wraps a `Payload` together with the corresponding `SignedMessage`.
///
/// Usually one wants to work with fully parsed messages (i.e., `Payload`). However, occasionally
/// we need to retransmit the message into the network or save its serialized form. We could
/// serialize the `Payload` back, but Protobuf does not have a canonical form so the resulting
/// payload may have different binary representation (thus invalidating the message signature).
///
/// So we use `Message` to keep the original byte buffer around with the parsed `Payload`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Ord, PartialOrd)]
pub struct Message<T> {
    // TODO: inner T duplicate data in SignedMessage, we can use owning_ref,
    //if our serialization format allows us (ECR-2315).
    payload: T,
    #[serde(with = "HexStringRepresentation")]
    message: SignedMessage,
}

impl<T: ProtocolMessage> Message<T> {
    /// Creates a new instance of the message.
    pub(in messages) fn new(payload: T, message: SignedMessage) -> Message<T> {
        Message { payload, message }
    }

    /// Returns hash of the full message.
    pub fn hash(&self) -> Hash {
        hash(self.message.raw())
    }

    /// Returns a serialized buffer.
    pub fn serialize(self) -> Vec<u8> {
        self.message.raw
    }

    /// Returns reference to the payload.
    pub fn payload(&self) -> &T {
        &self.payload
    }

    /// Returns reference to the signed message.
    pub(crate) fn signed_message(&self) -> &SignedMessage {
        &self.message
    }

    /// Returns public key of the message creator.
    pub fn author(&self) -> PublicKey {
        self.message.author()
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

impl<T> ToHex for Message<T> {
    fn write_hex<W: fmt::Write>(&self, w: &mut W) -> fmt::Result {
        self.message.raw().write_hex(w)
    }

    fn write_hex_upper<W: fmt::Write>(&self, w: &mut W) -> fmt::Result {
        self.message.raw().write_hex_upper(w)
    }
}

impl<X: ProtocolMessage> FromHex for Message<X> {
    type Error = Error;

    fn from_hex<T: AsRef<[u8]>>(v: T) -> Result<Self, Error> {
        let bytes = Vec::<u8>::from_hex(v)?;
        let protocol = Protocol::deserialize(SignedMessage::from_raw_buffer(bytes)?)?;
        ProtocolMessage::try_from(protocol)
            .map_err(|_| format_err!("Couldn't deserialize message."))
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

impl<T> From<Message<T>> for SignedMessage {
    fn from(message: Message<T>) -> Self {
        message.message
    }
}

impl<T: ProtocolMessage> Deref for Message<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.payload
    }
}

impl<T: ProtocolMessage> StorageValue for Message<T> {
    fn into_bytes(self) -> Vec<u8> {
        self.message.raw
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        let message = SignedMessage::from_vec_unchecked(value.into_owned());
        // TODO: Remove additional deserialization. [ECR-2315]
        let msg = Protocol::deserialize(message).unwrap();
        T::try_from(msg).unwrap()
    }
}

impl<T: ProtocolMessage> CryptoHash for Message<T> {
    fn hash(&self) -> Hash {
        self.hash()
    }
}

impl PartialEq<Message<RawTransaction>> for SignedMessage {
    fn eq(&self, other: &Message<RawTransaction>) -> bool {
        self.eq(other.signed_message())
    }
}
