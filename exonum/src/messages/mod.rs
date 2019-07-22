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

use std::borrow::Cow;

use crate::{
    crypto::{Hash, PublicKey, SecretKey, PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH},
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
#[derive(Debug, Clone, PartialEq)]
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
#[derive(Debug, Clone, PartialEq)]
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
            Consensus::Precommit(ref msg) => msg.as_raw(),
            Consensus::Propose(ref msg) => msg.as_raw(),
            Consensus::Prevote(ref msg) => msg.as_raw(),
        }
    }
}

/// Response messages.
#[derive(Debug, Clone, PartialEq)]
pub enum Responses {
    /// Transactions response message.
    TransactionsResponse(Verified<TransactionsResponse>),
    /// Block response message.
    BlockResponse(Verified<BlockResponse>),
}

impl Responses {
    fn signed_message(&self) -> &SignedMessage {
        match self {
            Responses::TransactionsResponse(ref msg) => msg.as_raw(),
            Responses::BlockResponse(ref msg) => msg.as_raw(),
        }
    }
}

impl From<Verified<TransactionsResponse>> for Responses {
    fn from(msg: Verified<TransactionsResponse>) -> Self {
        Responses::TransactionsResponse(msg)
    }
}

impl From<Verified<BlockResponse>> for Responses {
    fn from(msg: Verified<BlockResponse>) -> Self {
        Responses::BlockResponse(msg)
    }
}

/// Request messages.
#[derive(Debug, Clone, PartialEq)]
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
            Requests::ProposeRequest(ref msg) => msg.as_raw(),
            Requests::TransactionsRequest(ref msg) => msg.as_raw(),
            Requests::PrevotesRequest(ref msg) => msg.as_raw(),
            Requests::PeersRequest(ref msg) => msg.as_raw(),
            Requests::BlockRequest(ref msg) => msg.as_raw(),
        }
    }
}

/// Exonum protocol messages.
#[derive(Debug, Clone, PartialEq)]
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
    /// Creates a new signed Exonum message from the given value.
    pub fn from_value<T: Into<ExonumMessage>>(
        message: T,
        author: PublicKey,
        secret_key: &SecretKey,
    ) -> Self {
        Self::from(Verified::from_value(message.into(), author, secret_key))
    }

    /// Deserialize message from signed message.
    pub fn from_signed(signed: SignedMessage) -> Result<Self, failure::Error> {
        signed.verify::<ExonumMessage>().map(From::from)
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
        SignedMessage::from_bytes(buffer.into()).and_then(Self::from_signed)
    }
}

macro_rules! impl_message_from_verified {
    ( $($concrete:ident: $category:ident),* ) => {
        $(
            impl From<Verified<$concrete>> for Message {
                fn from(msg: Verified<$concrete>) -> Self {
                    Message::$category($category::$concrete(msg))
                }
            }

            impl std::convert::TryFrom<Message> for Verified<$concrete> {
                type Error = failure::Error;

                fn try_from(msg: Message) -> Result<Self, Self::Error> {
                    if let Message::$category($category::$concrete(msg)) = msg {
                        Ok(msg)
                    } else {
                        Err(failure::format_err!(
                            "Given message is not a {}::{}",
                            stringify!($category),
                            stringify!($concrete)
                        ))
                    }
                }
            }
        )*

        impl From<Verified<ExonumMessage>> for Message {
            fn from(msg: Verified<ExonumMessage>) -> Self {
                let raw = msg.raw;
                match msg.inner {
                    $(
                        ExonumMessage::$concrete(inner) => {
                            let inner = Verified::<$concrete> { raw, inner };
                            Message::from(inner)
                        }
                    )*
                }
            }
        }
    };
}

impl_message_from_verified! {
    AnyTx: Service,
    Connect: Service,
    Status: Service,
    Precommit: Consensus,
    Prevote: Consensus,
    Propose: Consensus,
    BlockResponse: Responses,
    TransactionsResponse: Responses,
    BlockRequest: Requests,
    PeersRequest: Requests,
    PrevotesRequest: Requests,
    ProposeRequest: Requests,
    TransactionsRequest: Requests
}

impl Requests {
    /// Returns public key of the message recipient.
    pub fn to(&self) -> PublicKey {
        match *self {
            Requests::ProposeRequest(ref msg) => msg.payload().to,
            Requests::TransactionsRequest(ref msg) => msg.payload().to,
            Requests::PrevotesRequest(ref msg) => msg.payload().to,
            Requests::PeersRequest(ref msg) => msg.payload().to,
            Requests::BlockRequest(ref msg) => msg.payload().to,
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
            Consensus::Propose(ref msg) => msg.payload().validator(),
            Consensus::Prevote(ref msg) => msg.payload().validator(),
            Consensus::Precommit(ref msg) => msg.payload().validator(),
        }
    }

    /// Returns height of the message.
    pub fn height(&self) -> Height {
        match *self {
            Consensus::Propose(ref msg) => msg.payload().height(),
            Consensus::Prevote(ref msg) => msg.payload().height(),
            Consensus::Precommit(ref msg) => msg.payload().height(),
        }
    }

    /// Returns round of the message.
    pub fn round(&self) -> Round {
        match *self {
            Consensus::Propose(ref msg) => msg.payload().round(),
            Consensus::Prevote(ref msg) => msg.payload().round(),
            Consensus::Precommit(ref msg) => msg.payload().round(),
        }
    }
}

impl BinaryValue for Message {
    fn to_bytes(&self) -> Vec<u8> {
        self.signed_message().to_bytes()
    }

    fn from_bytes(value: Cow<[u8]>) -> Result<Self, failure::Error> {
        let message = SignedMessage::from_bytes(value)?;
        Message::from_signed(message)
    }
}

impl ObjectHash for Message {
    fn object_hash(&self) -> Hash {
        self.signed_message().object_hash()
    }
}
