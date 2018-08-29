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
use std::fmt;
use std::borrow::Cow;
use std::ops::Deref;

use failure::Error;

use blockchain::{Transaction, TransactionSet};
use storage::StorageValue;
use crypto::{hash, CryptoHash, Hash, PublicKey, SecretKey};
use encoding;

use hex::{FromHex, ToHex};

pub(crate) use self::authorization::SignedMessage;
pub use self::helpers::{BinaryForm, buffer_to_partial_enum, partial_enum_to_vec};
pub(crate) use self::helpers::{HexStringRepresentation, BinaryFormSerialize};
pub use self::protocol::*;

#[macro_use]
mod spec;
mod authorization;
mod helpers;
mod protocol;
//mod raw;
#[cfg(test)]
mod test;

/// Version of the protocol. Different versions are incompatible.
pub const PROTOCOL_MAJOR_VERSION: u8 = 1;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
/// Transaction raw buffer.
/// This struct used to transfer transaction in network.
pub struct RawTransaction {
    service_id: u16,
    payload: Vec<u8>,
}

pub struct TransactionFromSet<T> {
    message_id: u16,
    payload: T
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

    pub(crate) fn verify_transaction(
        buffer: Vec<u8>,
    ) -> Result<Message<RawTransaction>, ::failure::Error> {
        let signed = SignedMessage::verify_buffer(buffer)?;
        Protocol::deserialize(signed)?
                .try_into_transaction()
            .map_err(|_| format_err!("Couldn't parse RawTransaction."))
    }
}


impl BinaryForm for RawTransaction {
    fn serialize(&self) -> Result<Vec<u8>, encoding::Error> {
        unimplemented!()
    }

    /// Converts serialized byte array into transaction.
    fn deserialize(buffer: &[u8]) -> Result<Self, encoding::Error>{
        unimplemented!()
    }
}

impl<T> BinaryForm for TransactionFromSet<T>{
    fn serialize(&self) -> Result<Vec<u8>, encoding::Error> {
        unimplemented!()
    }

    /// Converts serialized byte array into transaction.
    fn deserialize(buffer: &[u8]) -> Result<Self, encoding::Error>{
        unimplemented!()
    }
}


/// Wrappers around pair of concrete message payload, and full message binary form.
/// Internally binary form saves message lossless,
/// this important for use in a scheme with
/// non-canonical serialization, for example with a `ProtoBuf`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Ord, PartialOrd)]
pub struct Message<T>
{
    //TODO: inner T duplicate data in SignedMessage, we can use owning_ref,
    //if our serialisation format allows us
    payload: T,
    message: SignedMessage,
}

impl<T: ProtocolMessage> Message<T> {
    /// Creates new instance of message
    pub(in messages) fn new(payload: T, message: SignedMessage) -> Message<T> {
        Message { payload, message }
    }

    /// Returns hash of full message
    pub fn hash(&self) -> Hash {
        hash(self.message.raw())
    }

//    /// Returns hex representation of binary message form
//    pub fn to_hex_string(&self) -> String {
//        self.message.to_hex_string()
//    }

    /// Return link to inner
    pub fn inner(&self) -> &T {
        &self.payload
    }

    /// Return link to signed message
    pub(crate) fn signed_message(&self) -> &SignedMessage {
        &self.message
    }

    /// Returns public key of message creator.
    pub fn author(&self) -> &PublicKey {
        &self.message.author()
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


impl<T> StorageValue for Message<T> {
    fn into_bytes(self) -> Vec<u8> {
        unimplemented!()
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        unimplemented!()
    }
}

impl<T> CryptoHash for Message<T> {
    fn hash(&self) -> Hash {
        unimplemented!()
    }
}
