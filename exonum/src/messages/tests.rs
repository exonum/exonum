use serde_json;

use std::net::SocketAddr;
use std::time::SystemTime;

use crypto::{hash, gen_keypair};
use blockchain;

use super::{Field, RawMessage, Message, FromRaw, Connect, Propose, Prevote, Precommit, Status,
            Block, BlockProof, RequestBlock, BitVec};

#[test]
fn test_bitvec() {
    // TODO Think about BitVec len
    // now if the size of the BitVec is not a multiple of eight
    // then trailing bits will be filled-in with false.

    let mut b = BitVec::from_elem(14, false);
    b.set(11, true);
    b.set(4, true);
    b.push(true);
    b.push(true);

    let mut buf = vec![0; 8];
    Field::write(&b, &mut buf, 0, 8);
    <BitVec as Field>::check(&buf, 0, 8).unwrap();

    let buf2 = buf.clone();
    <BitVec as Field>::check(&buf2, 0, 8).unwrap();
    let b2: BitVec = Field::read(&buf2, 0, 8);

    assert_eq!(b2.len(), b.len());
    assert_eq!(b2, b);
}

#[test]
fn test_str_segment() {
    let mut buf = vec![0; 8];
    let s = "test юникодной строчки efw_adqq ss/adfq";
    Field::write(&s, &mut buf, 0, 8);
    <&str as Field>::check(&buf, 0, 8).unwrap();

    let buf2 = buf.clone();
    <&str as Field>::check(&buf2, 0, 8).unwrap();
    let s2: &str = Field::read(&buf2, 0, 8);
    assert_eq!(s2, s);
}

#[test]
fn test_vec_segment() {
    let mut buf = vec![0; 8];
    let v = vec![1, 2, 3, 5, 10];
    Field::write(&v, &mut buf, 0, 8);
    <Vec<u8> as Field>::check(&buf, 0, 8).unwrap();

    let buf2 = buf.clone();
    <Vec<u8> as Field>::check(&buf2, 0, 8).unwrap();
    let v2: Vec<u8> = Field::read(&buf2, 0, 8);
    assert_eq!(v2, v);
}

#[test]
fn test_u16_segment() {
    let mut buf = vec![0; 8];
    let s = [1u16, 3, 10, 15, 23, 4, 45];
    Field::write(&s.as_ref(), &mut buf, 0, 8);
    <&[u16] as Field>::check(&buf, 0, 8).unwrap();

    let buf2 = buf.clone();
    <&[u16] as Field>::check(&buf2, 0, 8).unwrap();
    let s2: &[u16] = Field::read(&buf2, 0, 8);
    assert_eq!(s2, s.as_ref());
}

#[test]
fn test_u32_segment() {
    let mut buf = vec![0; 8];
    let s = [1u32, 3, 10, 15, 23, 4, 45];
    Field::write(&s.as_ref(), &mut buf, 0, 8);
    <&[u32] as Field>::check(&buf, 0, 8).unwrap();

    let buf2 = buf.clone();
    <&[u32] as Field>::check(&buf2, 0, 8).unwrap();
    let s2: &[u32] = Field::read(&buf2, 0, 8);
    assert_eq!(s2, s.as_ref());
}

#[test]
fn test_segments_of_segments() {
    let mut buf = vec![255; 64];
    let v1 = [1u8, 2, 3];
    let v2 = [1u8, 3];
    let v3 = [2u8, 5, 2, 3, 56, 3];

    let dat = vec![v1.as_ref(), v2.as_ref(), v3.as_ref()];
    Field::write(&dat, &mut buf, 48, 56);
    <Vec<&[u8]> as Field>::check(&buf, 48, 56).unwrap();

    let buf2 = buf.clone();
    <Vec<&[u8]> as Field>::check(&buf2, 48, 56).unwrap();
    let dat2: Vec<&[u8]> = Field::read(&buf2, 48, 56);
    assert_eq!(dat2, dat);
    assert_eq!(buf.len(), 64 + v1.len() + v2.len() + v3.len() + 24);
}

