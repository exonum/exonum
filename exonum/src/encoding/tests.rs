// Copyright 2017 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use bit_vec::BitVec;

use std::net::SocketAddr;
use std::time::SystemTime;

use crypto::{hash, gen_keypair};
use blockchain::{self, BlockProof, Block};
use messages::{RawMessage, Message, FromRaw, Connect, Propose, Prevote, Precommit, Status,
               BlockResponse, BlockRequest};
use helpers::{Height, Round, ValidatorId};

use super::{Field, Offset};

static VALIDATOR: ValidatorId = ValidatorId(65_123);
static HEIGHT: Height = Height(123_123_123);
static ROUND: Round = Round(321_321_312);

#[allow(dead_code)]
// This structures used to test deserialize,
// so we should ingore unused `new` method.
mod ignore_new {
    use crypto::Hash;
    encoding_struct! {
        struct Parent {
            const SIZE = 8;
            field child: Child [0 => 8]
        }
    }

    encoding_struct! {
        struct Child {
            const SIZE = 32;
            field child: &Hash [0 => 32]
        }
    }
}

use self::ignore_new::*;

#[test]
#[should_panic(expected = "Found error in check: UnexpectedlyShortPayload")]
fn test_zero_size_segment() {
    let buf = vec![8,0,0,0, // not overlap
                   0,0,0,0,0]; // but with zero size

    <Parent as Field>::check(&buf, 0.into(), 8.into(), 8.into()).expect("Found error in check");
}

#[test]
#[should_panic(expected = "Found error in check: UnexpectedlyShortPayload")]
fn test_incorrect_pointer() {
    let buf = vec![8,0,0,0, // not overlap
                   0,0,0,0,0]; // but with zero size

    <Parent as Field>::check(&buf, 0.into(), 8.into(), 8.into()).expect("Found error in check");
}

#[test]
#[should_panic(expected = "Found error in check: OffsetOverflow")]
fn test_read_overflow_arithmetic() {
    let pos = <u32>::max_value();
    let buf = vec![255; 0];

    <Vec<u8> as Field>::check(&buf, pos.into(), 8.into(), pos.into())
        .expect("Found error in check");
}

#[test]
fn test_bitvec() {

    let mut b = BitVec::from_elem(14, false);
    b.set(11, true);
    b.set(4, true);
    b.push(true);
    b.push(true);

    assert_write_check_read(b, 8);
}

#[test]
fn test_str_segment() {
    let mut buf = vec![0; 8];
    let s = "test юникодной строчки efw_adqq ss/adfq";
    Field::write(&s, &mut buf, 0, 8);
    <&str as Field>::check(&buf, 0.into(), 8.into(), 8.into()).unwrap();

    let buf2 = buf.clone();
    <&str as Field>::check(&buf2, 0.into(), 8.into(), 8.into()).unwrap();
    let s2: &str = unsafe { Field::read(&buf2, 0, 8) };
    assert_eq!(s2, s);
}

#[test]
fn test_vec_segment() {
    let dat = vec![1u8, 2, 3, 5, 10];
    assert_write_check_read(dat, 8);
}

#[test]
fn test_u16_segment() {
    let dat = vec![1u16, 3, 10, 15, 23, 4, 45];
    assert_write_check_read(dat, 8);
}

#[test]
fn test_u32_segment() {
    let dat = vec![1u32, 3, 10, 15, 23, 4, 45];
    assert_write_check_read(dat, 8);
}

#[test]
fn test_byte_array() {
    let mut buf = vec![255; 8];
    let arr = [2u8, 5, 2, 3, 56, 3];

    Field::write(&arr.as_ref(), &mut buf, 0, 8);
    <&[u8] as Field>::check(&buf, 0.into(), 8.into(), 8.into()).unwrap();

    let buf2 = buf.clone();
    <&[u8] as Field>::check(&buf2, 0.into(), 8.into(), 8.into()).unwrap();
    let dat2: &[u8] = unsafe { Field::read(&buf2, 0, 8) };
    assert_eq!(dat2, arr);
    assert_eq!(buf.len(), 8 + arr.len());
}

