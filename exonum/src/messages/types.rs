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

pub use crate::runtime::AnyTx;

use bit_vec::BitVec;
use chrono::{DateTime, Utc};
use exonum_derive::{BinaryValue, ObjectHash};
use exonum_merkledb::{BinaryValue, HashTag};
use exonum_proto::ProtobufConvert;

use std::convert::TryFrom;

use crate::{
    blockchain::Block,
    crypto::{Hash, PublicKey, Signature},
    helpers::{Height, Round, ValidatorId},
    proto::schema::consensus,
};

/// Protobuf based container for any signed messages.
///
/// See module [documentation](index.html#examples) for examples.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "consensus::SignedMessage")]
pub struct SignedMessage {
    /// Payload of the message.
    pub payload: Vec<u8>,
    /// `PublicKey` of the author of the message.
    pub author: PublicKey,
    /// Digital signature over `payload` created with `SecretKey` of the author of the message.
    pub signature: Signature,
}

/// Connect to a node.
///
/// ### Validation
/// The message is ignored if its time is earlier than in the previous
/// `Connect` message received from the same peer.
///
/// ### Processing
/// Connect to the peer.
///
/// ### Generation
/// A node sends `Connect` message to all known addresses during
/// initialization. Additionally, the node responds by its own `Connect`
/// message after receiving `node::Event::Connected`.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug, ProtobufConvert)]
#[protobuf_convert(source = "consensus::Connect")]
pub struct Connect {
    /// The node's public address.
    pub host: String,
    /// Time when the message was created.
    pub time: DateTime<Utc>,
    /// String containing information about this node including Exonum, Rust and OS versions.
    pub user_agent: String,
}

impl Connect {
    /// Create new `Connect` message.
    pub fn new(
        host: impl Into<String>,
        time: DateTime<Utc>,
        user_agent: impl Into<String>,
    ) -> Self {
        Self {
            host: host.into(),
            time,
            user_agent: user_agent.into(),
        }
    }

    /// The node's address.
    pub fn pub_addr(&self) -> &str {
        &self.host
    }

    /// Time when the message was created.
    pub fn time(&self) -> DateTime<Utc> {
        self.time
    }

    /// String containing information about this node including Exonum, Rust and OS versions.
    pub fn user_agent(&self) -> &str {
        &self.user_agent
    }
}

/// Current node status.
///
/// ### Validation
/// The message is ignored if its signature is incorrect or its `height` is
/// lower than a node's height.
///
/// ### Processing
/// If the message's `height` number is bigger than a node's one, then
/// `BlockRequest` with current node's height is sent in reply.
///
/// ### Generation
/// `Status` message is broadcast regularly with the timeout controlled by
/// `blockchain::ConsensusConfig::status_timeout`. Also, it is broadcast
/// after accepting a new block.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug, ProtobufConvert)]
#[protobuf_convert(source = "consensus::Status")]
pub struct Status {
    /// The height to which the message is related.
    pub height: Height,
    /// Hash of the last committed block.
    pub last_hash: Hash,
    /// Transactions pool size.
    pub pool_size: u64,
}

impl Status {
    /// Create new `Status` message.
    pub fn new(height: Height, last_hash: Hash, pool_size: u64) -> Self {
        Self {
            height,
            last_hash,
            pool_size,
        }
    }

    /// The height to which the message is related.
    pub fn height(&self) -> Height {
        self.height
    }

    /// Hash of the last committed block.
    pub fn last_hash(&self) -> &Hash {
        &self.last_hash
    }

    /// Pool size.
    pub fn pool_size(&self) -> u64 {
        self.pool_size
    }
}

/// Proposal for a new block.
///
/// ### Validation
/// The message is ignored if it
///     * contains incorrect `prev_hash`
///     * is sent by non-leader
///     * contains already committed transactions
///     * is already known
///
/// ### Processing
/// If the message contains unknown transactions, then `TransactionsRequest`
/// is sent in reply.  Otherwise `Prevote` is broadcast.
///
/// ### Generation
/// A node broadcasts `Propose` if it is a leader and is not locked for a
/// different proposal. Also `Propose` can be sent as response to
/// `ProposeRequest`.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "consensus::Propose")]
pub struct Propose {
    /// The validator id.
    pub validator: ValidatorId,
    /// The height to which the message is related.
    pub height: Height,
    /// The round to which the message is related.
    pub round: Round,
    /// Hash of the previous block.
    pub prev_hash: Hash,
    /// The list of transactions to include in the next block.
    pub transactions: Vec<Hash>,
}

