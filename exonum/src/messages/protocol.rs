use std::net::SocketAddr;
use time::Timespec;
use super::super::crypto::{Hash, PublicKey, Signature};
use super::{RawMessage, BitVec};
use super::super::blockchain;
use serde::{Serialize, Serializer, Deserialize, Deserializer};
use ::messages::utils::{U64}; 

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
pub const REQUEST_PRECOMMITS_MESSAGE_ID: u16 = 9;
pub const REQUEST_PEERS_MESSAGE_ID: u16 = 10;
pub const REQUEST_BLOCK_MESSAGE_ID: u16 = 11;

// когда присоединяются узлы
message! {
    Connect {
        const TYPE = CONSENSUS;
        const ID = CONNECT_MESSAGE_ID;
        const SIZE = 46;

        pub_key:        &PublicKey  [00 => 32]
        addr:           SocketAddr  [32 => 38]
        time:           Timespec    [38 => 46]
    }
}

// консенсус
message! {
    Propose {
        const TYPE = CONSENSUS;
        const ID = PROPOSE_MESSAGE_ID;
        const SIZE = 64;

        validator:      u32         [00 => 04]
        height:         u64         [04 => 12]
        round:          u32         [12 => 16]
        time:           Timespec    [16 => 24]
        prev_hash:      &Hash       [24 => 56]
        transactions:   &[Hash]     [56 => 64]
    }
}

// консенсус
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

// консенсус
message! {
    Precommit {
        const TYPE = CONSENSUS;
        const ID = PRECOMMIT_MESSAGE_ID;
        const SIZE = 84;

        validator:      u32         [00 => 04]
        height:         u64         [08 => 16]
        round:          u32         [16 => 20]
        propose_hash:   &Hash       [20 => 52]
        block_hash:     &Hash       [52 => 84]
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

        let precommit = Precommit::new_with_signature(h.body.validator, h.body.height.0, h.body.round, &h.body.propose_hash, &h.body.block_hash, &h.signature); 
        Ok(precommit)
    }
}

// сообщение о текущем состоянии
message! {
    Status {
        const TYPE = CONSENSUS;
        const ID = STATUS_MESSAGE_ID;
        const SIZE = 44;

        validator:      u32         [00 => 04]
        height:         u64         [04 => 12]
        last_hash:      &Hash       [12 => 44]
    }
}

// ответ на requestblock
message! {
    Block {
        const TYPE = CONSENSUS;
        const ID = BLOCK_MESSAGE_ID;
        const SIZE = 96;

        from:           &PublicKey          [00 => 32]
        to:             &PublicKey          [32 => 64]
        time:           Timespec            [64 => 72]
        block:          blockchain::Block   [72 => 80]
        precommits:     Vec<Precommit>      [80 => 88]
        transactions:   Vec<RawMessage>     [88 => 96]
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockProof {
    pub block: blockchain::Block,
    pub precommits: Vec<Precommit>,
}

// запрос на получение предложения
message! {
    RequestPropose {
        const TYPE = CONSENSUS;
        const ID = REQUEST_PROPOSE_MESSAGE_ID;
        const SIZE = 112;

        from:           &PublicKey  [00 => 32]
        to:             &PublicKey  [32 => 64]
        time:           Timespec    [64 => 72] // текущее время, seed + ttl
        height:         u64         [72 => 80]
        propose_hash:   &Hash       [80 => 112]
    }
}

// запрос транзакций по списку hash
message! {
    RequestTransactions {
        const TYPE = CONSENSUS;
        const ID = REQUEST_TRANSACTIONS_MESSAGE_ID;
        const SIZE = 80;

        from:           &PublicKey  [00 => 32]
        to:             &PublicKey  [32 => 64]
        time:           Timespec    [64 => 72]
        txs:            &[Hash]     [72 => 80]
    }
}

// запрос prevotes
message! {
    RequestPrevotes {
        const TYPE = CONSENSUS;
        const ID = REQUEST_PREVOTES_MESSAGE_ID;
        const SIZE = 124;

        from:           &PublicKey  [00 => 32]
        to:             &PublicKey  [32 => 64]
        time:           Timespec    [64 => 72]
        height:         u64         [72 => 80]
        round:          u32         [80 => 84]
        propose_hash:   &Hash       [84 => 116]
        validators:     BitVec      [116 => 124]
    }
}
// запрос прекоммитов
message! {
    RequestPrecommits {
        const TYPE = CONSENSUS;
        const ID = REQUEST_PRECOMMITS_MESSAGE_ID;
        const SIZE = 156;

        from:           &PublicKey  [00 => 32]
        to:             &PublicKey  [32 => 64]
        time:           Timespec    [64 => 72]
        height:         u64         [72 => 80]
        round:          u32         [80 => 84]
        propose_hash:   &Hash       [84 => 116]
        block_hash:     &Hash       [116 => 148]
        validators:     BitVec      [148 => 156]
    }
}

// запрос узлов с которыми соединён
message! {
    RequestPeers {
        const TYPE = CONSENSUS;
        const ID = REQUEST_PEERS_MESSAGE_ID;
        const SIZE = 72;

        from:           &PublicKey  [00 => 32]
        to:             &PublicKey  [32 => 64]
        time:           Timespec    [64 => 72]
    }
}
// запрос блоков
message! {
    RequestBlock {
        const TYPE = CONSENSUS;
        const ID = REQUEST_BLOCK_MESSAGE_ID;
        const SIZE = 80;

        from:           &PublicKey  [00 => 32]
        to:             &PublicKey  [32 => 64]
        time:           Timespec    [64 => 72]
        height:         u64         [72 => 80]
    }
}
