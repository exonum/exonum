// Copyright 2017 The Exonum Team
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

use std::net::SocketAddr;
use std::time::SystemTime;

use crypto::{Hash, PublicKey};
use blockchain;
use helpers::{Height, Round, ValidatorId};
use super::{RawMessage, BitVec};

/// Consensus message type.
pub const CONSENSUS: u16 = 0;

/// `Connect` message id.
pub const CONNECT_MESSAGE_ID: u16 = 0;
/// `Status` message id.
pub const STATUS_MESSAGE_ID: u16 = 1;

/// `Propose` message id.
pub const PROPOSE_MESSAGE_ID: u16 = 2;
/// `Prevote` message id.
pub const PREVOTE_MESSAGE_ID: u16 = 3;
/// `Precommit` message id.
pub const PRECOMMIT_MESSAGE_ID: u16 = 4;
/// `BlockResponse` message id.
pub const BLOCK_RESPONSE_MESSAGE_ID: u16 = 5;

/// `ProposeRequest` message id.
pub const PROPOSE_REQUEST_MESSAGE_ID: u16 = 6;
/// `TransactionsRequest` message id.
pub const TRANSACTIONS_REQUEST_MESSAGE_ID: u16 = 7;
/// `PrevotesRequest` message id.
pub const PREVOTES_REQUEST_MESSAGE_ID: u16 = 8;
/// `PeersRequest` message id.
pub const PEERS_REQUEST_MESSAGE_ID: u16 = 9;
/// `BlockRequest` message id.
pub const BLOCK_REQUEST_MESSAGE_ID: u16 = 10;

message! {
/// Connect to a node.
///
/// ### Validation
/// The message is ignored if its time is earlier than in the previous `Connect` message received
/// from the same peer.
///
/// ### Processing
/// Connect to the peer.
///
/// ### Generation
/// A node sends `Connect` message to all known addresses during initialization. Additionally,
/// the node responds by its own `Connect` message after receiving `node::Event::Connected`.
    struct Connect {
        const TYPE = CONSENSUS;
        const ID = CONNECT_MESSAGE_ID;

        /// The sender's public key.
        pub_key: &PublicKey,
        /// The node's address.
        addr: SocketAddr,
        /// Time when the message was created.
        time: SystemTime,
    }
}

