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
use exonum_merkledb::BinaryValue;
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

/// Pre-commit for a block, essentially meaning that a validator node endorses the block.
/// The consensus algorithm ensures that once a Byzantine majority of validators has
/// endorsed a block, no other block at the same height may be endorsed at any point in the future.
/// Thus, such a block can be considered committed.
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

/// Subset of Exonum messages defined in the Exonum core.
///
/// This type is intentionally kept as minimal as possible to ensure compatibility
/// even if the consensus details change. Most of consensus messages are defined separately
/// in the `exonum-node` crate; they are not public.
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

impl TryFrom<SignedMessage> for CoreMessage {
    type Error = failure::Error;

    fn try_from(value: SignedMessage) -> Result<Self, Self::Error> {
        <Self as BinaryValue>::from_bytes(value.payload.into())
    }
}

#[doc(hidden)] // Library users should not define new message types.
#[macro_export]
macro_rules! impl_exonum_msg_try_from_signed {
    ( $base:ident => $( $name:ident ),* ) => {
        $(
            impl std::convert::TryFrom<$crate::messages::SignedMessage> for $name {
                type Error = failure::Error;

                fn try_from(value: $crate::messages::SignedMessage) -> Result<Self, Self::Error> {
                    <$base as $crate::merkledb::BinaryValue>::from_bytes(value.payload.into())
                        .and_then(Self::try_from)
                }
            }

            impl std::convert::TryFrom<&$crate::messages::SignedMessage> for $name {
                type Error = failure::Error;

                fn try_from(value: &$crate::messages::SignedMessage) -> Result<Self, Self::Error> {
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
