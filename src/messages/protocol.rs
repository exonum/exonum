use std::net::SocketAddr;
use time::Timespec;
use super::super::crypto::{Hash, PublicKey};

message! {
    Connect {
        const ID = 0;
        const SIZE = 46;

        pub_key:    &PublicKey  [00 => 32]
        addr:       SocketAddr  [32 => 38]
        time:       Timespec    [38 => 46]
    }
}

message! {
    Propose {
        const ID = 1;
        const SIZE = 56;

        validator:  u32         [00 => 04]
        height:     u64         [04 => 12]
        round:      u32         [12 => 16]
        time:       Timespec    [16 => 24]
        prev_hash:  &Hash       [24 => 56]
    }
}

message! {
    Prevote {
        const ID = 2;
        const SIZE = 48;

        validator:  u32         [00 => 04]
        height:     u64         [04 => 12]
        round:      u32         [12 => 16]
        hash:       &Hash       [16 => 48]
    }
}

message! {
    Precommit {
        const ID = 3;
        const SIZE = 48;

        validator:  u32         [00 => 04]
        height:     u64         [04 => 12]
        round:      u32         [12 => 16]
        hash:       &Hash       [16 => 48]
    }
}

message! {
    Commit {
        const ID = 4;
        const SIZE = 44;

        validator:  u32         [00 => 04]
        height:     u64         [04 => 12]
        hash:       &Hash       [12 => 44]
    }
}

message! {
    TxIssue {
        const ID = 5;
        const SIZE = 56;

        seed:       u64         [00 => 08]
        asset:      &PublicKey  [08 => 40]
        name:       &str        [40 => 48]
        amount:     u64         [48 => 56]
    }
}

message! {
    TxTransfer {
        const ID = 6;
        const SIZE = 80;

        seed:       u64         [00 => 08]
        from:       &PublicKey  [00 => 40]
        to:         &PublicKey  [40 => 72]
        amount:     u64         [78 => 80]
    }
}

message! {
    TxVoteValidator {
        const ID = 7;
        const SIZE = 48;

        seed:       u64         [00 => 08]
        pub_key:    &PublicKey  [08 => 40]
        power:      u64         [40 => 48]
    }
}

message! {
    TxVoteConfiguration {
        const ID = 8;
        const SIZE = 24;

        seed:       u64         [00 => 08]
        key:        &[u8]       [08 => 16]
        value:      &[u8]       [16 => 24]
    }
}