message! {
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
/// If the message contains unknown transactions, then `TransactionsRequest` is sent in reply.
/// Otherwise `Prevote` is broadcast.
///
/// ### Generation
/// A node broadcasts `Propose` if it is a leader and is not locked for a different proposal. Also
/// `Propose` can be sent as response to `ProposeRequest`.
    struct Propose {
        const TYPE = CONSENSUS;
        const ID = PROPOSE_MESSAGE_ID;

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

message! {
/// Pre-vote for a new block.
///
/// ### Validation
/// A node panics if it has already sent a different `Prevote` for the same round.
///
/// ### Processing
/// Pre-vote is added to the list of known votes for the same proposal.
/// If `locked_round` number from the message is bigger than in a node state, then a node replies
/// with `PrevotesRequest`.
/// If there are unknown transactions in the propose specified by `propose_hash`,
/// `TransactionsRequest` is sent in reply.
/// Otherwise if all transactions are known and there are +2/3 pre-votes, then a node is locked
/// to that proposal and `Precommit` is broadcast.
///
/// ### Generation
/// A node broadcasts `Prevote` in response to `Propose` when it has received all the transactions.
    struct Prevote {
        const TYPE = CONSENSUS;
        const ID = PREVOTE_MESSAGE_ID;

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

message! {
/// Pre-commit for a proposal.
///
/// ### Validation
/// A node panics if it  has already sent a different `Precommit` for the same round.
///
/// ### Processing
/// Pre-commit is added to the list of known pre-commits.
/// If a proposal is unknown to the node, `ProposeRequest` is sent in reply.
/// If `round` number from the message is bigger than a node's "locked round", then a node replies
/// with `PrevotesRequest`.
/// If there are unknown transactions, then `TransactionsRequest` is sent in reply.
/// If a validator receives +2/3 precommits for the same proposal with the same block_hash, then
/// block is executed and `Status` is broadcast.
///
/// ### Generation
/// A node broadcasts `Precommit` in response to `Prevote` if there are +2/3 pre-votes and no
/// unknown transactions.
    struct Precommit {
        const TYPE = CONSENSUS;
        const ID = PRECOMMIT_MESSAGE_ID;

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
        time: SystemTime,
    }
}

message! {
/// Current node status.
///
/// ### Validation
/// The message is ignored if its signature is incorrect or its `height` is lower than a node's
/// height.
///
/// ### Processing
/// If the message's `height` number is bigger than a node's one, then `BlockRequest` with current
/// node's height is sent in reply.
///
/// ### Generation
/// `Status` message is broadcast regularly with the timeout controlled by
/// `blockchain::ConsensusConfig::status_timeout`. Also, it is broadcast after accepting a new
/// block.
    struct Status {
        const TYPE = CONSENSUS;
        const ID = STATUS_MESSAGE_ID;

        /// The sender's public key.
        from: &PublicKey,
        /// The height to which the message is related.
        height: Height,
        /// Hash of the last committed block.
        last_hash: &Hash,
    }
}

message! {
/// Information about a block.
///
/// ### Validation
/// The message is ignored if
///     * its `to` field corresponds to a different node
///     * the `block`, `transaction` and `precommits` fields cannot be parsed or verified
///
/// ### Processing
/// The block is added to the blockchain.
///
/// ### Generation
/// The message is sent as response to `BlockRequest`.
    struct BlockResponse {
        const TYPE = CONSENSUS;
        const ID = BLOCK_RESPONSE_MESSAGE_ID;

        /// The sender's public key.
        from: &PublicKey,
        /// Public key of the recipient.
        to: &PublicKey,
        /// Block header.
        block: blockchain::Block,
        /// List of pre-commits.
        precommits: Vec<Precommit>,
        /// List of the transactions.
        transactions: Vec<RawMessage>,
    }
}

message! {
/// Request for the `Propose`.
///
/// ### Validation
/// The message is ignored if its `height` is not equal to the node's height.
///
/// ### Processing
/// `Propose` is sent as the response.
///
/// ### Generation
/// A node can send `ProposeRequest` during `Precommit` and `Prevote` handling.
    struct ProposeRequest {
        const TYPE = CONSENSUS;
        const ID = PROPOSE_REQUEST_MESSAGE_ID;

        /// The sender's public key.
        from: &PublicKey,
        /// Public key of the recipient.
        to: &PublicKey,
        /// The height to which the message is related.
        height: Height,
        /// Hash of the `Propose`.
        propose_hash: &Hash,
    }
}

message! {
/// Request for transactions by hash.
///
/// ### Processing
/// Requested transactions are sent to the recipient.
///
/// ### Generation
/// This message can be sent during `Propose`, `Prevote` and `Precommit` handling.
    struct TransactionsRequest {
        const TYPE = CONSENSUS;
        const ID = TRANSACTIONS_REQUEST_MESSAGE_ID;

        /// The sender's public key.
        from: &PublicKey,
        /// Public key of the recipient.
        to: &PublicKey,
        /// The list of the transaction hashes.
        txs: &[Hash],
    }
}

message! {
/// Request for pre-votes.
///
/// ### Validation
/// The message is ignored if its `height` is not equal to the node's height.
///
/// ### Processing
/// The requested pre-votes are sent to the recipient.
///
/// ### Generation
/// This message can be sent during `Prevote` and `Precommit` handling.
    struct PrevotesRequest {
        const TYPE = CONSENSUS;
        const ID = PREVOTES_REQUEST_MESSAGE_ID;

        /// The sender's public key.
        from: &PublicKey,
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

message! {
/// Request connected peers from a node.
///
/// ### Validation
/// Request is considered valid if the sender of the message on the network level corresponds to
/// the `from` field.
///
/// ### Processing
/// Peer `Connect` messages are sent to the recipient.
///
/// ### Generation
/// `PeersRequest` message is sent regularly with the timeout controlled by
/// `blockchain::ConsensusConfig::peers_timeout`.
    struct PeersRequest {
        const TYPE = CONSENSUS;
        const ID = PEERS_REQUEST_MESSAGE_ID;

        /// The sender's public key.
        from: &PublicKey,
        /// Public key of the recipient.
        to: &PublicKey,
    }
}

message! {
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
        const TYPE = CONSENSUS;
        const ID = BLOCK_REQUEST_MESSAGE_ID;

        /// The sender's public key.
        from: &PublicKey,
        /// Public key of the recipient.
        to: &PublicKey,
        /// The height to which the message is related.
        height: Height,
    }
}