#[test]
fn test_segments_of_arrays() {
    let mut buf = vec![255; 64];
    let v1 = [1u8, 2, 3];
    let v2 = [1u8, 3];
    let v3 = [2u8, 5, 2, 3, 56, 3];

    let dat = vec![v1.as_ref(), v2.as_ref(), v3.as_ref()];
    Field::write(&dat, &mut buf, 48, 56);
    <Vec<&[u8]> as Field>::check(&buf, 48.into(), 56.into(), 64.into()).unwrap();

    let buf2 = buf.clone();
    <Vec<&[u8]> as Field>::check(&buf2, 48.into(), 56.into(), 64.into()).unwrap();
    let dat2: Vec<&[u8]> = unsafe { Field::read(&buf2, 48, 56) };
    assert_eq!(dat2, dat);
    //48 spaces + 8 segment of vec + 8 spaces = 64 +
    // + v1_segment + v2_segment + v3_segment +
    // + v1_body + v2_body + v3_body
    assert_eq!(buf.len(), 64 + v1.len() + v2.len() + v3.len() + 3 * 8);
}


fn assert_write_check_read<T>(input: T, header_size: Offset)
where
    T: for<'r> Field<'r> + PartialEq + ::std::fmt::Debug,
{
    let mut buffer = vec![0; header_size as usize];
    Field::write(&input, &mut buffer, 0, header_size);
    <T as Field>::check(&buffer, 0.into(), header_size.into(), header_size.into()).unwrap();
    let new_buffer = buffer.clone();
    //clear buffer
    let len = buffer.len();
    buffer.clear();
    //and fill old buffer with zeros
    buffer.resize(len, 0);

    <T as Field>::check(
        &new_buffer,
        0.into(),
        header_size.into(),
        header_size.into(),
    ).unwrap();
    let output = unsafe { Field::read(&new_buffer, 0, header_size) };
    assert_eq!(input, output);

}

#[test]
fn test_segments_of_raw_buffers() {
    let buf = vec![255u8; 1];

    let input = vec![buf.clone(), buf.clone(), buf.clone()];
    assert_write_check_read(input, 8);
}

#[test]
fn test_segments_of_raw_messages() {
    let (pub_key, sec_key) = gen_keypair();

    let m1 = Status::new(&pub_key, Height(2), &hash(&[]), &sec_key);
    let m2 = Status::new(&pub_key, Height(4), &hash(&[1]), &sec_key);
    let m3 = Status::new(&pub_key, Height(5), &hash(&[3]), &sec_key);

    let dat = vec![m1.raw().clone(), m2.raw().clone(), m3.raw().clone()];
    assert_write_check_read(dat, 8);
}

#[test]
fn test_empty_segments() {
    let dat: Vec<RawMessage> = vec![];
    assert_write_check_read(dat, 8);
}

#[test]
fn test_segments_of_status_messages() {
    let (pub_key, sec_key) = gen_keypair();

    let m1 = Status::new(&pub_key, Height(2), &hash(&[]), &sec_key);
    let m2 = Status::new(&pub_key, Height(4), &hash(&[1]), &sec_key);
    let m3 = Status::new(&pub_key, Height(5), &hash(&[3]), &sec_key);

    let dat = vec![m1, m2, m3];
    assert_write_check_read(dat, 8);
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
    let prev_hash = hash(&[1, 2, 3]);
    let txs = vec![hash(&[1]), hash(&[2]), hash(&[2])];
    let (public_key, secret_key) = gen_keypair();

    // write
    let propose = Propose::new(VALIDATOR, HEIGHT, ROUND, &prev_hash, &txs, &secret_key);
    // read
    assert_eq!(propose.validator(), VALIDATOR);
    assert_eq!(propose.height(), HEIGHT);
    assert_eq!(propose.round(), ROUND);
    assert_eq!(propose.prev_hash(), &prev_hash);
    assert_eq!(propose.transactions().len(), 3);
    assert_eq!(propose.transactions()[0], txs[0]);
    assert_eq!(propose.transactions()[1], txs[1]);
    assert_eq!(propose.transactions()[2], txs[2]);
    assert!(propose.verify_signature(&public_key));
}

