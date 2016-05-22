use std::net::SocketAddr;

use time::Timespec;

use super::message::{RawMessage, ProtocolMessage};
use super::crypto::Hash;

pub enum Any {
    Connect(Connect),
    Propose(Propose),
    Prevote(Prevote),
    Precommit(Precommit),
    Commit(Commit),
}

impl Any {
    pub fn from_raw(raw: RawMessage) -> Any {
        // TODO: check input message size
        match raw.message_type() {
            Connect::MESSAGE_TYPE => Any::Connect(Connect::from_raw(raw)),
            Propose::MESSAGE_TYPE => Any::Propose(Propose::from_raw(raw)),
            Prevote::MESSAGE_TYPE => Any::Prevote(Prevote::from_raw(raw)),
            Precommit::MESSAGE_TYPE => Any::Precommit(Precommit::from_raw(raw)),
            Commit::MESSAGE_TYPE => Any::Commit(Commit::from_raw(raw)),
            _ => {
                // TODO: use result here
                panic!("unrecognized message type");
            }
        }
    }
}

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

#[test]
fn test_connect() {
    use std::str::FromStr;

    let socket_address = SocketAddr::from_str("18.34.3.4:7777").unwrap();
    let time = ::time::get_time();
    let (public_key, secret_key) = super::crypto::gen_keypair();

    // write
    let connect = Connect::new(socket_address.clone(), time,
                               &public_key, &secret_key);
    // read
    assert_eq!(connect.addr(), socket_address);
    assert_eq!(connect.time(), time);
    assert!(connect.verify());
}

#[test]
fn test_propose() {
    let height = 123_123_123;
    let round = 321_321_312;
    let time = ::time::get_time();
    let prev_hash = super::crypto::hash(&[1, 2, 3]);
    let (public_key, secret_key) = super::crypto::gen_keypair();

    // write
    let propose = Propose::new(height, round, time, &prev_hash,
                               &public_key, &secret_key);
    // read
    assert_eq!(propose.height(), height);
    assert_eq!(propose.round(), round);
    assert_eq!(propose.time(), time);
    assert_eq!(propose.prev_hash(), &prev_hash);
    assert!(propose.verify());
}

#[test]
fn test_prevote() {
    let height = 123_123_123;
    let round = 321_321_312;
    let hash = super::crypto::hash(&[1, 2, 3]);
    let (public_key, secret_key) = super::crypto::gen_keypair();

    // write
    let prevote = Prevote::new(height, round, &hash, &public_key, &secret_key);
    // read
    assert_eq!(prevote.height(), height);
    assert_eq!(prevote.round(), round);
    assert_eq!(prevote.hash(), &hash);
    assert!(prevote.verify());
}

#[test]
fn test_precommit() {
    let height = 123_123_123;
    let round = 321_321_312;
    let hash = super::crypto::hash(&[1, 2, 3]);
    let (public_key, secret_key) = super::crypto::gen_keypair();

    // write
    let precommit = Precommit::new(height, round, &hash,
                                   &public_key, &secret_key);
    // read
    assert_eq!(precommit.height(), height);
    assert_eq!(precommit.round(), round);
    assert_eq!(precommit.hash(), &hash);
    assert!(precommit.verify());
}

#[test]
fn test_commit() {
    let height = 123_123_123;
    let hash = super::crypto::hash(&[1, 2, 3]);
    let (public_key, secret_key) = super::crypto::gen_keypair();

    // write
    let commit = Commit::new(height, &hash, &public_key, &secret_key);
    // read
    assert_eq!(commit.height(), height);
    assert_eq!(commit.hash(), &hash);
    assert!(commit.verify());
}

