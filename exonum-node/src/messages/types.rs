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

use bit_vec::BitVec;
use chrono::{DateTime, Utc};
use exonum::{
    blockchain::Block,
    crypto::{Hash, PublicKey},
    helpers::{Height, Round, ValidatorId},
    impl_exonum_msg_try_from_signed,
    merkledb::{BinaryValue, HashTag},
    messages::{AnyTx, Precommit, SignedMessage},
};
use exonum_derive::{BinaryValue, ObjectHash};
use exonum_proto::ProtobufConvert;

use std::convert::TryFrom;

use crate::proto::consensus;

/// Connect to a node.
///
/// ### Validation
///
/// The message is ignored if its time is earlier than in the previous
/// `Connect` message received from the same peer.
///
/// ### Processing
///
/// Connect to the peer.
///
/// ### Generation
///
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
///
/// The message is ignored if its signature is incorrect or its `epoch` / `blockchain_height` is
/// lower than the corresponding characteristics of the receiver.
///
/// ### Processing
///
/// - If `blockchain_height` is greater or equal to the current blockchain height of the node,
///   then `BlockRequest` with the current height is sent in reply.
/// - Otherwise, if `epoch` is greater or equal to the current epoch of the node,
///   then `BlockRequest` with the current height and the current epoch is sent in reply.
///
/// ### Generation
///
/// `Status` message is broadcast regularly with the timeout controlled by
/// `ConsensusConfig::status_timeout`. Also, it is broadcast after accepting a new block.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug, ProtobufConvert)]
#[protobuf_convert(source = "consensus::Status")]
pub struct Status {
    /// The epoch to which the message is related.
    pub epoch: Height,
    /// Current height of the blockchain.
    pub blockchain_height: Height,
    /// Hash of the last committed block.
    pub last_hash: Hash,
    /// Transactions pool size.
    pub pool_size: u64,
}

impl Status {
    /// Create new `Status` message.
    pub fn new(epoch: Height, blockchain_height: Height, last_hash: Hash, pool_size: u64) -> Self {
        Self {
            epoch,
            blockchain_height,
            last_hash,
            pool_size,
        }
    }
}

/// Proposal for a new block.
///
/// ### Validation
///
/// The message is ignored if it:
///
/// - contains incorrect `prev_hash`
/// - is sent by non-leader
/// - contains already committed transactions
/// - is already known
///
/// ### Processing
///
/// If the message contains unknown transactions, then `TransactionsRequest`
/// is sent in reply.  Otherwise, a `Prevote` is broadcast.
///
/// ### Generation
///
/// A node broadcasts `Propose` if it is a leader and is not locked for a
/// different proposal. Also `Propose` can be sent as response to
/// `ProposeRequest`.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "consensus::Propose")]
pub struct Propose {
    /// The validator id.
    pub validator: ValidatorId,
    /// The epoch to which the message is related.
    pub epoch: Height,
    /// The round to which the message is related.
    pub round: Round,
    /// Hash of the previous block.
    pub prev_hash: Hash,
    /// The list of transactions to include in the next block.
    pub transactions: Vec<Hash>,
    /// Do nothing instead of approving a new block.
    pub skip: bool,
}

impl Propose {
    /// Create new `Propose` message.
    pub fn new(
        validator: ValidatorId,
        epoch: Height,
        round: Round,
        prev_hash: Hash,
        transactions: impl IntoIterator<Item = Hash>,
    ) -> Self {
        Self {
            validator,
            epoch,
            round,
            prev_hash,
            transactions: transactions.into_iter().collect(),
            skip: false,
        }
    }

    pub fn skip(validator: ValidatorId, epoch: Height, round: Round, prev_hash: Hash) -> Self {
        Self {
            validator,
            epoch,
            round,
            prev_hash,
            transactions: vec![],
            skip: true,
        }
    }
}

/// Pre-vote for a new block.
///
/// ### Validation
///
/// A node panics if it has already sent a different `Prevote` for the same
/// round.
///
/// ### Processing
///
/// `Prevote` is added to the list of known votes for the same proposal.  If
/// `locked_round` number from the message is greater than in the node state,
/// then the node replies with `PrevotesRequest`.  If there are unknown
/// transactions in the propose specified by `propose_hash`,
/// `TransactionsRequest` is sent in reply.  Otherwise if all transactions
/// are known and there are +2/3 pre-votes, then a node is locked to that
/// proposal and a `Precommit` is broadcast.
///
/// ### Generation
///
/// A node broadcasts `Prevote` in response to `Propose` when it has
/// received all the transactions.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug, ProtobufConvert)]
#[protobuf_convert(source = "consensus::Prevote")]
pub struct Prevote {
    /// The validator id.
    pub validator: ValidatorId,
    /// The epoch to which the message is related.
    pub epoch: Height,
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
        epoch: Height,
        round: Round,
        propose_hash: Hash,
        locked_round: Round,
    ) -> Self {
        Self {
            validator,
            epoch,
            round,
            propose_hash,
            locked_round,
        }
    }
}

/// Information about a block.
///
/// ### Processing
///
/// The block is added to the blockchain.
///
/// ### Generation
///
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

    /// Verifies Merkle root of transactions in the block.
    pub fn verify_tx_hash(&self) -> bool {
        self.block.tx_hash == HashTag::hash_list(&self.transactions)
    }
}

/// Information about the transactions.
///
/// ### Validation
///
/// The message is ignored if:
///
/// - its `to` field corresponds to a different node
/// - the `transactions` field cannot be parsed or verified
///
/// ### Processing
///
/// Returns information about the transactions requested by the hash.
///
/// ### Generation
///
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
}

