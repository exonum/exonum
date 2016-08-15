use std::net::SocketAddr;

use super::super::crypto::{hash, Hash, gen_keypair};

use super::{Message, Connect, Propose, Prevote, Precommit, Status};

#[test]
fn test_connect() {
    use std::str::FromStr;

    let socket_address = SocketAddr::from_str("18.34.3.4:7777").unwrap();
    let time = ::time::get_time();
    let (public_key, secret_key) = gen_keypair();

    // write
    let connect = Connect::new(&public_key, socket_address, time, &secret_key);
    // read
    assert_eq!(connect.pub_key(), &public_key);
    assert_eq!(connect.addr(), socket_address);
    assert_eq!(connect.time(), time);
    assert!(connect.verify(&public_key));
}

#[test]
fn test_propose() {
    let validator = 123_123;
    let height = 123_123_123;
    let round = 321_321_312;
    let time = ::time::get_time();
    let prev_hash = hash(&[1, 2, 3]);
    let txs = vec![hash(&[1]), hash(&[2]), hash(&[2])];
    let (public_key, secret_key) = gen_keypair();

    // write
    let propose = Propose::new(validator,
                               height,
                               round,
                               time,
                               &prev_hash,
                               &txs,
                               &secret_key);
    // read
    assert_eq!(propose.validator(), validator);
    assert_eq!(propose.height(), height);
    assert_eq!(propose.round(), round);
    assert_eq!(propose.time(), time);
    assert_eq!(propose.prev_hash(), &prev_hash);
    assert_eq!(propose.transactions().len(), 3);
    assert_eq!(propose.transactions()[0], txs[0]);
    assert_eq!(propose.transactions()[1], txs[1]);
    assert_eq!(propose.transactions()[2], txs[2]);
    assert!(propose.verify(&public_key));
}

#[test]
fn test_prevote() {
    let validator = 123_123;
    let height = 123_123_123;
    let round = 321_321_312;
    let propose_hash = hash(&[1, 2, 3]);
    let locked_round = 654_345;
    let (public_key, secret_key) = gen_keypair();

    // write
    let prevote = Prevote::new(validator,
                               height,
                               round,
                               &propose_hash,
                               locked_round,
                               &secret_key);
    // read
    assert_eq!(prevote.validator(), validator);
    assert_eq!(prevote.height(), height);
    assert_eq!(prevote.round(), round);
    assert_eq!(prevote.propose_hash(), &propose_hash);
    assert_eq!(prevote.locked_round(), locked_round);
    assert!(prevote.verify(&public_key));
}

#[test]
fn test_precommit() {
    let validator = 123_123;
    let height = 123_123_123;
    let round = 321_321_312;
    let propose_hash = hash(&[1, 2, 3]);
    let block_hash = hash(&[3, 2, 1]);
    let (public_key, secret_key) = gen_keypair();

    // write
    let precommit = Precommit::new(validator,
                                   height,
                                   round,
                                   &propose_hash,
                                   &block_hash,
                                   &secret_key);
    // read
    assert_eq!(precommit.validator(), validator);
    assert_eq!(precommit.height(), height);
    assert_eq!(precommit.round(), round);
    assert_eq!(precommit.propose_hash(), &propose_hash);
    assert_eq!(precommit.block_hash(), &block_hash);
    assert!(precommit.verify(&public_key));
}

#[test]
fn test_status() {
    let validator = 123_123;
    let height = 123_123_123;
    let last_hash = hash(&[3, 2, 1]);
    let (public_key, secret_key) = gen_keypair();

    // write
    let commit = Status::new(validator, height, &last_hash, &secret_key);
    // read
    assert_eq!(commit.validator(), validator);
    assert_eq!(commit.height(), height);
    assert_eq!(commit.last_hash(), &last_hash);
    assert!(commit.verify(&public_key));
}
