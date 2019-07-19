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

//! Messages used in the Exonum consensus algorithm.
//!
//! Every message, unless stated otherwise, is checked by the same set of rules. The message is
//! ignored if it
//!     * is sent from a lower height than the current one
//!     * contains incorrect validator id
//!     * is signed with incorrect signature
//!
//! Specific nuances are described in each message documentation and typically consist of three
//! parts:
//!     * validation - additional checks before processing
//!     * processing - how message is processed and result of the processing
//!     * generation - in which cases message is generated

pub use crate::runtime::{AnyTx, MethodId, ServiceInstanceId, CallInfo};
pub use super::types::*;

use bit_vec::BitVec;
use chrono::{DateTime, Utc};
use exonum_merkledb::{BinaryValue, HashTag, ObjectHash};
use protobuf::Message as PbMessage;

use std::{borrow::Cow, fmt::Debug};

use crate::{
    blockchain,
    crypto::{Hash, PublicKey, SecretKey, PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH},
    helpers::{Height, Round, ValidatorId},
    proto::{
        self, schema::protocol::ExonumMessage_oneof_message as ExonumMessageEnum, ExonumMessage,
        ProtobufConvert,
    },
};

use super::{ServiceTransaction, Signed, SignedMessage};

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

/// Full message constraints list.
#[doc(hidden)]
pub trait ProtocolMessage: Debug + Clone + BinaryValue {
    /// Trying to convert `Message` to concrete message,
    /// if ok returns message `Signed<Self>` if fails, returns `Message` back.
    fn try_from(p: Message) -> Result<Signed<Self>, Message>;

    /// Create `Message` from concrete signed instance.
    fn into_protocol(this: Signed<Self>) -> Message;

    /// Convert message to protobuf message.
    fn as_exonum_message(&self) -> ExonumMessage;
}

/// Service messages.
#[derive(Debug, PartialEq)]
pub enum Service {
    /// Transaction message.
    AnyTx(Signed<AnyTx>),
    /// Connect message.
    Connect(Signed<Connect>),
    /// Status message.
    Status(Signed<Status>),
}

impl Service {
    fn signed_message(&self) -> &SignedMessage {
        match self {
            Service::AnyTx(ref msg) => msg.signed_message(),
            Service::Connect(ref msg) => msg.signed_message(),
            Service::Status(ref msg) => msg.signed_message(),
        }
    }
}

/// Consensus messages.
#[derive(Debug, PartialEq)]
pub enum Consensus {
    /// Precommit message.
    Precommit(Signed<Precommit>),
    /// Propose message.
    Propose(Signed<Propose>),
    /// Prevote message.
    Prevote(Signed<Prevote>),
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
    TransactionsResponse(Signed<TransactionsResponse>),
    /// Block response message.
    BlockResponse(Signed<BlockResponse>),
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
    ProposeRequest(Signed<ProposeRequest>),
    /// Transactions request message.
    TransactionsRequest(Signed<TransactionsRequest>),
    /// Prevotes request message.
    PrevotesRequest(Signed<PrevotesRequest>),
    /// Peers request message.
    PeersRequest(Signed<PeersRequest>),
    /// Block request message.
    BlockRequest(Signed<BlockRequest>),
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
    pub fn deserialize(signed: SignedMessage) -> Result<Self, failure::Error> {
        let mut exonum_message_pb = ExonumMessage::new();
        exonum_message_pb.merge_from_bytes(signed.exonum_message())?;
        let exonum_msg_enum = match exonum_message_pb.message {
            Some(msg) => msg,
            None => bail!("Message is empty"),
        };
        macro_rules! pb_enum_to_message {
            ($($pb_enum:path => $message_variant:path, $subclass_variant:path)+) =>
            {
                Ok(match exonum_msg_enum {
                    $(
                        $pb_enum(m) => $message_variant($subclass_variant(Signed::new(
                                        ProtobufConvert::from_pb(m)?,signed, ))),
                    )+
                })
            }
        }
        pb_enum_to_message!(
            ExonumMessageEnum::transaction => Message::Service, Service::AnyTx
            ExonumMessageEnum::connect => Message::Service, Service::Connect
            ExonumMessageEnum::status => Message::Service,Service::Status
            ExonumMessageEnum::precommit => Message::Consensus, Consensus::Precommit
            ExonumMessageEnum::propose => Message::Consensus, Consensus::Propose
            ExonumMessageEnum::prevote => Message::Consensus, Consensus::Prevote
            ExonumMessageEnum::txs_response => Message::Responses, Responses::TransactionsResponse
            ExonumMessageEnum::block_response => Message::Responses, Responses::BlockResponse
            ExonumMessageEnum::propose_req => Message::Requests, Requests::ProposeRequest
            ExonumMessageEnum::txs_req => Message::Requests, Requests::TransactionsRequest
            ExonumMessageEnum::prevotes_req => Message::Requests, Requests::PrevotesRequest
            ExonumMessageEnum::peers_req => Message::Requests, Requests::PeersRequest
            ExonumMessageEnum::block_req => Message::Requests, Requests::BlockRequest
        )
    }