#[test]
fn test_prevote() {
    let propose_hash = hash(&[1, 2, 3]);
    let locked_round = Round(654_345);
    let (public_key, secret_key) = gen_keypair();

    // write
    let prevote = Prevote::new(
        VALIDATOR,
        HEIGHT,
        ROUND,
        &propose_hash,
        locked_round,
        &secret_key,
    );
    // read
    assert_eq!(prevote.validator(), VALIDATOR);
    assert_eq!(prevote.height(), HEIGHT);
    assert_eq!(prevote.round(), ROUND);
    assert_eq!(prevote.propose_hash(), &propose_hash);
    assert_eq!(prevote.locked_round(), locked_round);
    assert!(prevote.verify_signature(&public_key));
}

#[test]
fn test_precommit() {
    let propose_hash = hash(&[1, 2, 3]);
    let block_hash = hash(&[3, 2, 1]);
    let (public_key, secret_key) = gen_keypair();
    let time = SystemTime::now();

    // write
    let precommit = Precommit::new(
        VALIDATOR,
        HEIGHT,
        ROUND,
        &propose_hash,
        &block_hash,
        time,
        &secret_key,
    );
    // read
    assert_eq!(precommit.validator(), VALIDATOR);
    assert_eq!(precommit.height(), HEIGHT);
    assert_eq!(precommit.round(), ROUND);
    assert_eq!(precommit.propose_hash(), &propose_hash);
    assert_eq!(precommit.block_hash(), &block_hash);
    assert!(precommit.verify_signature(&public_key));
    assert_eq!(precommit.time(), time);
    let json_str = ::serde_json::to_string(&precommit).unwrap();
    assert!(json_str.len() > 0);
    let precommit1: Precommit = ::serde_json::from_str(&json_str).unwrap();
    assert_eq!(precommit, precommit1);
}

#[test]
fn test_status() {
    let last_hash = hash(&[3, 2, 1]);
    let (public_key, secret_key) = gen_keypair();

    // write
    let commit = Status::new(&public_key, HEIGHT, &last_hash, &secret_key);
    // read
    assert_eq!(commit.from(), &public_key);
    assert_eq!(commit.height(), HEIGHT);
    assert_eq!(commit.last_hash(), &last_hash);
    assert!(commit.verify_signature(&public_key));
}

#[test]
fn test_block() {
    let (pub_key, secret_key) = gen_keypair();
    let ts = SystemTime::now();
    let txs = [2];
    let tx_count = txs.len() as u32;

    let content = Block::new(
        blockchain::SCHEMA_MAJOR_VERSION,
        ValidatorId::zero(),
        Height(500),
        tx_count,
        &hash(&[1]),
        &hash(&txs),
        &hash(&[3]),
    );

    let precommits = vec![
        Precommit::new(
            ValidatorId(123),
            Height(15),
            Round(25),
            &hash(&[1, 2, 3]),
            &hash(&[3, 2, 1]),
            ts,
            &secret_key
        ),
        Precommit::new(
            ValidatorId(13),
            Height(25),
            Round(35),
            &hash(&[4, 2, 3]),
            &hash(&[3, 3, 1]),
            ts,
            &secret_key
        ),
        Precommit::new(
            ValidatorId(323),
            Height(15),
            Round(25),
            &hash(&[1, 1, 3]),
            &hash(&[5, 2, 1]),
            ts,
            &secret_key
        ),
    ];
    let transactions = vec![
        Status::new(&pub_key, Height(2), &hash(&[]), &secret_key)
            .raw()
            .clone(),
        Status::new(&pub_key, Height(4), &hash(&[2]), &secret_key)
            .raw()
            .clone(),
        Status::new(&pub_key, Height(7), &hash(&[3]), &secret_key)
            .raw()
            .clone(),
    ];
    let block = BlockResponse::new(
        &pub_key,
        &pub_key,
        content.clone(),
        precommits.clone(),
        transactions.clone(),
        &secret_key,
    );

    assert_eq!(block.from(), &pub_key);
    assert_eq!(block.to(), &pub_key);
    assert_eq!(block.block(), content);
    assert_eq!(block.precommits(), precommits);
    assert_eq!(block.transactions(), transactions);

    let block2 = BlockResponse::from_raw(block.raw().clone()).unwrap();
    assert_eq!(block2.from(), &pub_key);
    assert_eq!(block2.to(), &pub_key);
    assert_eq!(block2.block(), content);
    assert_eq!(block2.precommits(), precommits);
    assert_eq!(block2.transactions(), transactions);
    let block_proof = BlockProof {
        block: content.clone(),
        precommits: precommits.clone(),
    };
    let json_str = ::serde_json::to_string(&block_proof).unwrap();
    let block_proof_1: BlockProof = ::serde_json::from_str(&json_str).unwrap();
    assert_eq!(block_proof, block_proof_1);
}

