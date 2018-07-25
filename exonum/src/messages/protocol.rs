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

use bit_vec::BitVec;
use chrono::{DateTime, Utc};

use std::fmt::Debug;
use std::net::SocketAddr;

use failure;

use super::{Message, RawTransaction, SignedMessage, UncheckedBuffer, BinaryFormSerialize};
use blockchain;
use crypto::{Hash, PublicKey};
use helpers::{Height, Round, ValidatorId};
use storage::{Database, MemoryDB, ProofListIndex};

#[doc(hidden)]
/// TransactionsResponse size with zero transactions inside.
pub const TRANSACTION_RESPONSE_EMPTY_SIZE: usize = 261;

#[doc(hidden)]
/// RawTransaction size with zero transactions payload.
pub const RAW_TRANSACTION_EMPTY_SIZE: usize = 0;

/// Any possible message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Protocol {
    /// Transaction.
    Transaction(RawTransaction),
    /// Consensus message.
    Consensus(ConsensusMessage),
    /// `Connect` message.
    #[serde(with = "BinaryFormSerialize")]
    Connect(Connect),
    /// `Status` message.
    #[serde(with = "BinaryFormSerialize")]
    Status(Status),
    /// `Block` message.
    #[serde(with = "BinaryFormSerialize")]
    Block(BlockResponse),
    /// Request for the some data.
    Request(RequestMessage),
    /// A batch of the transactions.
    #[serde(with = "BinaryFormSerialize")]
    TransactionsBatch(TransactionsResponse),
}

/// Consensus message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ConsensusMessage {
    /// `Precommit` message.
    #[serde(with = "BinaryFormSerialize")]
    Precommit(Precommit),
    /// `Propose` message.
    #[serde(with = "BinaryFormSerialize")]
    Propose(Propose),
    /// `Prevote` message.
    #[serde(with = "BinaryFormSerialize")]
    Prevote(Prevote),
}

/// A request for the some data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RequestMessage {
    /// Propose request.
    #[serde(with = "BinaryFormSerialize")]
    Propose(ProposeRequest),
    /// Transactions request.
    #[serde(with = "BinaryFormSerialize")]
    Transactions(TransactionsRequest),
    /// Prevotes request.
    #[serde(with = "BinaryFormSerialize")]
    Prevotes(PrevotesRequest),
    /// Peers request.
    #[serde(with = "BinaryFormSerialize")]
    Peers(PeersRequest),
    /// Block request.
    #[serde(with = "BinaryFormSerialize")]
    Block(BlockRequest),
}

impl RequestMessage {
    /// Returns public key of the message recipient.
    pub fn to(&self) -> &PublicKey {
        match *self {
            RequestMessage::Propose(ref msg) => msg.to(),
            RequestMessage::Transactions(ref msg) => msg.to(),
            RequestMessage::Prevotes(ref msg) => msg.to(),
            RequestMessage::Peers(ref msg) => msg.to(),
            RequestMessage::Block(ref msg) => msg.to(),
        }
    }
}

impl ConsensusMessage {
    /// Returns validator id of the message sender.
    pub fn validator(&self) -> ValidatorId {
        match *self {
            ConsensusMessage::Propose(ref msg) => msg.validator(),
            ConsensusMessage::Prevote(ref msg) => msg.validator(),
            ConsensusMessage::Precommit(ref msg) => msg.validator(),
        }
    }

    /// Returns height of the message.
    pub fn height(&self) -> Height {
        match *self {
            ConsensusMessage::Propose(ref msg) => msg.height(),
            ConsensusMessage::Prevote(ref msg) => msg.height(),
            ConsensusMessage::Precommit(ref msg) => msg.height(),
        }
    }

    /// Returns round of the message.
    pub fn round(&self) -> Round {
        match *self {
            ConsensusMessage::Propose(ref msg) => msg.round(),
            ConsensusMessage::Prevote(ref msg) => msg.round(),
            ConsensusMessage::Precommit(ref msg) => msg.round(),
        }
    }
}