impl Propose {
    /// Create new `Propose` message.
    pub fn new(
        validator: ValidatorId,
        height: Height,
        round: Round,
        prev_hash: Hash,
        transactions: impl IntoIterator<Item = Hash>,
    ) -> Self {
        Self {
            validator,
            height,
            round,
            prev_hash,
            transactions: transactions.into_iter().collect(),
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
    /// Hash of the previous block.
    pub fn prev_hash(&self) -> &Hash {
        &self.prev_hash
    }
    /// The list of transactions to include in the next block.
    pub fn transactions(&self) -> &[Hash] {
        &self.transactions
    }
}

/// Pre-vote for a new block.
///
/// ### Validation
/// A node panics if it has already sent a different `Prevote` for the same
/// round.
///
/// ### Processing
/// Pre-vote is added to the list of known votes for the same proposal.  If
/// `locked_round` number from the message is bigger than in a node state,
/// then a node replies with `PrevotesRequest`.  If there are unknown
/// transactions in the propose specified by `propose_hash`,
/// `TransactionsRequest` is sent in reply.  Otherwise if all transactions
/// are known and there are +2/3 pre-votes, then a node is locked to that
/// proposal and `Precommit` is broadcast.
///
/// ### Generation
/// A node broadcasts `Prevote` in response to `Propose` when it has
/// received all the transactions.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug, ProtobufConvert)]
#[protobuf_convert(source = "consensus::Prevote")]
pub struct Prevote {
    /// The validator id.
    pub validator: ValidatorId,
    /// The height to which the message is related.
    pub height: Height,
    /// The round to which the message is related.
    pub round: Round,
    /// Hash of the corresponding `Propose`.
    pub propose_hash: Hash,
    /// Locked round.
    pub locked_round: Round,
}

impl Prevote {
    /// Create new `Prevote` message.
    pub fn new(
        validator: ValidatorId,
        height: Height,
        round: Round,
        propose_hash: Hash,
        locked_round: Round,
    ) -> Self {
        Self {
            validator,
            height,
            round,
            propose_hash,
            locked_round,
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
    /// Locked round.
    pub fn locked_round(&self) -> Round {
        self.locked_round
    }
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
#[protobuf_convert(source = "consensus::Precommit")]
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

/// Information about a block.
///
/// ### Validation
/// The message is ignored if
///     * its `to` field corresponds to a different node
///     * the `block`, `transaction` and `precommits` fields cannot be
///     parsed or verified
///
/// ### Processing
/// The block is added to the blockchain.
///
/// ### Generation
/// The message is sent as response to `BlockRequest`.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug, ProtobufConvert)]
#[protobuf_convert(source = "consensus::BlockResponse")]
pub struct BlockResponse {
    /// Public key of the recipient.
    pub to: PublicKey,
    /// Block header.
    pub block: Block,
    /// List of pre-commits.
    pub precommits: Vec<Vec<u8>>,
    /// List of the transaction hashes.
    pub transactions: Vec<Hash>,
}

impl BlockResponse {
    /// Create new `BlockResponse` message.
    pub fn new(
        to: PublicKey,
        block: Block,
        precommits: impl IntoIterator<Item = Vec<u8>>,
        transactions: impl IntoIterator<Item = Hash>,
    ) -> Self {
        Self {
            to,
            block,
            precommits: precommits.into_iter().collect(),
            transactions: transactions.into_iter().collect(),
        }
    }

    /// Public key of the recipient.
    pub fn to(&self) -> &PublicKey {
        &self.to
    }

    /// Block header.
    pub fn block(&self) -> &Block {
        &self.block
    }

    /// List of precommits.
    pub fn precommits(&self) -> &[Vec<u8>] {
        &self.precommits
    }

    /// List of the transaction hashes.
    pub fn transactions(&self) -> &[Hash] {
        &self.transactions
    }
}

/// Information about the transactions.
///
/// ### Validation
/// The message is ignored if
///     * its `to` field corresponds to a different node
///     * the `transactions` field cannot be parsed or verified
///
/// ### Processing
/// Returns information about the transactions requested by the hash.
///
/// ### Generation
/// The message is sent as response to `TransactionsRequest`.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug, ProtobufConvert)]
#[protobuf_convert(source = "consensus::TransactionsResponse")]
pub struct TransactionsResponse {
    /// Public key of the recipient.
    pub to: PublicKey,
    /// List of the transactions.
    pub transactions: Vec<Vec<u8>>,
}

impl TransactionsResponse {
    /// Create new `TransactionsResponse` message.
    pub fn new(to: PublicKey, transactions: impl IntoIterator<Item = Vec<u8>>) -> Self {
        Self {
            to,
            transactions: transactions.into_iter().collect(),
        }
    }

    /// Public key of the recipient.
    pub fn to(&self) -> &PublicKey {
        &self.to
    }

    /// List of the transactions.
    pub fn transactions(&self) -> &[Vec<u8>] {
        &self.transactions
    }
}

/// Request for the `Propose`.
///
/// ### Validation
/// The message is ignored if its `height` is not equal to the node's
/// height.
///
/// ### Processing
/// `Propose` is sent as the response.
///
/// ### Generation
/// A node can send `ProposeRequest` during `Precommit` and `Prevote`
/// handling.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug, ProtobufConvert)]
#[protobuf_convert(source = "consensus::ProposeRequest")]
pub struct ProposeRequest {
    /// Public key of the recipient.
    pub to: PublicKey,
    /// The height to which the message is related.
    pub height: Height,
    /// Hash of the `Propose`.
    pub propose_hash: Hash,
}

impl ProposeRequest {
    /// Create new `ProposeRequest`.
    pub fn new(to: PublicKey, height: Height, propose_hash: Hash) -> Self {
        Self {
            to,
            height,
            propose_hash,
        }
    }

    /// Public key of the recipient.
    pub fn to(&self) -> &PublicKey {
        &self.to
    }
    /// The height to which the message is related.
    pub fn height(&self) -> Height {
        self.height
    }
    /// Hash of the `Propose`.
    pub fn propose_hash(&self) -> &Hash {
        &self.propose_hash
    }
}

/// Request for transactions by hash.
///
/// ### Processing
/// Requested transactions are sent to the recipient.
///
/// ### Generation
/// This message can be sent during `Propose`, `Prevote` and `Precommit`
/// handling.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug, ProtobufConvert)]
#[protobuf_convert(source = "consensus::TransactionsRequest")]
pub struct TransactionsRequest {
    /// Public key of the recipient.
    pub to: PublicKey,
    /// The list of the transaction hashes.
    pub txs: Vec<Hash>,
}

impl TransactionsRequest {
    /// Create new `TransactionsRequest`.
    pub fn new(to: PublicKey, txs: impl IntoIterator<Item = Hash>) -> Self {
        Self {
            to,
            txs: txs.into_iter().collect(),
        }
    }

    /// Public key of the recipient.
    pub fn to(&self) -> &PublicKey {
        &self.to
    }

    /// The list of the transaction hashes.
    pub fn txs(&self) -> &[Hash] {
        &self.txs
    }
}

/// Request for pool transactions.
///
/// ### Processing
/// All transactions from mempool are sent to the recipient.
///
/// ### Generation
/// A node can send `PoolTransactionsRequest` during `Status` message
/// handling.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug, ProtobufConvert)]
#[protobuf_convert(source = "consensus::PoolTransactionsRequest")]
pub struct PoolTransactionsRequest {
    /// Public key of the recipient.
    pub to: PublicKey,
}

impl PoolTransactionsRequest {
    /// Create new `TransactionsRequest`.
    pub fn new(to: PublicKey) -> Self {
        Self { to }
    }
}

/// Request for pre-votes.
///
/// ### Validation
/// The message is ignored if its `height` is not equal to the node's
/// height.
///
/// ### Processing
/// The requested pre-votes are sent to the recipient.
///
/// ### Generation
/// This message can be sent during `Prevote` and `Precommit` handling.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug, ProtobufConvert)]
#[protobuf_convert(source = "consensus::PrevotesRequest")]
pub struct PrevotesRequest {
    /// Public key of the recipient.
    pub to: PublicKey,
    /// The height to which the message is related.
    pub height: Height,
    /// The round to which the message is related.
    pub round: Round,
    /// Hash of the `Propose`.
    pub propose_hash: Hash,
    /// The list of validators that send pre-votes.
    pub validators: BitVec,
}

impl PrevotesRequest {
    /// Create new `PrevotesRequest`.
    pub fn new(
        to: PublicKey,
        height: Height,
        round: Round,
        propose_hash: Hash,
        validators: BitVec,
    ) -> Self {
        Self {
            to,
            height,
            round,
            propose_hash,
            validators,
        }
    }

    /// Public key of the recipient.
    pub fn to(&self) -> &PublicKey {
        &self.to
    }
    /// The height to which the message is related.
    pub fn height(&self) -> Height {
        self.height
    }
    /// The round to which the message is related.
    pub fn round(&self) -> Round {
        self.round
    }
    /// Hash of the `Propose`.
    pub fn propose_hash(&self) -> &Hash {
        &self.propose_hash
    }
    /// The list of validators that send pre-votes.
    pub fn validators(&self) -> BitVec {
        self.validators.clone()
    }
}

/// Request connected peers from a node.
///
/// ### Validation
/// Request is considered valid if the sender of the message on the network
/// level corresponds to the `from` field.
///
/// ### Processing
/// Peer `Connect` messages are sent to the recipient.
///
/// ### Generation
/// `PeersRequest` message is sent regularly with the timeout controlled by
/// `blockchain::ConsensusConfig::peers_timeout`.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug, ProtobufConvert)]
#[protobuf_convert(source = "consensus::PeersRequest")]
pub struct PeersRequest {
    /// Public key of the recipient.
    pub to: PublicKey,
}

impl PeersRequest {
    /// Create new `PeersRequest`.
    pub fn new(to: PublicKey) -> Self {
        Self { to }
    }
    /// Public key of the recipient.
    pub fn to(&self) -> &PublicKey {
        &self.to
    }
}

/// Request for the block with the given `height`.
///
/// ### Validation
/// The message is ignored if its `height` is bigger than the node's one.
///
/// ### Processing
/// `BlockResponse` message is sent as the response.
///
/// ### Generation
/// This message can be sent during `Status` processing.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug, ProtobufConvert)]
#[protobuf_convert(source = "consensus::BlockRequest")]
pub struct BlockRequest {
    /// Public key of the recipient.
    pub to: PublicKey,
    /// The height to which the message is related.
    pub height: Height,
}

impl BlockRequest {
    /// Create new `BlockRequest`.
    pub fn new(to: PublicKey, height: Height) -> Self {
        Self { to, height }
    }
    /// Public key of the recipient.
    pub fn to(&self) -> &PublicKey {
        &self.to
    }
    /// The height to which the message is related.
    pub fn height(&self) -> Height {
        self.height
    }
}

impl BlockResponse {
    /// Verify Merkle root of transactions in the block.
    pub fn verify_tx_hash(&self) -> bool {
        self.block().tx_hash == HashTag::hash_list(self.transactions())
    }
}

/// This type describes all possible types of Exonum messages
/// which are used in p2p communications.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(
    source = "consensus::ExonumMessage",
    rename(case = "snake_case"),
    impl_from_trait
)]
#[allow(clippy::large_enum_variant)]
pub enum ExonumMessage {
    /// Exonum transaction.
    AnyTx(AnyTx),
    /// Handshake to other node.
    Connect(Connect),
    /// Status information of the other node.
    Status(Status),
    /// Consensus `Precommit` message.
    Precommit(Precommit),
    /// Consensus `Propose` message.
    Propose(Propose),
    /// Consensus `Prevote` message.
    Prevote(Prevote),
    /// Information about transactions, that sent as response to `TransactionsRequest`.
    TransactionsResponse(TransactionsResponse),
    /// Information about block, that sent as response to `BlockRequest`.
    BlockResponse(BlockResponse),
    /// Request of some propose which hash is known.
    ProposeRequest(ProposeRequest),
    /// Request of unknown transactions.
    TransactionsRequest(TransactionsRequest),
    /// Request of prevotes for some propose.
    PrevotesRequest(PrevotesRequest),
    /// Request of peer exchange.
    PeersRequest(PeersRequest),
    /// Request of some future block.
    BlockRequest(BlockRequest),
    /// Request of uncommitted transactions.
    PoolTransactionsRequest(PoolTransactionsRequest),
}

impl TryFrom<SignedMessage> for ExonumMessage {
    type Error = failure::Error;

    fn try_from(value: SignedMessage) -> Result<Self, Self::Error> {
        Self::from_bytes(value.payload.into())
    }
}

impl TryFrom<&SignedMessage> for ExonumMessage {
    type Error = failure::Error;

    fn try_from(value: &SignedMessage) -> Result<Self, Self::Error> {
        let bytes = std::borrow::Cow::Borrowed(value.payload.as_ref());
        Self::from_bytes(bytes)
    }
}

macro_rules! impl_exonum_msg_try_from_signed {
    ( $( $name:ident ),* ) => {
        $(
            impl TryFrom<SignedMessage> for $name {
                type Error = failure::Error;

                fn try_from(value: SignedMessage) -> Result<Self, Self::Error> {
                    ExonumMessage::try_from(value).and_then(Self::try_from)
                }
            }

            impl TryFrom<&SignedMessage> for $name {
                type Error = failure::Error;

                fn try_from(value: &SignedMessage) -> Result<Self, Self::Error> {
                    ExonumMessage::try_from(value).and_then(Self::try_from)
                }
            }
        )*
    }
}

impl_exonum_msg_try_from_signed! {
    AnyTx, Connect, Status, Precommit,
    Propose, Prevote, TransactionsResponse,
    BlockResponse, ProposeRequest, TransactionsRequest,
    PrevotesRequest, PeersRequest, BlockRequest, PoolTransactionsRequest
}
