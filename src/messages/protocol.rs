use std::net::SocketAddr;
use time::Timespec;
use super::super::crypto::Hash;

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

// message! {
//     TxIssue {
//         const ID = 5;
//         const SIZE = 40;

//     }
// }

// message! {
//     TxTransfer {
//         const ID = 6;
//         const SIZE = 40;

//     }
// }

// message! {
//     TxVoteValidator {
//         const ID = 6;
//         const SIZE = 40;
//     }
// }

// message! {
//     TxVoteConfiguration {
//         const ID = 6;
//         const SIZE = 40;
//     }
// }