encoding_struct! {
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
    struct Connect {
        /// The node's address.
        addr: SocketAddr,
        /// Time when the message was created.
        time: DateTime<Utc>,
        /// String containing information about this node including Exonum, Rust and OS versions.
        user_agent: &str,
    }

}
encoding_struct! {
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
    struct Status {
        /// The height to which the message is related.
        height: Height,
        /// Hash of the last committed block.
        last_hash: &Hash,
    }
}
encoding_struct! {
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
    struct Propose {
        /// The validator id.
        validator: ValidatorId,
        /// The height to which the message is related.
        height: Height,
        /// The round to which the message is related.
        round: Round,
        /// Hash of the previous block.
        prev_hash: &Hash,
        /// The list of transactions to include in the next block.
        transactions: &[Hash],
    }
}
encoding_struct! {
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
    struct Prevote {
        /// The validator id.
        validator: ValidatorId,
        /// The height to which the message is related.
        height: Height,
        /// The round to which the message is related.
        round: Round,
        /// Hash of the corresponding `Propose`.
        propose_hash: &Hash,
        /// Locked round.
        locked_round: Round,
    }
}
encoding_struct! {
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
    struct Precommit {
        /// The validator id.
        validator: ValidatorId,
        /// The height to which the message is related.
        height: Height,
        /// The round to which the message is related.
        round: Round,
        /// Hash of the corresponding `Propose`.
        propose_hash: &Hash,
        /// Hash of the new block.
        block_hash: &Hash,
        /// Time of the `Precommit`.
        time: DateTime<Utc>,
    }
}
encoding_struct! {
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
    struct BlockResponse {
        /// Public key of the recipient.
        to: &PublicKey,
        /// Block header.
        block: blockchain::Block,
        /// List of pre-commits.
        precommits: Vec<UncheckedBuffer>,
        /// List of the transaction hashes.
        transactions: &[Hash],
    }
}
encoding_struct! {

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
    struct TransactionsResponse {
        /// Public key of the recipient.
        to: &PublicKey,
        /// List of the transactions.
        transactions: Vec<UncheckedBuffer>,
    }

}
encoding_struct! {
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
    struct ProposeRequest {
        /// Public key of the recipient.
        to: &PublicKey,
        /// The height to which the message is related.
        height: Height,
        /// Hash of the `Propose`.
        propose_hash: &Hash,
    }
}
encoding_struct! {
    /// Request for transactions by hash.
    ///
    /// ### Processing
    /// Requested transactions are sent to the recipient.
    ///
    /// ### Generation
    /// This message can be sent during `Propose`, `Prevote` and `Precommit`
    /// handling.
    struct TransactionsRequest {
        /// Public key of the recipient.
        to: &PublicKey,
        /// The list of the transaction hashes.
        txs: &[Hash],
    }
}
encoding_struct! {
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
    struct PrevotesRequest {
        /// Public key of the recipient.
        to: &PublicKey,
        /// The height to which the message is related.
        height: Height,
        /// The round to which the message is related.
        round: Round,
        /// Hash of the `Propose`.
        propose_hash: &Hash,
        /// The list of validators that send pre-votes.
        validators: BitVec,
    }
}
encoding_struct! {
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
    struct PeersRequest {
        /// Public key of the recipient.
        to: &PublicKey,
    }
}
encoding_struct! {
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

    struct BlockRequest {
        /// Public key of the recipient.
        to: & PublicKey,
        /// The height to which the message is related.
        height: Height,
    }
}

impl BlockResponse {
    /// Verify Merkle root of transactions in the block.
    pub fn verify_tx_hash(&self) -> bool {
        let db = MemoryDB::new();
        let mut fork = db.fork();
        let mut index = ProofListIndex::new("verify_tx_hash", &mut fork);
        index.extend(self.transactions().iter().cloned());
        let tx_hashes = index.merkle_root();
        tx_hashes == *self.block().tx_hash()
    }
}

