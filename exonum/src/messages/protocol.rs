//! Consensus messages.
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

use serde::{Serialize, Serializer, Deserialize, Deserializer};

use std::net::SocketAddr;
use std::time::SystemTime;

use messages::utils::{U64, SystemTimeSerdeHelper};
use crypto::{Hash, PublicKey, Signature};
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

/// Connect to a node.
///
/// ### Validation
/// The message is ignored if the peer is already known and `time` is earlier than in the previous
/// message.
///
/// ### Processing
/// Connect to the peer.
///
/// ### Generation
/// A node sends `Connect` message to all known addresses during initialization. Additionally,
/// the node responds by its own `Connect` message after receiving `node::Event::Connected`.
message! {
    Connect {
        const TYPE = CONSENSUS;
        const ID = CONNECT_MESSAGE_ID;
        const SIZE = 50;

        pub_key:        &PublicKey  [00 => 32]
        addr:           SocketAddr  [32 => 38]
        time:           SystemTime  [38 => 50]
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
/// If the message contains unknown transactions, then `RequestTransactions` is sent. Otherwise
/// `Prevote` is broadcast.
///
/// ### Generation
/// A node broadcasts `Propose` if it is a leader and is not locked for a different proposal. Also
/// `Propose` can be sent as response to `RequestPropose`.
message! {
    Propose {
        const TYPE = CONSENSUS;
        const ID = PROPOSE_MESSAGE_ID;
        const SIZE = 56;

        validator:      u32         [00 => 04]
        height:         u64         [04 => 12]
        round:          u32         [12 => 16]
        prev_hash:      &Hash       [16 => 48]
        transactions:   &[Hash]     [48 => 56]
    }
}

/// Pre-vote for a new block.
///
/// ### Validation
/// A node panics if it has already sent a different `Prevote` for the same round.
///
/// ### Processing
/// Pre-vote is added to the list of known votes for the same proposal.
/// If `locked_round` number from the message is bigger than in a node state, then
/// `RequestPrevotes` is sent.
/// If there are unknown transactions in the propose specified by `propose_hash`,
/// `RequestTransactions` is sent.
/// Otherwise if all transactions are known and there are +2/3 pre-votes, then a node is locked
/// to that proposal and `Precommit` is broadcast.
///
/// ### Generation
/// A node broadcasts `Prevote` in response to `Propose` when it has received all the transactions.
message! {
    Prevote {
        const TYPE = CONSENSUS;
        const ID = PREVOTE_MESSAGE_ID;
        const SIZE = 52;

        validator:      u32         [00 => 04]
        height:         u64         [04 => 12]
        round:          u32         [12 => 16]
        propose_hash:   &Hash       [16 => 48]
        locked_round:   u32         [48 => 52]
    }
}

/// Pre-commit for a proposal.
///
/// ### Validation
/// A node panics if it  has already sent a different `Precommit` for the same round.
///
/// ### Processing
/// Pre-commit is added to the list of known pre-commits.
/// If a proposal is unknown to the node, `RequestPropose` is sent.
/// If `round` number from the message is bigger than a node's "locked round", then
/// `RequestPrevotes` is sent.
/// If there are unknown transactions, then `RequestTransactions` is sent.
/// If a validator receives +2/3 precommits for the same proposal with the same block_hash, then
/// block is executed and `Status` is broadcast.
///
/// ### Generation
/// A node broadcasts `Precommit` in response to `Prevote` if there are +2/3 pre-votes and no
/// unknown transactions.
message! {
    Precommit {
        const TYPE = CONSENSUS;
        const ID = PRECOMMIT_MESSAGE_ID;
        const SIZE = 96;

        validator:      u32         [00 => 04]
        height:         u64         [08 => 16]
        round:          u32         [16 => 20]
        propose_hash:   &Hash       [20 => 52]
        block_hash:     &Hash       [52 => 84]
        time:           SystemTime  [84 => 96]
    }
}

#[derive(Serialize, Deserialize)]
struct PrecommitSerdeHelper {
    body: PrecommitBodySerdeHelper,
    signature: Signature,
}

#[derive(Serialize, Deserialize)]
struct PrecommitBodySerdeHelper {
   validator: u32,
   height: U64,
   round: u32,
   propose_hash: Hash,
   block_hash: Hash,
   time: SystemTimeSerdeHelper,
}

impl Serialize for Precommit {
    fn serialize<S>(&self, ser: &mut S) -> Result<(), S::Error>
        where S: Serializer
    {
        let body = PrecommitBodySerdeHelper{
            validator: self.validator(),
            height: U64(self.height()),
            round: self.round(),
            propose_hash: *self.propose_hash(),
            block_hash: *self.block_hash(),
            time: SystemTimeSerdeHelper(self.time()),
        };
        let helper = PrecommitSerdeHelper {
            body: body,
            signature: *self.raw.signature(),
        };
        helper.serialize(ser)
    }
}

impl Deserialize for Precommit {
    fn deserialize<D>(deserializer: &mut D) -> Result<Self, D::Error>
        where D: Deserializer
    {
        let h = <PrecommitSerdeHelper>::deserialize(deserializer)?;

        let precommit = Precommit::new_with_signature(h.body.validator, h.body.height.0, h.body.round, &h.body.propose_hash, &h.body.block_hash, h.body.time.0, &h.signature);
        Ok(precommit)
    }
}

/// Current node status.
///
/// ### Validation
/// The message is ignored if its signature is incorrect or its `height` is lower than a node's
/// height.
///
/// ### Processing
/// If the message's `height` number is bigger than a node's one, then `RequestBlock` with current
/// node's height is sent.
///
/// ### Generation
/// `Status` message is broadcast regularly with the timeout controlled by
/// `blockchain::ConsensusConfig::status_timeout`. Also, it is broadcast after accepting a new block.
message! {
    Status {
        const TYPE = CONSENSUS;
        const ID = STATUS_MESSAGE_ID;
        const SIZE = 72;

        from:           &PublicKey          [00 => 32]
        height:         u64                 [32 => 40]
        last_hash:      &Hash               [40 => 72]
    }
}

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
message! {
    Block {
        const TYPE = CONSENSUS;
        const ID = BLOCK_MESSAGE_ID;
        const SIZE = 88;

        from:           &PublicKey          [00 => 32]
        to:             &PublicKey          [32 => 64]
        block:          blockchain::Block   [64 => 72]
        precommits:     Vec<Precommit>      [72 => 80]
        transactions:   Vec<RawMessage>     [80 => 88]
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockProof {
    pub block: blockchain::Block,
    pub precommits: Vec<Precommit>,
}

/// Request for the `Propose`.
///
/// ### Validation
/// The message is ignored if its `height` is not equal to the node's height.
///
/// ### Processing
/// `Propose` is sent as the response.
///
/// ### Generation
/// A node can send `RequestPropose` during `Precommit` handling.
message! {
    RequestPropose {
        const TYPE = CONSENSUS;
        const ID = REQUEST_PROPOSE_MESSAGE_ID;
        const SIZE = 104;

        from:           &PublicKey  [00 => 32]
        to:             &PublicKey  [32 => 64]
        height:         u64         [64 => 72]
        propose_hash:   &Hash       [72 => 104]
    }
}

/// Request for transactions by hash.
///
/// ### Processing
/// Requested transactions are sent to the recipient.
///
/// ### Generation
/// This message can be sent during `Propose`, `Prevote` and `Precommit` handling.
message! {
    RequestTransactions {
        const TYPE = CONSENSUS;
        const ID = REQUEST_TRANSACTIONS_MESSAGE_ID;
        const SIZE = 72;

        from:           &PublicKey  [00 => 32]
        to:             &PublicKey  [32 => 64]
        txs:            &[Hash]     [64 => 72]
    }
}

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
message! {
    RequestPrevotes {
        const TYPE = CONSENSUS;
        const ID = REQUEST_PREVOTES_MESSAGE_ID;
        const SIZE = 116;

        from:           &PublicKey  [00 => 32]
        to:             &PublicKey  [32 => 64]
        height:         u64         [64 => 72]
        round:          u32         [72 => 76]
        propose_hash:   &Hash       [76 => 108]
        validators:     BitVec      [108 => 116]
    }
}

/// Request connected peers from a node.
///
/// ### Validation
/// Response is sent only if the node is connected to the sender.
///
/// ### Processing
/// Peer `Connect` messages are sent to the recipient.
///
/// ### Generation
/// `RequestPeers` message is sent regularly with the timeout controlled by
/// `blockchain::ConsensusConfig::peers_timeout`.
message! {
    RequestPeers {
        const TYPE = CONSENSUS;
        const ID = REQUEST_PEERS_MESSAGE_ID;
        const SIZE = 64;

        from:           &PublicKey  [00 => 32]
        to:             &PublicKey  [32 => 64]
    }
}

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
message! {
    RequestBlock {
        const TYPE = CONSENSUS;
        const ID = REQUEST_BLOCK_MESSAGE_ID;
        const SIZE = 72;

        from:           &PublicKey  [00 => 32]
        to:             &PublicKey  [32 => 64]
        height:         u64         [64 => 72]
    }
}
