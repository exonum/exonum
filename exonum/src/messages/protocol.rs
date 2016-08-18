use std::net::SocketAddr;
use time::Timespec;
use super::super::crypto::{Hash, PublicKey};

pub const CONNECT_MESSAGE_ID: u16 = 0;
pub const STATUS_MESSAGE_ID: u16 = 1;

pub const PROPOSE_MESSAGE_ID: u16 = 2;
pub const PREVOTE_MESSAGE_ID: u16 = 3;
pub const PRECOMMIT_MESSAGE_ID: u16 = 4;

pub const REQUEST_PROPOSE_MESSAGE_ID: u16 = 5;
pub const REQUEST_TRANSACTIONS_MESSAGE_ID: u16 = 6;
pub const REQUEST_PREVOTES_MESSAGE_ID: u16 = 7;
pub const REQUEST_PRECOMMITS_MESSAGE_ID: u16 = 8;
pub const REQUEST_COMMIT_MESSAGE_ID: u16 = 9;
pub const REQUEST_PEERS_MESSAGE_ID: u16 = 10;

message! {
    Connect {
        const ID = CONNECT_MESSAGE_ID;
        const SIZE = 46;

        pub_key:        &PublicKey  [00 => 32]
        addr:           SocketAddr  [32 => 38]
        time:           Timespec    [38 => 46]
    }
}

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

message! {
    Precommit {
        const ID = PRECOMMIT_MESSAGE_ID;
        const SIZE = 80;

        validator:      u32         [00 => 04]
        height:         u64         [04 => 12]
        round:          u32         [12 => 16]
        propose_hash:   &Hash       [16 => 48]
        block_hash:     &Hash       [48 => 80]
    }
}

message! {
    Status {
        const ID = STATUS_MESSAGE_ID;
        const SIZE = 44;

        validator:      u32         [00 => 04]
        height:         u64         [04 => 12]
        last_hash:      &Hash       [12 => 44]
    }
}


message! {
    RequestPropose {
        const ID = REQUEST_PROPOSE_MESSAGE_ID;
        const SIZE = 112;

        from:           &PublicKey  [00 => 32]
        to:             &PublicKey  [32 => 64]
        time:           Timespec    [64 => 72]
        height:         u64         [72 => 80]
        propose_hash:   &Hash       [80 => 112]
    }
}

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

message! {
    RequestPrevotes {
        const ID = REQUEST_PREVOTES_MESSAGE_ID;
        const SIZE = 116;

        from:           &PublicKey  [00 => 32]
        to:             &PublicKey  [32 => 64]
        time:           Timespec    [64 => 72]
        height:         u64         [72 => 80]
        round:          u32         [80 => 84]
        propose_hash:   &Hash       [84 => 116]
        // validators:     &Bitset     [60 => 68]
    }
}

message! {
    RequestPrecommits {
        const ID = REQUEST_PRECOMMITS_MESSAGE_ID;
        const SIZE = 148;

        from:           &PublicKey  [00 => 32]
        to:             &PublicKey  [32 => 64]
        time:           Timespec    [64 => 72]
        height:         u64         [72 => 80]
        round:          u32         [80 => 84]
        propose_hash:   &Hash       [84 => 116]
        block_hash:     &Hash       [116 => 148]
        // validators:     &Bitset     [60 => 68]
    }
}

message! {
    RequestCommit {
        const ID = REQUEST_COMMIT_MESSAGE_ID;
        const SIZE = 80;

        from:           &PublicKey  [00 => 32]
        to:             &PublicKey  [32 => 64]
        time:           Timespec    [64 => 72]
        height:         u64         [72 => 80]
    }
}

message! {
    RequestPeers {
        const ID = REQUEST_PEERS_MESSAGE_ID;
        const SIZE = 72;

        from:           &PublicKey  [00 => 32]
        to:             &PublicKey  [32 => 64]
        time:           Timespec    [64 => 72]
    }
}

// message! {
//     TxIssue//{
//         const ID = 11;
//         const SIZE = 56;

//         seed:           u64         [00 => 08]
//         asset:          &PublicKey  [08 => 40]
//         name:           &str        [40 => 48]
//         amount:         u64         [48 => 56]
//     }
// }

// message! {
//     TxTransfer//{
//         const ID = 12;
//         const SIZE = 80;

//         seed:           u64         [00 => 08]
//         from:           &PublicKey  [08 => 40]
//         to:             &PublicKey  [40 => 72]
//         amount:         u64         [72 => 80]
//     }
// }

// message! {
//     TxVoteValidator//{
//         const ID = 13;
//         const SIZE = 48;

//         seed:           u64         [00 => 08]
//         pub_key:        &PublicKey  [08 => 40]
//         power:          u64         [40 => 48]
//     }
// }

// message! {
//     TxVoteConfig//{
//         const ID = 14;
//         const SIZE = 24;

//         seed:           u64         [00 => 08]
//         key:            &[u8]       [08 => 16]
//         value:          &[u8]       [16 => 24]
//     }
// }