impl Precommit {
    /// Verify precommit's signature and return it's safer wrapper
    pub(crate) fn verify_precommit(
        buffer: UncheckedBuffer,
    ) -> Result<Message<Precommit>, ::failure::Error> {
        let signed = SignedMessage::verify_buffer(buffer)?;
        signed.into_message().map_into::<Precommit>()
    }
}
/// Full message constraints list.
#[doc(hidden)]
pub trait ProtocolMessage:
    Debug + Into<Protocol> + PartialEq<Protocol> + Clone + TryFromProtocol
{
}
impl<T: Debug + Into<Protocol> + PartialEq<Protocol> + Clone + TryFromProtocol> ProtocolMessage
    for T
{
}

/// Specialised `TryFrom` analog.
#[doc(hidden)]
pub trait TryFromProtocol: Sized {
    fn try_from_protocol(value: Protocol) -> Result<Self, failure::Error>;
}

impl TryFromProtocol for Protocol {
    fn try_from_protocol(value: Protocol) -> Result<Self, failure::Error> {
        Ok(value)
    }
}

macro_rules! impl_protocol {
    ($val:ident => $v:ident = ($($ma:tt)*) => $($ma2:tt)*) => {
    impl PartialEq<Protocol> for $val {
        fn eq(&self, other: &Protocol) -> bool {
            if let $($ma2)* = *other {
                return $v == self;
            }
            false
        }
    }

    impl TryFromProtocol for $val {
        fn try_from_protocol(value: Protocol) -> Result<Self, failure::Error> {
            match value {
                $($ma)* => Ok($v),
                _ => bail!(concat!("Received message other than ", stringify!($val)) )
            }
        }
    }

    impl Into<Protocol> for $val {
        fn into(self) -> Protocol {
            let $v = self;
            $($ma)*
        }
    }

    };
}

//TODO: Replace by better arm parsing

impl_protocol!{Connect => c =
(Protocol::Connect(c)) => Protocol::Connect(ref c)}
impl_protocol!{Status => c =
(Protocol::Status(c)) => Protocol::Status(ref c)}
impl_protocol!{BlockResponse => c =
(Protocol::Block(c)) => Protocol::Block(ref c)}
impl_protocol!{RawTransaction => c =
(Protocol::Transaction(c)) => Protocol::Transaction(ref c)}
impl_protocol!{TransactionsResponse => c =
(Protocol::TransactionsBatch(c)) => Protocol::TransactionsBatch(ref c)}

impl_protocol!{ConsensusMessage => c =
(Protocol::Consensus(c)) => Protocol::Consensus(ref c)}
impl_protocol!{Propose => c =
(Protocol::Consensus(ConsensusMessage::Propose(c))) =>
Protocol::Consensus(ConsensusMessage::Propose(ref c))}
impl_protocol!{Prevote => c =
(Protocol::Consensus(ConsensusMessage::Prevote(c))) =>
Protocol::Consensus(ConsensusMessage::Prevote(ref c))}
impl_protocol!{Precommit => c =
(Protocol::Consensus(ConsensusMessage::Precommit(c))) =>
Protocol::Consensus(ConsensusMessage::Precommit(ref c))}

impl_protocol!{RequestMessage => c =
(Protocol::Request(c)) => Protocol::Request(ref c)}
impl_protocol!{ProposeRequest => c =
(Protocol::Request(RequestMessage::Propose(c))) =>
Protocol::Request(RequestMessage::Propose(ref c))}
impl_protocol!{TransactionsRequest => c =
(Protocol::Request(RequestMessage::Transactions(c))) =>
Protocol::Request(RequestMessage::Transactions(ref c))}
impl_protocol!{PrevotesRequest => c =
(Protocol::Request(RequestMessage::Prevotes(c))) =>
Protocol::Request(RequestMessage::Prevotes(ref c))}
impl_protocol!{PeersRequest => c =
(Protocol::Request(RequestMessage::Peers(c))) =>
Protocol::Request(RequestMessage::Peers(ref c))}
impl_protocol!{BlockRequest => c =
(Protocol::Request(RequestMessage::Block(c))) =>
Protocol::Request(RequestMessage::Block(ref c))}
