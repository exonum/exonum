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
//!
//! Message represents object received from p2p network.
//! There next flow between objects:
//! ```text
//! +---------+           +---------------+                 +------------+
//! | Vec<u8> |    ->     | SignedMessage |        ->       |  Protocol  |
//! +---------+  (verify) +---------------+  (deserialize)  +------------+
//!                 |                             |
//!                 V                             V
//!              (      message dropped if failed       )
//!
//! ```

#![allow(missing_docs, missing_debug_implementations)]
use std::borrow::Cow;
use std::cmp::PartialEq;
use std::fmt;
use std::ops::Deref;

use byteorder::{ByteOrder, LittleEndian};
use failure::Error;

use crypto::{hash, CryptoHash, Hash, PublicKey};
use encoding;
use storage::StorageValue;

use hex::{FromHex, ToHex};

pub(crate) use self::authorization::SignedMessage;
pub use self::helpers::BinaryForm;
pub(crate) use self::helpers::{BinaryFormSerialize, HexStringRepresentation};
pub use self::protocol::*;

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
/// This struct used to transfer transaction in network.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RawTransaction {
    service_id: u16,
    transaction_set: TransactionFromSet,
}

/// Concrete raw transaction payload inside `TransactionSet` linked with `message_id` in this set.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TransactionFromSet {
    message_id: u16,
    payload: Vec<u8>,
}

impl TransactionFromSet {
    pub fn from_raw_unchecked(message_id: u16, payload: Vec<u8>) -> Self {
        TransactionFromSet {
            message_id,
            payload,
        }
    }

    pub fn into_raw_parts(self) -> (u16, Vec<u8>) {
        (self.message_id, self.payload)
    }
}

impl RawTransaction {
    /// Creates new instance of RawTransaction.
    pub(in messages) fn new(
        service_id: u16,
        transaction_set: TransactionFromSet,
    ) -> RawTransaction {
        RawTransaction {
            service_id,
            transaction_set,
        }
    }
    /// Returns user defined data that should be used for deserialization.
    pub fn transaction_set(self) -> TransactionFromSet {
        self.transaction_set
    }
    /// Returns service_id specified for current transaction.
    pub fn service_id(&self) -> u16 {
        self.service_id
    }

    //    pub(crate) fn verify_transaction(
    //        buffer: Vec<u8>,
    //    ) -> Result<Message<RawTransaction>, ::failure::Error> {
    //        let signed = SignedMessage::verify_buffer(buffer)?;
    //        Protocol::deserialize(signed)?
    //            .try_into_transaction()
    //            .map_err(|_| format_err!("Couldn't parse RawTransaction."))
    //    }
}

impl BinaryForm for RawTransaction {
    fn serialize(&self) -> Result<Vec<u8>, encoding::Error> {
        let mut buffer = Vec::new();
        buffer.resize(2, 0);
        LittleEndian::write_u16(&mut buffer[0..2], self.service_id);
        let value = self.transaction_set.serialize()?;
        buffer.extend_from_slice(&value);
        Ok(buffer)
    }

    /// Converts serialized byte array into transaction.
    fn deserialize(buffer: &[u8]) -> Result<Self, encoding::Error> {
        let service_id = LittleEndian::read_u16(&buffer[0..2]);
        let transaction_set = TransactionFromSet::deserialize(&buffer[2..])?;
        Ok(RawTransaction {
            service_id,
            transaction_set,
        })
    }
}

impl BinaryForm for TransactionFromSet {
    fn serialize(&self) -> Result<Vec<u8>, encoding::Error> {
        let mut buffer = Vec::new();
        buffer.resize(2, 0);
        LittleEndian::write_u16(&mut buffer[0..2], self.message_id);
        buffer.extend_from_slice(&self.payload);
        Ok(buffer)
    }

    fn deserialize(buffer: &[u8]) -> Result<Self, encoding::Error> {
        let message_id = LittleEndian::read_u16(&buffer[0..2]);
        let payload = buffer[2..].to_vec();
        Ok(TransactionFromSet {
            message_id,
            payload,
        })
    }
}

/// Wrappers around pair of concrete message payload, and full message binary form.
/// Internally binary form saves message lossless,
/// this important for use in a scheme with
/// non-canonical serialization, for example with a `ProtoBuf`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Ord, PartialOrd)]
pub struct Message<T> {
    //TODO: inner T duplicate data in SignedMessage, we can use owning_ref,
    //if our serialisation format allows us
    payload: T,
    #[serde(with = "HexStringRepresentation")]
    message: SignedMessage,
}

impl<T: ProtocolMessage> Message<T> {
    /// Creates new instance of message.
    pub(in messages) fn new(payload: T, message: SignedMessage) -> Message<T> {
        Message { payload, message }
    }

    /// Returns hash of full message.
    pub fn hash(&self) -> Hash {
        hash(self.message.raw())
    }

    /// Returns serialized buffer.
    pub fn serialize(self) -> Vec<u8> {
        self.message.raw
    }

    /// Return link to inner.
    pub fn inner(&self) -> &T {
        &self.payload
    }

    /// Return link to signed message.
    pub(crate) fn signed_message(&self) -> &SignedMessage {
        &self.message
    }

    /// Returns public key of message creator.
    pub fn author(&self) -> PublicKey {
        self.message.author()
    }
}

impl fmt::Debug for TransactionFromSet {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Transaction")
            .field("message_id", &self.message_id)
            .field("payload_len", &self.payload.len())
            .finish()
    }
}

impl<T> ToHex for Message<T> {
    fn write_hex<W: ::std::fmt::Write>(&self, w: &mut W) -> ::std::fmt::Result {
        self.message.raw().write_hex(w)
    }

    fn write_hex_upper<W: ::std::fmt::Write>(&self, w: &mut W) -> ::std::fmt::Result {
        self.message.raw().write_hex_upper(w)
    }
}

impl<X: ProtocolMessage> FromHex for Message<X> {
    type Error = Error;

    fn from_hex<T: AsRef<[u8]>>(v: T) -> Result<Self, Error> {
        let bytes = Vec::<u8>::from_hex(v)?;
        let protocol = Protocol::deserialize(SignedMessage::verify_buffer(bytes)?)?;
        ProtocolMessage::try_from(protocol).map_err(|_| format_err!("Couldn't deserialize mesage."))
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
        let message = SignedMessage::unchecked_from_vec(value.into_owned());
        //TODO: Remove additional deserialization
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