#[test]
fn test_empty_block() {
    let (pub_key, secret_key) = gen_keypair();

    let content = Block::new(
        blockchain::SCHEMA_MAJOR_VERSION,
        ValidatorId::zero(),
        Height(200),
        1,
        &hash(&[1]),
        &hash(&[2]),
        &hash(&[3]),
    );

    let precommits = Vec::new();
    let transactions = Vec::new();
    let block = BlockResponse::new(
        &pub_key,
        &pub_key,
        content.clone(),
        precommits.clone(),
        transactions.clone(),
        &secret_key,
    );

    assert_eq!(block.from(), &pub_key);
    assert_eq!(block.to(), &pub_key);
    assert_eq!(block.block(), content);
    assert_eq!(block.precommits(), precommits);
    assert_eq!(block.transactions(), transactions);

    let block2 = BlockResponse::from_raw(block.raw().clone()).unwrap();
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
    let request = BlockRequest::new(&public_key, &public_key, Height(1), &secret_key);
    // read
    assert_eq!(request.from(), &public_key);
    assert_eq!(request.height(), Height(1));
    assert_eq!(request.to(), &public_key);
    assert!(request.verify_signature(&public_key));
}

#[test]
fn test_correct_encoding_struct() {
    encoding_struct! {
        struct NoFields {
            const SIZE = 0;
        }
    }
    drop(NoFields::new());

    encoding_struct! {
        struct OneField {
            const SIZE = 8;

            field one:   u64 [00 => 08]
        }
    }
    drop(OneField::new(0));

    encoding_struct! {
        struct TwoFields {
            const SIZE = 8;

            field one:   u32 [00 => 04]
            field two:   u32 [04 => 08]
        }
    }
    drop(TwoFields::new(0, 0));

    encoding_struct! {
        struct ThreeFields {
            const SIZE = 8;

            field one:   u16 [00 => 02]
            field two:   u16 [02 => 04]
            field three: u32 [04 => 08]
        }
    }
    drop(ThreeFields::new(0, 0, 0));
}

#[test]
#[should_panic(expected = "fields should be adjacent")]
fn test_encoding_struct_with_hole() {
    encoding_struct! {
        struct MiddleHole {
            const SIZE = 9;

            field one:   u16 [00 => 02]
            field two:   u16 [03 => 05] // start should be 2
            field three: u32 [05 => 09]
        }
    }
    drop(MiddleHole::new(0, 0, 0));
}

#[test]
#[should_panic(expected = "fields should be adjacent")]
fn test_encoding_struct_with_overlay() {
    encoding_struct! {
        struct FieldOverlay {
            const SIZE = 7;

            field one:   u16 [00 => 02]
            field two:   u16 [01 => 03] // start should be 2
            field three: u32 [03 => 07]
        }
    }
    drop(FieldOverlay::new(0, 0, 0));
}

#[test]
#[should_panic(expected = "wrong size of field")]
fn test_encoding_struct_wrong_size() {
    encoding_struct! {
        struct WrongSize {
            const SIZE = 7;

            field one:   u16 [00 => 02]
            field two:   u16 [02 => 03] // size should be 2
            field three: u32 [03 => 07]
        }
    }
    drop(WrongSize::new(0, 0, 0));
}
