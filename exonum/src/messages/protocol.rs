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
use super::{RawMessage, BitVec};

pub const CONSENSUS: u16 = 0;

pub const CONNECT_MESSAGE_ID: u16 = 0;
pub const STATUS_MESSAGE_ID: u16 = 1;

pub const PROPOSE_MESSAGE_ID: u16 = 2;
pub const PREVOTE_MESSAGE_ID: u16 = 3;
pub const PRECOMMIT_MESSAGE_ID: u16 = 4;
pub const BLOCK_MESSAGE_ID: u16 = 5;

pub const REQUEST_PROPOSE_MESSAGE_ID: u16 = 6;
pub const REQUEST_TRANSACTIONS_MESSAGE_ID: u16 = 7;
pub const REQUEST_PREVOTES_MESSAGE_ID: u16 = 8;
pub const REQUEST_PEERS_MESSAGE_ID: u16 = 9;
pub const REQUEST_BLOCK_MESSAGE_ID: u16 = 10;

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
        const SIZE = 50;

        field pub_key:        &PublicKey  [00 => 32]
        field addr:           SocketAddr  [32 => 38]
        field time:           SystemTime  [38 => 50]
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
/// If the message contains unknown transactions, then `RequestTransactions` is sent in reply.
/// Otherwise `Prevote` is broadcast.
///
/// ### Generation
/// A node broadcasts `Propose` if it is a leader and is not locked for a different proposal. Also
/// `Propose` can be sent as response to `RequestPropose`.
    struct Propose {
        const TYPE = CONSENSUS;
        const ID = PROPOSE_MESSAGE_ID;
        const SIZE = 54;

        field validator:      u16         [00 => 02]
        field height:         u64         [02 => 10]
        field round:          u32         [10 => 14]
        field prev_hash:      &Hash       [14 => 46]
        field transactions:   &[Hash]     [46 => 54]
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
/// with `RequestPrevotes`.
/// If there are unknown transactions in the propose specified by `propose_hash`,
/// `RequestTransactions` is sent in reply.
/// Otherwise if all transactions are known and there are +2/3 pre-votes, then a node is locked
/// to that proposal and `Precommit` is broadcast.
///
/// ### Generation
/// A node broadcasts `Prevote` in response to `Propose` when it has received all the transactions.
    struct Prevote {
        const TYPE = CONSENSUS;
        const ID = PREVOTE_MESSAGE_ID;
        const SIZE = 50;

        field validator:      u16         [00 => 02]
        field height:         u64         [02 => 10]
        field round:          u32         [10 => 14]
        field propose_hash:   &Hash       [14 => 46]
        field locked_round:   u32         [46 => 50]
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
/// If a proposal is unknown to the node, `RequestPropose` is sent in reply.
/// If `round` number from the message is bigger than a node's "locked round", then a node replies
/// with `RequestPrevotes`.
/// If there are unknown transactions, then `RequestTransactions` is sent in reply.
/// If a validator receives +2/3 precommits for the same proposal with the same block_hash, then
/// block is executed and `Status` is broadcast.
///
/// ### Generation
/// A node broadcasts `Precommit` in response to `Prevote` if there are +2/3 pre-votes and no
/// unknown transactions.
    struct Precommit {
        const TYPE = CONSENSUS;
        const ID = PRECOMMIT_MESSAGE_ID;
        const SIZE = 90;

        field validator:      u16         [00 => 02]
        field height:         u64         [02 => 10]
        field round:          u32         [10 => 14]
        field propose_hash:   &Hash       [14 => 46]
        field block_hash:     &Hash       [46 => 78]
        field time:           SystemTime  [78 => 90]
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
/// If the message's `height` number is bigger than a node's one, then `RequestBlock` with current
/// node's height is sent in reply.
///
/// ### Generation
/// `Status` message is broadcast regularly with the timeout controlled by
/// `blockchain::ConsensusConfig::status_timeout`. Also, it is broadcast after accepting a new
/// block.
    struct Status {
        const TYPE = CONSENSUS;
        const ID = STATUS_MESSAGE_ID;
        const SIZE = 72;

        field from:           &PublicKey          [00 => 32]
        field height:         u64                 [32 => 40]
        field last_hash:      &Hash               [40 => 72]
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
/// The message is sent as response to `RequestBlock`.
    struct Block {
        const TYPE = CONSENSUS;
        const ID = BLOCK_MESSAGE_ID;
        const SIZE = 88;

        field from:           &PublicKey          [00 => 32]
        field to:             &PublicKey          [32 => 64]
        field block:          blockchain::Block   [64 => 72]
        field precommits:     Vec<Precommit>      [72 => 80]
        field transactions:   Vec<RawMessage>     [80 => 88]
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockProof {
    pub block: blockchain::Block,
    pub precommits: Vec<Precommit>,
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
/// A node can send `RequestPropose` during `Precommit` and `Prevote` handling.
    struct RequestPropose {
        const TYPE = CONSENSUS;
        const ID = REQUEST_PROPOSE_MESSAGE_ID;
        const SIZE = 104;

        field from:           &PublicKey  [00 => 32]
        field to:             &PublicKey  [32 => 64]
        field height:         u64         [64 => 72]
        field propose_hash:   &Hash       [72 => 104]
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
    struct RequestTransactions {
        const TYPE = CONSENSUS;
        const ID = REQUEST_TRANSACTIONS_MESSAGE_ID;
        const SIZE = 72;

        field from:           &PublicKey  [00 => 32]
        field to:             &PublicKey  [32 => 64]
        field txs:            &[Hash]     [64 => 72]
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
    struct RequestPrevotes {
        const TYPE = CONSENSUS;
        const ID = REQUEST_PREVOTES_MESSAGE_ID;
        const SIZE = 116;

        field from:           &PublicKey  [00 => 32]
        field to:             &PublicKey  [32 => 64]
        field height:         u64         [64 => 72]
        field round:          u32         [72 => 76]
        field propose_hash:   &Hash       [76 => 108]
        field validators:     BitVec      [108 => 116]
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
/// `RequestPeers` message is sent regularly with the timeout controlled by
/// `blockchain::ConsensusConfig::peers_timeout`.
    struct RequestPeers {
        const TYPE = CONSENSUS;
        const ID = REQUEST_PEERS_MESSAGE_ID;
        const SIZE = 64;

        field from:           &PublicKey  [00 => 32]
        field to:             &PublicKey  [32 => 64]
    }
}

message! {
/// Request for the block with the given `height`.
///
/// ### Validation
/// The message is ignored if its `height` is bigger than the node's one.
///
/// ### Processing
/// `Block` message is sent as the response.
///
/// ### Generation
/// This message can be sent during `Status` processing.
    struct RequestBlock {
        const TYPE = CONSENSUS;
        const ID = REQUEST_BLOCK_MESSAGE_ID;
        const SIZE = 72;

        field from:           &PublicKey  [00 => 32]
        field to:             &PublicKey  [32 => 64]
        field height:         u64         [64 => 72]
    }
}
