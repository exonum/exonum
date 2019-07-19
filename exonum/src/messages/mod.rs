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

pub use self::{signed::Verified, types::*};
pub use exonum_merkledb::BinaryValue;

use exonum_merkledb::ObjectHash;
use serde::Deserialize;

use std::borrow::Cow;

use crate::{
    crypto::{Hash, PublicKey, PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH},
    helpers::{Height, Round, ValidatorId},
};

mod signed;
#[cfg(test)]
mod tests;
mod types;

/// Lower bound on the size of the correct `SignedMessage`.
/// Size of message fields + protobuf overhead.
#[doc(hidden)]
pub const SIGNED_MESSAGE_MIN_SIZE: usize = PUBLIC_KEY_LENGTH + SIGNATURE_LENGTH + 8;

#[doc(hidden)]
pub const TX_RES_EMPTY_SIZE: usize = SIGNED_MESSAGE_MIN_SIZE + PUBLIC_KEY_LENGTH + 8;

/// When we add transaction to TransactionResponse message we will add some overhead
/// to the message size due to protobuf.
/// This is higher bound on this overhead.
/// Tx response message size <= TX_RES_EMPTY_SIZE + (tx1 size + TX_RES_PB_OVERHEAD_PAYLOAD) +
///                             + (tx2 size + TX_RES_PB_OVERHEAD_PAYLOAD) + ...
#[doc(hidden)]
pub const TX_RES_PB_OVERHEAD_PAYLOAD: usize = 8;

/// Service messages.
#[derive(Debug, PartialEq)]
pub enum Service {
    /// Transaction message.
    AnyTx(Verified<AnyTx>),
    /// Connect message.
    Connect(Verified<Connect>),
    /// Status message.
    Status(Verified<Status>),
}

impl Service {
    fn signed_message(&self) -> &SignedMessage {
        match self {
            Service::AnyTx(ref msg) => msg.as_raw(),
            Service::Connect(ref msg) => msg.as_raw(),
            Service::Status(ref msg) => msg.as_raw(),
        }
    }
}

/// Consensus messages.
#[derive(Debug, PartialEq)]
pub enum Consensus {
    /// Precommit message.
    Precommit(Verified<Precommit>),
    /// Propose message.
    Propose(Verified<Propose>),
    /// Prevote message.
    Prevote(Verified<Prevote>),
}

impl Consensus {
    fn signed_message(&self) -> &SignedMessage {
        match self {
            Consensus::Precommit(ref msg) => msg.signed_message(),
            Consensus::Propose(ref msg) => msg.signed_message(),
            Consensus::Prevote(ref msg) => msg.signed_message(),
        }
    }
}

/// Response messages.
#[derive(Debug, PartialEq)]
pub enum Responses {
    /// Transactions response message.
    TransactionsResponse(Verified<TransactionsResponse>),
    /// Block response message.
    BlockResponse(Verified<BlockResponse>),
}

impl Responses {
    fn signed_message(&self) -> &SignedMessage {
        match self {
            Responses::TransactionsResponse(ref msg) => msg.signed_message(),
            Responses::BlockResponse(ref msg) => msg.signed_message(),
        }
    }
}

/// Request messages.
#[derive(Debug, PartialEq)]
pub enum Requests {
    /// Propose request message.
    ProposeRequest(Verified<ProposeRequest>),
    /// Transactions request message.
    TransactionsRequest(Verified<TransactionsRequest>),
    /// Prevotes request message.
    PrevotesRequest(Verified<PrevotesRequest>),
    /// Peers request message.
    PeersRequest(Verified<PeersRequest>),
    /// Block request message.
    BlockRequest(Verified<BlockRequest>),
}

impl Requests {
    fn signed_message(&self) -> &SignedMessage {
        match self {
            Requests::ProposeRequest(ref msg) => msg.signed_message(),
            Requests::TransactionsRequest(ref msg) => msg.signed_message(),
            Requests::PrevotesRequest(ref msg) => msg.signed_message(),
            Requests::PeersRequest(ref msg) => msg.signed_message(),
            Requests::BlockRequest(ref msg) => msg.signed_message(),
        }
    }
}

/// Exonum protocol messages.
#[derive(Debug, PartialEq)]
pub enum Message {
    /// Service messages.
    Service(Service),
    /// Consensus messages.
    Consensus(Consensus),
    /// Responses messages.
    Responses(Responses),
    /// Requests messages.
    Requests(Requests),
}

