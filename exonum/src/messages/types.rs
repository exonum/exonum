// Copyright 2020 The Exonum Team
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

pub use crate::runtime::AnyTx;

use chrono::{DateTime, Utc};
use exonum_derive::{BinaryValue, ObjectHash};
use exonum_proto::ProtobufConvert;

use std::convert::TryFrom;

use crate::{
    crypto::{Hash, PublicKey, Signature},
    helpers::{Height, Round, ValidatorId},
    proto::schema::messages,
};

/// Protobuf based container for any signed messages.
///
/// See module [documentation](index.html#examples) for examples.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "messages::SignedMessage")]
pub struct SignedMessage {
    /// Payload of the message.
    pub payload: Vec<u8>,
    /// `PublicKey` of the author of the message.
    pub author: PublicKey,
    /// Digital signature over `payload` created with `SecretKey` of the author of the message.
    pub signature: Signature,
}

/// Pre-commit for a proposal.
///
/// ### Validation
/// A node panics if it has already sent a different `Precommit` for the
/// same round.
///
/// ### Processing
/// Pre-commit is added to the list of known pre-commits.  If a proposal is
/// unknown to the node, `ProposeRequest` is sent in reply.  If `round`
/// number from the message is bigger than a node's "locked round", then a
/// node replies with `PrevotesRequest`.  If there are unknown transactions,
/// then `TransactionsRequest` is sent in reply.  If a validator receives
/// +2/3 precommits for the same proposal with the same `block_hash`, then
/// block is executed and `Status` is broadcast.
///
/// ### Generation
/// A node broadcasts `Precommit` in response to `Prevote` if there are +2/3
/// pre-votes and no unknown transactions.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert)]
#[protobuf_convert(source = "messages::Precommit")]
pub struct Precommit {
    /// The validator id.
    pub validator: ValidatorId,
    /// The height to which the message is related.
    pub height: Height,
    /// The round to which the message is related.
    pub round: Round,
    /// Hash of the corresponding `Propose`.
    pub propose_hash: Hash,
    /// Hash of the new block.
    pub block_hash: Hash,
    /// Time of the `Precommit`.
    pub time: DateTime<Utc>,
}

impl Precommit {
    /// Create new `Precommit` message.
    pub fn new(
        validator: ValidatorId,
        height: Height,
        round: Round,
        propose_hash: Hash,
        block_hash: Hash,
        time: DateTime<Utc>,
    ) -> Self {
        Self {
            validator,
            height,
            round,
            propose_hash,
            block_hash,
            time,
        }
    }
    /// The validator id.
    pub fn validator(&self) -> ValidatorId {
        self.validator
    }
    /// The height to which the message is related.
    pub fn height(&self) -> Height {
        self.height
    }
    /// The round to which the message is related.
    pub fn round(&self) -> Round {
        self.round
    }
    /// Hash of the corresponding `Propose`.
    pub fn propose_hash(&self) -> &Hash {
        &self.propose_hash
    }
    /// Hash of the new block.
    pub fn block_hash(&self) -> &Hash {
        &self.block_hash
    }
    /// Time of the `Precommit`.
    pub fn time(&self) -> DateTime<Utc> {
        self.time
    }
}

/// This type describes a subset of Exonum messages defined in the Exonum core.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(
    source = "messages::CoreMessage",
    rename(case = "snake_case"),
    impl_from_trait
)]
pub enum CoreMessage {
    /// Transaction message.
    AnyTx(AnyTx),
    /// Precommit message.
    Precommit(Precommit),
}

#[doc(hidden)] // Library users should not define new message types.
#[macro_export]
macro_rules! impl_exonum_msg_try_from_signed {
    ( $base:ident => $( $name:ident ),* ) => {
        $(
            impl TryFrom<$crate::messages::SignedMessage> for $name {
                type Error = failure::Error;

                fn try_from(value: SignedMessage) -> Result<Self, Self::Error> {
                    <$base as $crate::merkledb::BinaryValue>::from_bytes(value.payload.into())
                        .and_then(Self::try_from)
                }
            }

            impl TryFrom<&$crate::messages::SignedMessage> for $name {
                type Error = failure::Error;

                fn try_from(value: &SignedMessage) -> Result<Self, Self::Error> {
                    let bytes = std::borrow::Cow::Borrowed(value.payload.as_slice());
                    <$base as $crate::merkledb::BinaryValue>::from_bytes(bytes)
                        .and_then(Self::try_from)
                }
            }

            impl $crate::messages::IntoMessage for $name {
                type Container = $base;
            }
        )*
    }
}

impl_exonum_msg_try_from_signed!(CoreMessage => AnyTx, Precommit);