    /// Creates new protocol message.
    /// Return concrete `Signed<T>`
    ///
    /// # Panics
    ///
    /// This method can panic on serialization failure.
    pub fn concrete<T: ProtocolMessage>(
        message: T,
        author: PublicKey,
        secret_key: &SecretKey,
    ) -> Signed<T> {
        let value = message
            .as_exonum_message()
            .write_to_bytes()
            .expect("Couldn't serialize data.");
        Signed::new(message, SignedMessage::new(&value, author, secret_key))
    }

    /// Creates a new raw transaction message.
    ///
    /// # Panics
    ///
    /// This method can panic on serialization failure.
    pub fn sign_transaction<T>(
        transaction: T,
        service_id: ServiceInstanceId,
        public_key: PublicKey,
        secret_key: &SecretKey,
    ) -> Signed<AnyTx>
    where
        T: Into<ServiceTransaction>,
    {
        let set: ServiceTransaction = transaction.into();
        let any_tx = AnyTx::new(service_id as u16, set.transaction_id, set.payload);
        Self::concrete(any_tx, public_key, secret_key)
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

// TODO simplify macro. [ECR-3222]

macro_rules! impl_protocol_message {
    ($message_variant:path, $subclass_variant:path, $message:ty, $exonum_msg_field:ident) => {
        impl ProtocolMessage for $message {
            fn try_from(p: Message) -> Result<Signed<Self>, Message> {
                match p {
                    $message_variant($subclass_variant(signed)) => Ok(signed),
                    _ => Err(p),
                }
            }

            fn into_protocol(this: Signed<Self>) -> Message {
                $message_variant($subclass_variant(this))
            }

            fn as_exonum_message(&self) -> ExonumMessage {
                let mut msg = ExonumMessage::new();
                msg.$exonum_msg_field(self.to_pb().into());
                msg
            }
        }
    };
}

impl_protocol_message!(Message::Service, Service::AnyTx, AnyTx, set_transaction);
impl_protocol_message!(Message::Service, Service::Connect, Connect, set_connect);
impl_protocol_message!(Message::Service, Service::Status, Status, set_status);
impl_protocol_message!(
    Message::Consensus,
    Consensus::Precommit,
    Precommit,
    set_precommit
);
impl_protocol_message!(Message::Consensus, Consensus::Propose, Propose, set_propose);
impl_protocol_message!(Message::Consensus, Consensus::Prevote, Prevote, set_prevote);
impl_protocol_message!(
    Message::Responses,
    Responses::TransactionsResponse,
    TransactionsResponse,
    set_txs_response
);
impl_protocol_message!(
    Message::Responses,
    Responses::BlockResponse,
    BlockResponse,
    set_block_response
);
impl_protocol_message!(
    Message::Requests,
    Requests::ProposeRequest,
    ProposeRequest,
    set_propose_req
);
impl_protocol_message!(
    Message::Requests,
    Requests::TransactionsRequest,
    TransactionsRequest,
    set_txs_req
);
impl_protocol_message!(
    Message::Requests,
    Requests::PrevotesRequest,
    PrevotesRequest,
    set_prevotes_req
);
impl_protocol_message!(
    Message::Requests,
    Requests::PeersRequest,
    PeersRequest,
    set_peers_req
);
impl_protocol_message!(
    Message::Requests,
    Requests::BlockRequest,
    BlockRequest,
    set_block_req
);

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

impl<T: ProtocolMessage> From<Signed<T>> for Message {
    fn from(other: Signed<T>) -> Self {
        ProtocolMessage::into_protocol(other)
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