#[test]
fn test_segments_of_raw_messages() {
    let (pub_key, sec_key) = gen_keypair();

    let mut buf = vec![255; 8];
    let m1 = Status::new(&pub_key, 2, &hash(&[]), &sec_key);
    let m2 = Status::new(&pub_key, 4, &hash(&[1]), &sec_key);
    let m3 = Status::new(&pub_key, 5, &hash(&[3]), &sec_key);

    let dat = vec![m1.raw().clone(), m2.raw().clone(), m3.raw().clone()];
    Field::write(&dat, &mut buf, 0, 8);
    <Vec<RawMessage> as Field>::check(&buf, 0, 8).unwrap();

    let buf2 = buf.clone();
    <Vec<RawMessage> as Field>::check(&buf2, 0, 8).unwrap();
    let dat2: Vec<RawMessage> = Field::read(&buf2, 0, 8);
    assert_eq!(dat2, dat);
}

#[test]
fn test_empty_segments() {
    let mut buf = vec![255; 8];
    let dat: Vec<RawMessage> = vec![];
    Field::write(&dat, &mut buf, 0, 8);
    <Vec<RawMessage> as Field>::check(&buf, 0, 8).unwrap();

    let buf2 = buf.clone();
    <Vec<RawMessage> as Field>::check(&buf2, 0, 8).unwrap();
    let dat2: Vec<RawMessage> = Field::read(&buf2, 0, 8);
    assert_eq!(dat2, dat);
}

#[test]
fn test_segments_of_status_messages() {
    let (pub_key, sec_key) = gen_keypair();

    let mut buf = vec![255; 8];
    let m1 = Status::new(&pub_key, 2, &hash(&[]), &sec_key);
    let m2 = Status::new(&pub_key, 4, &hash(&[1]), &sec_key);
    let m3 = Status::new(&pub_key, 5, &hash(&[3]), &sec_key);

    let dat = vec![m1, m2, m3];
    Field::write(&dat, &mut buf, 0, 8);
    <Vec<Status> as Field>::check(&buf, 0, 8).unwrap();

    let buf2 = buf.clone();
    <Vec<Status> as Field>::check(&buf2, 0, 8).unwrap();
    let dat2: Vec<Status> = Field::read(&buf2, 0, 8);
    assert_eq!(dat2, dat);
}

#[test]
fn test_connect() {
    use std::str::FromStr;

    let socket_address = SocketAddr::from_str("18.34.3.4:7777").unwrap();
    let time = SystemTime::now();
    let (public_key, secret_key) = gen_keypair();

    // write
    let connect = Connect::new(&public_key, socket_address, time, &secret_key);
    // read
    assert_eq!(connect.pub_key(), &public_key);
    assert_eq!(connect.addr(), socket_address);
    assert_eq!(connect.time(), time);
    assert!(connect.verify_signature(&public_key));
}

#[test]
fn test_propose() {
    let validator = 123_123;
    let height = 123_123_123;
    let round = 321_321_312;
    let prev_hash = hash(&[1, 2, 3]);
    let txs = vec![hash(&[1]), hash(&[2]), hash(&[2])];
    let (public_key, secret_key) = gen_keypair();

    // write
    let propose = Propose::new(validator,
                               height,
                               round,
                               &prev_hash,
                               &txs,
                               &secret_key);
    // read
    assert_eq!(propose.validator(), validator);
    assert_eq!(propose.height(), height);
    assert_eq!(propose.round(), round);
    assert_eq!(propose.prev_hash(), &prev_hash);
    assert_eq!(propose.transactions().len(), 3);
    assert_eq!(propose.transactions()[0], txs[0]);
    assert_eq!(propose.transactions()[1], txs[1]);
    assert_eq!(propose.transactions()[2], txs[2]);
    assert!(propose.verify_signature(&public_key));
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
    assert!(prevote.verify_signature(&public_key));
}

#[test]
fn test_precommit() {
    let validator = 123_123;
    let height = 123_123_123;
    let round = 321_321_312;
    let propose_hash = hash(&[1, 2, 3]);
    let block_hash = hash(&[3, 2, 1]);
    let (public_key, secret_key) = gen_keypair();
    let time = SystemTime::now();

    // write
    let precommit = Precommit::new(validator,
                                   height,
                                   round,
                                   &propose_hash,
                                   &block_hash,
                                   time,
                                   &secret_key);
    // read
    assert_eq!(precommit.validator(), validator);
    assert_eq!(precommit.height(), height);
    assert_eq!(precommit.round(), round);
    assert_eq!(precommit.propose_hash(), &propose_hash);
    assert_eq!(precommit.block_hash(), &block_hash);
    assert!(precommit.verify_signature(&public_key));
    assert_eq!(precommit.time(), time);
    let json_str = ::serialize::json::to_string(&precommit).unwrap();
    println!("{}", json_str);
    let precommit1 : Precommit = ::serialize::json::from_str(&json_str).unwrap(); 
    assert_eq!(precommit, precommit1);
}

