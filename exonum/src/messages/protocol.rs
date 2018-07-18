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

use chrono::{DateTime, Utc};

use std::net::SocketAddr;

use super::{BitVec, RawMessage, ServiceMessage};
use blockchain;
use crypto::{self, Hash, HashStream, PublicKey};
use helpers::{Height, Round, ValidatorId};

/// Consensus message type.
pub const CONSENSUS: u16 = 0;

/// `Connect` message id.
pub const CONNECT_MESSAGE_ID: u16 = Connect::MESSAGE_ID;
/// `Status` message id.
pub const STATUS_MESSAGE_ID: u16 = Status::MESSAGE_ID;

/// `Propose` message id.
pub const PROPOSE_MESSAGE_ID: u16 = Propose::MESSAGE_ID;
/// `Prevote` message id.
pub const PREVOTE_MESSAGE_ID: u16 = Prevote::MESSAGE_ID;
/// `Precommit` message id.
pub const PRECOMMIT_MESSAGE_ID: u16 = Precommit::MESSAGE_ID;
/// `BlockResponse` message id.
pub const BLOCK_RESPONSE_MESSAGE_ID: u16 = BlockResponse::MESSAGE_ID;
/// `TransactionsResponse` message id.
pub const TRANSACTIONS_RESPONSE_MESSAGE_ID: u16 = TransactionsResponse::MESSAGE_ID;

/// `ProposeRequest` message id.
pub const PROPOSE_REQUEST_MESSAGE_ID: u16 = ProposeRequest::MESSAGE_ID;
/// `TransactionsRequest` message id.
pub const TRANSACTIONS_REQUEST_MESSAGE_ID: u16 = TransactionsRequest::MESSAGE_ID;
/// `PrevotesRequest` message id.
pub const PREVOTES_REQUEST_MESSAGE_ID: u16 = PrevotesRequest::MESSAGE_ID;
/// `PeersRequest` message id.
pub const PEERS_REQUEST_MESSAGE_ID: u16 = PeersRequest::MESSAGE_ID;
/// `BlockRequest` message id.
pub const BLOCK_REQUEST_MESSAGE_ID: u16 = BlockRequest::MESSAGE_ID;

messages! {
    const SERVICE_ID = CONSENSUS;

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
        /// The sender's public key.
        pub_key: &PublicKey,
        /// The node's address.
        addr: SocketAddr,
        /// Time when the message was created.
        time: DateTime<Utc>,
        /// String containing information about this node including Exonum, Rust and OS versions.
        user_agent: &str,
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
    struct Status {
        /// The sender's public key.
        from: &PublicKey,
        /// The height to which the message is related.
        height: Height,
        /// Hash of the last committed block.
        last_hash: &Hash,
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
        /// The sender's public key.
        from: &PublicKey,
        /// Public key of the recipient.
        to: &PublicKey,
        /// Block header.
        block: blockchain::Block,
        /// List of pre-commits.
        precommits: Vec<Precommit>,
        /// List of the transaction hashes.
        transactions: &[Hash],
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
    struct TransactionsResponse {
        /// The sender's public key.
        from: &PublicKey,
        /// Public key of the recipient.
        to: &PublicKey,
        /// List of the transactions.
        transactions: Vec<RawMessage>,
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
    struct ProposeRequest {
        /// The sender's public key.
        from: &PublicKey,
        /// Public key of the recipient.
        to: &PublicKey,
        /// The height to which the message is related.
        height: Height,
        /// Hash of the `Propose`.
        propose_hash: &Hash,
    }

    /// Request for transactions by hash.
    ///
    /// ### Processing
    /// Requested transactions are sent to the recipient.
    ///
    /// ### Generation
    /// This message can be sent during `Propose`, `Prevote` and `Precommit`
    /// handling.
    struct TransactionsRequest {
        /// The sender's public key.
        from: &PublicKey,
        /// Public key of the recipient.
        to: &PublicKey,
        /// The list of the transaction hashes.
        txs: &[Hash],
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
    struct PrevotesRequest {
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
        /// The sender's public key.
        from: &PublicKey,
        /// Public key of the recipient.
        to: &PublicKey,
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
    struct BlockRequest {
        /// The sender's public key.
        from: &PublicKey,
        /// Public key of the recipient.
        to: &PublicKey,
        /// The height to which the message is related.
        height: Height,
    }
}

impl BlockResponse {
    /// Verify Merkle root of transactions in the block.
    pub fn verify_tx_hash(&self) -> bool {
        *self.block().tx_hash() == merkle_root(self.transactions())
    }
}

fn merkle_root(hashes: &[Hash]) -> Hash {
    if hashes.is_empty() {
        return Hash::zero();
    }
    let mut current_hashes = hashes.to_vec();
    while current_hashes.len() > 1 {
        current_hashes = combine_hash_list(&current_hashes);
    }
    current_hashes[0]
}

fn combine_hash_list(hashes: &[Hash]) -> Vec<Hash> {
    hashes
        .chunks(2)
        .map(|pair|
            // Keep hash combination consistent with ProofListIndex.
            if pair.len() == 2 {
                hash_pair(&pair[0], &pair[1])
            } else {
                hash_one(&pair[0])
            }
        )
        .collect()
}

fn hash_one(lhs: &Hash) -> Hash {
    crypto::hash(lhs.as_ref())
}

fn hash_pair(lhs: &Hash, rhs: &Hash) -> Hash {
    HashStream::new()
        .update(lhs.as_ref())
        .update(rhs.as_ref())
        .hash()
}

#[cfg(test)]
mod tests {
    use crypto::{self, Hash};
    use encoding::serialize::FromHex;
    use storage::{Database, MemoryDB, ProofListIndex};

    use super::*;

    /// Cross-verify `merkle_root()` with `ProofListIndex` against expected root hash value.
    fn assert_root_hash_eq(hashes: &[Hash], expected: &str) {
        let root_actual = merkle_root(hashes);
        let root_index = proof_list_index_root(hashes);
        let root_expected = Hash::from_hex(expected).expect("hex hash");
        assert_eq!(root_actual, root_expected);
        assert_eq!(root_actual, root_index);
    }

    fn proof_list_index_root(hashes: &[Hash]) -> Hash {
        let db = MemoryDB::new();
        let mut fork = db.fork();
        let mut index = ProofListIndex::new("merkle_root", &mut fork);
        index.extend(hashes.iter().cloned());
        index.merkle_root()
    }

    fn hash_list(bytes: &[&[u8]]) -> Vec<Hash> {
        bytes.iter().map(|chunk| crypto::hash(chunk)).collect()
    }

    #[test]
    fn root_hash_single() {
        assert_root_hash_eq(
            &hash_list(&[b"1"]),
            "6b86b273ff34fce19d6b804eff5a3f5747ada4eaa22f1d49c01e52ddb7875b4b",
        );
    }

    #[test]
    fn root_hash_even() {
        assert_root_hash_eq(
            &hash_list(&[b"1", b"2", b"3", b"4"]),
            "cd53a2ce68e6476c29512ea53c395c7f5d8fbcb4614d89298db14e2a5bdb5456",
        );
    }

    #[test]
    fn root_hash_odd() {
        assert_root_hash_eq(
            &hash_list(&[b"1", b"2", b"3", b"4", b"5"]),
            "9d6f6f12f390c2f281beacc79fd527f2355f555aa6f47682de41cbaf7756e187",
        );
    }

    #[test]
    fn root_hash_empty() {
        assert_root_hash_eq(
            &hash_list(&[]),
            "0000000000000000000000000000000000000000000000000000000000000000",
        );
    }
}
