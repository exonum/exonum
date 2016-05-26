use std::net::SocketAddr;
use time::Timespec;
use super::super::crypto::{Hash, PublicKey};

message! {
    Connect {
        const ID = 0;
        const SIZE = 14;

        addr:       SocketAddr  [00 => 06]
        time:       Timespec    [06 => 14]
    }
}

message! {
    Propose {
        const ID = 1;
        const SIZE = 52;

        height:     u64         [00 => 08]
        round:      u32         [08 => 12]
        time:       Timespec    [12 => 20]
        prev_hash:  &Hash       [20 => 52]
    }
}

message! {
    Prevote {
        const ID = 2;
        const SIZE = 44;

        height:     u64         [00 => 08]
        round:      u32         [08 => 12]
        hash:       &Hash       [12 => 44]
    }
}

message! {
    Precommit {
        const ID = 3;
        const SIZE = 44;

        height:     u64         [00 => 08]
        round:      u32         [08 => 12]
        hash:       &Hash       [12 => 44]
    }
}

message! {
    Commit {
        const ID = 4;
        const SIZE = 40;

        height:     u64         [00 => 08]
        hash:       &Hash       [08 => 40]
    }
}

message! {
    TxIssue {
        const ID = 5;
        const SIZE = 56;

        seed:       u64         [00 => 08]
        pub_key:    &PublicKey  [08 => 40]
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
