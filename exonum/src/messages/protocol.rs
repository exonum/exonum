use std::net::SocketAddr;
use time::Timespec;
use super::super::crypto::{Hash, PublicKey};
use super::{RawMessage, BitVec};
use super::super::blockchain;

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

pub const CONFIG_PROPOSE_MESSAGE_ID: u16 = 12;
pub const CONFIG_VOTE_MESSAGE_ID: u16 = 13;

// когда присоединяются узлы
message! {
    Connect {
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
        const ID = PRECOMMIT_MESSAGE_ID;
        const SIZE = 84;

        validator:      u32         [00 => 04]
        height:         u64         [08 => 16]
        round:          u32         [16 => 20]
        propose_hash:   &Hash       [20 => 52]
        block_hash:     &Hash       [52 => 84]
    }
}

// сообщение о текущем состоянии
message! {
    Status {
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

// запрос на получение предложения
message! {
    RequestPropose {
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
        const ID = REQUEST_BLOCK_MESSAGE_ID;
        const SIZE = 80;

        from:           &PublicKey  [00 => 32]
        to:             &PublicKey  [32 => 64]
        time:           Timespec    [64 => 72]
        height:         u64         [72 => 80]
    }
}

message! {
    ConfigPropose {
        const ID = CONFIG_PROPOSE_MESSAGE_ID;
        const SIZE = 56;

        from:           &PublicKey  [00 => 32]
        height:         u64         [32 => 40]
        config:        &[u8]        [40 => 48] // serialized config bytes
        actual_from_height: u64     [48 => 56] // с какой высоты становится актуальным
    }
}

message! {
    ConfigVote {
        const ID = CONFIG_VOTE_MESSAGE_ID;
        const SIZE = 81;

        from:           &PublicKey  [00 => 32]
        height:         u64         [32 => 40]
        hash_propose:   &Hash       [40 => 72] // hash of transacion we're voting for
        seed:           u64         [72 => 80] // incremental (1, 2, 3, 4, 5, 6, 7) проверять +1
        revoke:         bool        [80 => 81] // голос_за=false / отозвать=true
    }
}