#[test]
fn test_status() {
    let height = 123_123_123;
    let last_hash = hash(&[3, 2, 1]);
    let (public_key, secret_key) = gen_keypair();

    // write
    let commit = Status::new(&public_key, height, &last_hash, &secret_key);
    // read
    assert_eq!(commit.from(), &public_key);
    assert_eq!(commit.height(), height);
    assert_eq!(commit.last_hash(), &last_hash);
    assert!(commit.verify_signature(&public_key));
}

#[test]
fn test_block() {
    let (pub_key, secret_key) = gen_keypair();
    let ts = SystemTime::now();

    let content = blockchain::Block::new(500,
                                         1,
                                         &hash(&[1]),
                                         &hash(&[2]),
                                         &hash(&[3]));

    let precommits = vec![Precommit::new(123,
                                         15,
                                         25,
                                         &hash(&[1, 2, 3]),
                                         &hash(&[3, 2, 1]),
                                         ts,
                                         &secret_key),
                          Precommit::new(13,
                                         25,
                                         35,
                                         &hash(&[4, 2, 3]),
                                         &hash(&[3, 3, 1]),
                                         ts,
                                         &secret_key),
                          Precommit::new(323,
                                         15,
                                         25,
                                         &hash(&[1, 1, 3]),
                                         &hash(&[5, 2, 1]),
                                         ts,
                                         &secret_key)];
    let transactions = vec![Status::new(&pub_key, 2, &hash(&[]), &secret_key).raw().clone(),
                            Status::new(&pub_key, 4, &hash(&[2]), &secret_key).raw().clone(),
                            Status::new(&pub_key, 7, &hash(&[3]), &secret_key).raw().clone()];
    let block = Block::new(&pub_key,
                           &pub_key,
                           content.clone(),
                           precommits.clone(),
                           transactions.clone(),
                           &secret_key);

    assert_eq!(block.from(), &pub_key);
    assert_eq!(block.to(), &pub_key);
    assert_eq!(block.block(), content);
    assert_eq!(block.precommits(), precommits);
    assert_eq!(block.transactions(), transactions);

    let block2 = Block::from_raw(block.raw().clone()).unwrap();
    assert_eq!(block2.from(), &pub_key);
    assert_eq!(block2.to(), &pub_key);
    assert_eq!(block2.block(), content);
    assert_eq!(block2.precommits(), precommits);
    assert_eq!(block2.transactions(), transactions);
    let block_proof = BlockProof {
        block: content.clone(),
        precommits: precommits.clone(),
    };
    let json_str = ::serialize::json::to_string(&block_proof).unwrap();
    let block_proof_1: BlockProof = ::serialize::json::from_str(&json_str).unwrap(); 
    assert_eq!(block_proof, block_proof_1);
}

#[test]
fn test_empty_block() {
    let (pub_key, secret_key) = gen_keypair();

    let content = blockchain::Block::new(200, 1, &hash(&[1]), &hash(&[2]), &hash(&[3]));

    let precommits = Vec::new();
    let transactions = Vec::new();
    let block = Block::new(&pub_key,
                           &pub_key,
                           content.clone(),
                           precommits.clone(),
                           transactions.clone(),
                           &secret_key);

    assert_eq!(block.from(), &pub_key);
    assert_eq!(block.to(), &pub_key);
    assert_eq!(block.block(), content);
    assert_eq!(block.precommits(), precommits);
    assert_eq!(block.transactions(), transactions);

    let block2 = Block::from_raw(block.raw().clone()).unwrap();
    assert_eq!(block2.from(), &pub_key);
    assert_eq!(block2.to(), &pub_key);
    assert_eq!(block2.block(), content);
    assert_eq!(block2.precommits(), precommits);
    assert_eq!(block2.transactions(), transactions);
}

#[test]
fn test_request_block() {
    let (public_key, secret_key) = gen_keypair();

    // write
    let request = RequestBlock::new(&public_key, &public_key, 1, &secret_key);
    // read
    assert_eq!(request.from(), &public_key);
    assert_eq!(request.height(), 1);
    assert_eq!(request.to(), &public_key);
    assert!(request.verify_signature(&public_key));
}
