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

// Node connection
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

// Consensus
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

// Consensus
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

// Consensus
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

// Message with current state.
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

// Response to `RequestBlock`.
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

// `Propose` request.
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

// Request for transactions by hash.
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

// Prevotes request.
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

/// Request connected peers from the node `to`.
///
/// ### Processing
///   * The message is authenticated by the pubkey `from`.
///     It must be in the receiver's full node list
///   * If the message is properly authorized, the node responds with...
///
/// ### Generation
/// A node generates `RequestPeers` under such and such conditions...
message! {
    RequestPeers {
        const TYPE = CONSENSUS;
        const ID = REQUEST_PEERS_MESSAGE_ID;
        const SIZE = 64;

        from:           &PublicKey  [00 => 32]
        to:             &PublicKey  [32 => 64]
    }
}

// `Block` request.
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