impl Message {
    /// Deserialize message from signed message.
    pub fn from_signed(signed: SignedMessage) -> Result<Self, failure::Error> {
        match signed.verify::<ExonumMessage>()? {
            // Service
            ExonumMessage::AnyTx(msg) => Message::Service(Service::AnyTx(msg)),
            ExonumMessage::Connect(msg) => Message::Service(Service::Connect(msg)),
            ExonumMessage::Status(msg) => Message::Service(Service::Status(msg)),
            // Consensus
            ExonumMessage::Precommit(msg) => Message::Consensus(Consensus::Precommit(msg)),
            ExonumMessage::Prevote(msg) => Message::Consensus(Consensus::Prevote(msg)),
            ExonumMessage::Propose(msg) => Message::Consensus(Consensus::Propose(msg)),
            // Responses
            ExonumMessage::BlockResponse(msg) => Message::Responses(Responses::BlockResponse(msg)),
            ExonumMessage::TransactionsResponse(msg) => {
                Message::Responses(Responses::TransactionsResponse(msg))
            }
            // Requests
            ExonumMessage::BlockRequest(msg) => Message::Requests(Requests::BlockRequest(msg)),
            ExonumMessage::PeersRequest(msg) => Message::Requests(Requests::PeersRequest(msg)),
            ExonumMessage::PrevotesRequest(msg) => {
                Message::Requests(Requests::PrevotesRequest(msg))
            }
            ExonumMessage::ProposeRequest(msg) => Message::Requests(Requests::ProposeRequest(msg)),
            ExonumMessage::TransactionsRequest(msg) => {
                Message::Requests(Requests::TransactionsRequest(msg))
            }
        }
    }

    /// Get inner SignedMessage.
    pub fn signed_message(&self) -> &SignedMessage {
        match self {
            Message::Service(ref msg) => msg.signed_message(),
            Message::Consensus(ref msg) => msg.signed_message(),
            Message::Requests(ref msg) => msg.signed_message(),
            Message::Responses(ref msg) => msg.signed_message(),
        }
    }

    /// Checks buffer and return instance of `Message`.
    pub fn from_raw_buffer(buffer: Vec<u8>) -> Result<Message, failure::Error> {
        let signed = SignedMessage::from_bytes(buffer.into())?;
        Self::deserialize(signed)
    }
}

impl Requests {
    /// Returns public key of the message recipient.
    pub fn to(&self) -> PublicKey {
        *match *self {
            Requests::ProposeRequest(ref msg) => msg.to(),
            Requests::TransactionsRequest(ref msg) => msg.to(),
            Requests::PrevotesRequest(ref msg) => msg.to(),
            Requests::PeersRequest(ref msg) => msg.to(),
            Requests::BlockRequest(ref msg) => msg.to(),
        }
    }

    /// Returns author public key of the message sender.
    pub fn author(&self) -> PublicKey {
        match *self {
            Requests::ProposeRequest(ref msg) => msg.author(),
            Requests::TransactionsRequest(ref msg) => msg.author(),
            Requests::PrevotesRequest(ref msg) => msg.author(),
            Requests::PeersRequest(ref msg) => msg.author(),
            Requests::BlockRequest(ref msg) => msg.author(),
        }
    }
}

impl Consensus {
    /// Returns author public key of the message sender.
    pub fn author(&self) -> PublicKey {
        match *self {
            Consensus::Propose(ref msg) => msg.author(),
            Consensus::Prevote(ref msg) => msg.author(),
            Consensus::Precommit(ref msg) => msg.author(),
        }
    }

    /// Returns validator id of the message sender.
    pub fn validator(&self) -> ValidatorId {
        match *self {
            Consensus::Propose(ref msg) => msg.validator(),
            Consensus::Prevote(ref msg) => msg.validator(),
            Consensus::Precommit(ref msg) => msg.validator(),
        }
    }

    /// Returns height of the message.
    pub fn height(&self) -> Height {
        match *self {
            Consensus::Propose(ref msg) => msg.height(),
            Consensus::Prevote(ref msg) => msg.height(),
            Consensus::Precommit(ref msg) => msg.height(),
        }
    }

    /// Returns round of the message.
    pub fn round(&self) -> Round {
        match *self {
            Consensus::Propose(ref msg) => msg.round(),
            Consensus::Prevote(ref msg) => msg.round(),
            Consensus::Precommit(ref msg) => msg.round(),
        }
    }
}

impl BinaryValue for Message {
    fn to_bytes(&self) -> Vec<u8> {
        self.signed_message().to_bytes()
    }

    fn from_bytes(value: Cow<[u8]>) -> Result<Self, failure::Error> {
        let message = SignedMessage::from_bytes(value)?;
        // TODO: Remove additional deserialization. [ECR-2315]
        Message::deserialize(message)
    }
}

impl ObjectHash for Message {
    fn object_hash(&self) -> Hash {
        self.signed_message().object_hash()
    }
}
