use std::net::SocketAddr;
use time::Timespec;
use super::super::crypto::{Hash, PublicKey};

message! {
    Connect {
        const ID = 0;
        const SIZE = 46;

        pub_key:        &PublicKey  [00 => 32]
        addr:           SocketAddr  [32 => 38]
        time:           Timespec    [38 => 46]
    }
}

message! {
    Propose {
        const ID = 1;
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
        const ID = 2;
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
        const ID = 3;
        const SIZE = 80;

        validator:      u32         [00 => 04]
        height:         u64         [04 => 12]
        round:          u32         [12 => 16]
        propose_hash:   &Hash       [16 => 48]
        block_hash:     &Hash       [48 => 80]
    }
}

message! {
    Commit {
        const ID = 4;
        const SIZE = 80;

        validator:      u32         [00 => 04]
        height:         u64         [04 => 12]
        round:          u32         [12 => 16]
        propose_hash:   &Hash       [16 => 48]
        block_hash:     &Hash       [48 => 80]
    }
}

// TODO: видимо поля `from` и `to` должны быть PublicKey, поскольку узлы
// могут находиться на разных высотах и вообще не быть валидаторами

message! {
    RequestPropose {
        const ID = 5;
        const SIZE = 56;

        from:           u32         [00 => 04]
        to:             u32         [04 => 08]
        time:           Timespec    [08 => 16]
        height:         u64         [16 => 24]
        propose_hash:   &Hash       [24 => 56]
    }
}

message! {
    RequestTransactions {
        const ID = 6;
        const SIZE = 24;

        from:           u32         [00 => 04]
        to:             u32         [04 => 08]
        time:           Timespec    [08 => 16]
        txs:            &[Hash]     [16 => 24]
    }
}

message! {
    RequestPrevotes {
        const ID = 7;
        const SIZE = 60;

        from:           u32         [00 => 04]
        to:             u32         [04 => 08]
        time:           Timespec    [08 => 16]
        height:         u64         [16 => 24]
        round:          u32         [24 => 28]
        propose_hash:   &Hash       [28 => 60]
        // validators:     &Bitset     [60 => 68]
    }
}

message! {
    RequestPrecommits {
        const ID = 8;
        const SIZE = 92;

        from:           u32         [00 => 04]
        to:             u32         [04 => 08]
        time:           Timespec    [08 => 16]
        height:         u64         [16 => 24]
        round:          u32         [24 => 28]
        propose_hash:   &Hash       [28 => 60]
        block_hash:     &Hash       [60 => 92]
        // validators:     &Bitset     [60 => 68]
    }
}

message! {
    RequestCommit {
        const ID = 9;
        const SIZE = 24;

        from:           u32         [00 => 04]
        to:             u32         [04 => 08]
        time:           Timespec    [08 => 16]
        height:         u64         [16 => 24]
    }
}

message! {
    RequestPeers {
        const ID = 10;
        const SIZE = 16;

        from:           u32         [00 => 04]
        to:             u32         [04 => 08]
        time:           Timespec    [08 => 16]
    }
}

message! {
    TxIssue {
        const ID = 11;
        const SIZE = 56;

        seed:           u64         [00 => 08]
        asset:          &PublicKey  [08 => 40]
        name:           &str        [40 => 48]
        amount:         u64         [48 => 56]
    }
}

message! {
    TxTransfer {
        const ID = 12;
        const SIZE = 80;

        seed:           u64         [00 => 08]
        from:           &PublicKey  [08 => 40]
        to:             &PublicKey  [40 => 72]
        amount:         u64         [72 => 80]
    }
}

message! {
    TxVoteValidator {
        const ID = 13;
        const SIZE = 48;

        seed:           u64         [00 => 08]
        pub_key:        &PublicKey  [08 => 40]
        power:          u64         [40 => 48]
    }
}

message! {
    TxVoteConfig {
        const ID = 14;
        const SIZE = 24;

        seed:           u64         [00 => 08]
        key:            &[u8]       [08 => 16]
        value:          &[u8]       [16 => 24]
    }
}
