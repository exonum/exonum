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

use bit_vec::BitVec;
use chrono::{DateTime, Utc};
use exonum_merkledb::{BinaryValue, HashTag};

use std::borrow::Cow;

use crate::{
    blockchain::Block,
    crypto::{Hash, PublicKey, Signature},
    helpers::{Height, Round, ValidatorId},
    proto::{
        schema::{consensus, runtime},
        ProtobufConvert,
    },
};

/// Container for the signed messages.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug, ProtobufConvert)]
#[exonum(pb = "consensus::Signed", crate = "crate")]
pub struct Signed {
    /// Message payload.
    pub(super) payload: Vec<u8>,
    /// Message author.
    pub(super) author: PublicKey,
    /// Digital signature.
    pub(super) signature: Signature,
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
#[exonum(pb = "consensus::Connect", crate = "crate")]
pub struct Connect {
    /// The node's address.
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
        Connect {
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
#[exonum(pb = "consensus::Status", crate = "crate")]
pub struct Status {
    /// The height to which the message is related.
    pub height: Height,
    /// Hash of the last committed block.
    pub last_hash: Hash,
}

impl Status {
    /// Create new `Status` message.
    pub fn new(height: Height, last_hash: Hash) -> Self {
        Self { height, last_hash }
    }

    /// The height to which the message is related.
    pub fn height(&self) -> Height {
        self.height
    }

    /// Hash of the last committed block.
    pub fn last_hash(&self) -> &Hash {
        &self.last_hash
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
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug, ProtobufConvert)]
#[exonum(pb = "consensus::Propose", crate = "crate")]
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
        prev_hash: &Hash,
        transactions: &[Hash],
    ) -> Self {
        Self {
            validator,
            height,
            round,
            prev_hash: *prev_hash,
            transactions: transactions.to_vec(),
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
#[exonum(pb = "consensus::Prevote", crate = "crate")]
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
        propose_hash: &Hash,
        locked_round: Round,
    ) -> Self {
        Self {
            validator,
            height,
            round,
            propose_hash: *propose_hash,
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
/// +2/3 precommits for the same proposal with the same block_hash, then
/// block is executed and `Status` is broadcast.
///
/// ### Generation
/// A node broadcasts `Precommit` in response to `Prevote` if there are +2/3
/// pre-votes and no unknown transactions.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug, Serialize, Deserialize, ProtobufConvert)]
#[exonum(pb = "consensus::Precommit", crate = "crate")]
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
        propose_hash: &Hash,
        block_hash: &Hash,
        time: DateTime<Utc>,
    ) -> Self {
        Self {
            validator,
            height,
            round,
            propose_hash: *propose_hash,
            block_hash: *block_hash,
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
#[exonum(pb = "consensus::BlockResponse", crate = "crate")]
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
        to: &PublicKey,
        block: Block,
        precommits: Vec<Vec<u8>>,
        transactions: &[Hash],
    ) -> Self {
        Self {
            to: *to,
            block,
            precommits,
            transactions: transactions.to_vec(),
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
    /// List of pre-commits.
    pub fn precommits(&self) -> Vec<Vec<u8>> {
        self.precommits.clone()
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
#[exonum(pb = "consensus::TransactionsResponse", crate = "crate")]
pub struct TransactionsResponse {
    /// Public key of the recipient.
    pub to: PublicKey,
    /// List of the transactions.
    pub transactions: Vec<Vec<u8>>,
}

impl TransactionsResponse {
    /// Create new `TransactionsResponse` message.
    pub fn new(to: &PublicKey, transactions: Vec<Vec<u8>>) -> Self {
        Self {
            to: *to,
            transactions,
        }
    }

    /// Public key of the recipient.
    pub fn to(&self) -> &PublicKey {
        &self.to
    }
    /// List of the transactions.
    pub fn transactions(&self) -> Vec<Vec<u8>> {
        self.transactions.clone()
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
#[exonum(pb = "consensus::ProposeRequest", crate = "crate")]
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
    pub fn new(to: &PublicKey, height: Height, propose_hash: &Hash) -> Self {
        Self {
            to: *to,
            height,
            propose_hash: *propose_hash,
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
#[exonum(pb = "consensus::TransactionsRequest", crate = "crate")]
pub struct TransactionsRequest {
    /// Public key of the recipient.
    pub to: PublicKey,
    /// The list of the transaction hashes.
    pub txs: Vec<Hash>,
}

impl TransactionsRequest {
    /// Create new `TransactionsRequest`.
    pub fn new(to: &PublicKey, txs: &[Hash]) -> Self {
        Self {
            to: *to,
            txs: txs.to_vec(),
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
#[exonum(pb = "consensus::PrevotesRequest", crate = "crate")]
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
        to: &PublicKey,
        height: Height,
        round: Round,
        propose_hash: &Hash,
        validators: BitVec,
    ) -> Self {
        Self {
            to: *to,
            height,
            round,
            propose_hash: *propose_hash,
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
#[exonum(pb = "consensus::PeersRequest", crate = "crate")]
pub struct PeersRequest {
    /// Public key of the recipient.
    pub to: PublicKey,
}

impl PeersRequest {
    /// Create new `PeersRequest`.
    pub fn new(to: &PublicKey) -> Self {
        Self { to: *to }
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
#[exonum(pb = "consensus::BlockRequest", crate = "crate")]
pub struct BlockRequest {
    /// Public key of the recipient.
    pub to: PublicKey,
    /// The height to which the message is related.
    pub height: Height,
}

impl BlockRequest {
    /// Create new `BlockRequest`.
    pub fn new(to: &PublicKey, height: Height) -> Self {
        Self { to: *to, height }
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
        // TODO WTF? [ECR-3222]
        //        let tx_hash = schema.block_transactions(height).object_hash();
        // HashTag::hash_list_node(self.len(), self.merkle_root())

        //        let list_hash =
        //            HashTag::hash_list_node(self.transactions().len() as u64, *self.block().tx_hash());

        let res = *self.block().tx_hash() == HashTag::hash_list(self.transactions());

        println!("block {:?}", self.block());
        //        println!("list_hash {:?}", list_hash);
        println!("tx_hash {:?}", *self.block().tx_hash());
        println!("hash_list {:?}", HashTag::hash_list(self.transactions()));
        println!("verify_tx_hash: res {}", res);
        res
    }
}

// TODO Move to runtime module

/// Unique service transaction identifier.
#[derive(Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Debug, ProtobufConvert)]
#[exonum(pb = "runtime::CallInfo", crate = "crate")]
pub struct CallInfo {
    /// Service instance identifier.
    pub instance_id: u32,
    /// Identifier of method in service interface to call.
    pub method_id: u32,
}

impl CallInfo {
    /// Creates a new `CallInfo` instance.
    pub fn new(instance_id: u32, method_id: u32) -> Self {
        Self {
            instance_id,
            method_id,
        }
    }
}

/// Transaction with call info.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug, ProtobufConvert)]
#[exonum(pb = "runtime::AnyTx", crate = "crate")]
pub struct AnyTx {
    /// Dispatch info.
    pub call_info: CallInfo,
    /// Serialized transaction.
    pub payload: Vec<u8>,
}

impl AnyTx {
    /// Method for compatibility with old transactions.
    /// Creates equivalent of `RawTransaction`.
    pub fn new(service_id: u16, tx_id: u16, payload: Vec<u8>) -> Self {
        Self {
            call_info: CallInfo {
                instance_id: u32::from(service_id),
                method_id: u32::from(tx_id),
            },
            payload,
        }
    }

    /// Parses transaction content as concrete type.
    pub fn parse<T: BinaryValue>(&self) -> Result<T, failure::Error> {
        T::from_bytes(Cow::Borrowed(&self.payload))
    }
}

#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug)]
pub enum ProtocolMessage {
    AnyTx(AnyTx),
    Connect(Connect),
    Status(Status),
    Precommit(Precommit),
    Propose(Propose),
    Prevote(Prevote),
    TransactionsResponse(TransactionsResponse),
    BlockResponse(BlockResponse),
    ProposeRequest(ProposeRequest),
    TransactionsRequest(TransactionsRequest),
    PrevotesRequest(PrevotesRequest),
    PeersRequest(PeersRequest),
    BlockRequest(BlockRequest),
}

macro_rules! impl_protocol_message {
    ( $($name:ident : $into_pb:expr, $from_pb:expr)* ) => {
        impl ProtobufConvert for ProtocolMessage {
            type ProtoStruct = consensus::ProtocolMessage;

            fn to_pb(&self) -> Self::ProtoStruct {
                let mut inner = Self::ProtoStruct::default();
                match self {
                    $( ProtocolMessage::$name(msg) => unimplemented!(), )*
                }
                inner
            }

            fn from_pb(pb: Self::ProtoStruct) -> Result<Self, failure::Error> {
                unimplemented!()
            }
        }
    }
}

impl_protocol_message! {
    AnyTx: set_any_tx, any_tx
    Connect: set_connect, connect
    Status: set_status, status
    Precommit: set_precommit, precommits
    Propose: set_propose, propose
    Prevote: set_prevote, prevote
    TransactionsResponse: set_txs_response, txs_response
    BlockResponse: set_block_response, block_response
    ProposeRequest: set_propose_req, propose_req
    TransactionsRequest: set_txs_req, txs_req
    PrevotesRequest: set_prevotes_req, prevotes_req
    PeersRequest: set_peers_req, peers_req
    BlockRequest: set_block_req, block_req
}

// Implement #[derive(ProtobufConvert)]
use crate::proto::schema::consensus::ProtocolMessage_oneof_message as ProtocolMessagePb;

// impl ProtobufConvert for ProtocolMessage {
//     type ProtoStruct = consensus::ProtocolMessage;

//     fn to_pb(&self) -> Self::ProtoStruct {
//         let mut inner = Self::ProtoStruct::default();
//         match self {
//             ProtocolMessage::AnyTx(msg) => inner.set_any_tx(msg.to_pb()),
//             ProtocolMessage::Connect(msg) => inner.set_connect(msg),
//             ProtocolMessage::Status(msg) => inner.set_status(msg),
//             ProtocolMessage::Precommit(msg) => inner.set_precommit(msg),
//             ProtocolMessage::Propose(msg) => inner.set_propose(msg),
//             ProtocolMessage::Prevote(msg) => inner.set_prevote(msg),
//             ProtocolMessage::TransactionsResponse(msg) => inner.set_txs_response(msg),
//             ProtocolMessage::BlockResponse(msg) => inner.set_block_response(msg),
//             ProtocolMessage::ProposeRequest(msg) => inner.set_any_tx(msg),
//             ProtocolMessage::TransactionsRequest(msg) => inner.set_any_tx(msg),
//             ProtocolMessage::PrevotesRequest(msg) => inner.set_any_tx(msg),
//             ProtocolMessage::PeersRequest(msg) => inner.set_any_tx(msg),
//             ProtocolMessage::BlockRequest(msg) => inner.set_any_tx(msg),
//         }
//         inner
//     }

//     fn from_pb(pb: Self::ProtoStruct) -> Result<Self, failure::Error> {
//         unimplemented!()
//     }
// }