/// Request for the `Propose`.
///
/// ### Validation
///
/// The message is ignored if its `epoch` is not equal to the node epoch.
///
/// ### Processing
///
/// `Propose` is sent as the response.
///
/// ### Generation
///
/// A node can send `ProposeRequest` during `Precommit` and `Prevote` handling.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug, ProtobufConvert)]
#[protobuf_convert(source = "consensus::ProposeRequest")]
pub struct ProposeRequest {
    /// Public key of the recipient.
    pub to: PublicKey,
    /// The epoch to which the message is related.
    pub epoch: Height,
    /// Hash of the `Propose`.
    pub propose_hash: Hash,
}

impl ProposeRequest {
    /// Create new `ProposeRequest`.
    pub fn new(to: PublicKey, epoch: Height, propose_hash: Hash) -> Self {
        Self {
            to,
            epoch,
            propose_hash,
        }
    }
}

/// Request for transactions by hash.
///
/// ### Processing
///
/// Requested transactions are sent to the recipient.
///
/// ### Generation
///
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
///
/// The message is ignored if its `epoch` is not equal to the node epoch.
///
/// ### Processing
///
/// The requested pre-votes are sent to the recipient.
///
/// ### Generation
///
/// This message can be sent during `Prevote` and `Precommit` handling.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug, ProtobufConvert)]
#[protobuf_convert(source = "consensus::PrevotesRequest")]
pub struct PrevotesRequest {
    /// Public key of the recipient.
    pub to: PublicKey,
    /// The epoch to which the message is related.
    pub epoch: Height,
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
        epoch: Height,
        round: Round,
        propose_hash: Hash,
        validators: BitVec,
    ) -> Self {
        Self {
            to,
            epoch,
            round,
            propose_hash,
            validators,
        }
    }
}

/// Request connected peers from a node.
///
/// ### Validation
///
/// Request is considered valid if the sender of the message on the network
/// level corresponds to the `from` field.
///
/// ### Processing
///
/// Peer `Connect` messages are sent to the recipient.
///
/// ### Generation
///
/// `PeersRequest` message is sent regularly with the timeout controlled by
/// `ConsensusConfig::peers_timeout`.
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
}

/// Request for the block with the given height or the latest block skip at the previous height.
///
/// ### Validation
///
/// The message is ignored if its `height` is greater than the node's one and any of these conditions
/// hold:
///
/// - Message `epoch` is set to 0.
/// - Message `epoch` is lesser than the epoch of the node.
///
/// ### Processing
///
/// `BlockResponse` message is sent as the response. Which block or block skip is sent, depends
/// on the following rules:
///
/// - If the `epoch` is set to 0, it is a block at the specified `height`.
/// - If the `epoch != 0`, it is a block at the specified `height` (if it is known to the node),
///   or the latest block skip with the epoch greater or equal to the `epoch` mentioned
///   in the message.
///
/// ### Generation
///
/// This message can be sent during `Status` processing.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug, ProtobufConvert)]
#[protobuf_convert(source = "consensus::BlockRequest")]
pub struct BlockRequest {
    /// Public key of the recipient.
    pub to: PublicKey,
    /// The blockchain height to retrieve.
    pub height: Height,
    /// The epoch to retrieve if the specified blockchain height is not reached by the node.
    /// This value is set to `Height(0)` to signal to skip this stage of message processing.
    pub epoch: Height,
}

impl BlockRequest {
    /// Creates a new `BlockRequest`.
    pub fn new(to: PublicKey, height: Height) -> Self {
        Self {
            to,
            height,
            epoch: Height(0),
        }
    }

    /// Creates a new `BlockRequest` with the specified epoch.
    pub fn with_epoch(to: PublicKey, height: Height, epoch: Height) -> Self {
        debug_assert!(epoch > Height(0));
        Self { to, height, epoch }
    }

    /// Returns the effective value of `epoch` in this request.
    pub fn epoch(&self) -> Option<Height> {
        if self.epoch == Height(0) {
            None
        } else {
            Some(self.epoch)
        }
    }
}

/// Enumeration of all possible types of Exonum messages which are used in P2P communication
/// between nodes.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(
    source = "consensus::ExonumMessage",
    rename(case = "snake_case"),
    impl_from_trait
)]
#[allow(clippy::large_enum_variant)]
pub enum ExonumMessage {
    /// Transaction.
    AnyTx(AnyTx),
    /// Handshake to other node.
    Connect(Connect),
    /// Status information of the other node.
    Status(Status),
    /// Consensus `Propose` message.
    Propose(Propose),
    /// Consensus `Prevote` message.
    Prevote(Prevote),
    /// Consensus `Precommit` message.
    Precommit(Precommit),
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
    type Error = anyhow::Error;

    fn try_from(value: SignedMessage) -> Result<Self, Self::Error> {
        Self::from_bytes(value.payload.into())
    }
}

impl TryFrom<&SignedMessage> for ExonumMessage {
    type Error = anyhow::Error;

    fn try_from(value: &SignedMessage) -> Result<Self, Self::Error> {
        let bytes = std::borrow::Cow::Borrowed(value.payload.as_ref());
        Self::from_bytes(bytes)
    }
}

impl_exonum_msg_try_from_signed! {
    ExonumMessage => Connect, Status,
    Propose, Prevote, TransactionsResponse,
    BlockResponse, ProposeRequest, TransactionsRequest,
    PrevotesRequest, PeersRequest, BlockRequest, PoolTransactionsRequest
}
