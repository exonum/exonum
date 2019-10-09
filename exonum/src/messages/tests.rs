use chrono::Utc;
use exonum_merkledb::ObjectHash;
use exonum_proto::ProtobufConvert;
use protobuf::Message as PbMessage;

use std::convert::TryFrom;

use crate::{
    blockchain::{Block, BlockProof},
    crypto::{self, gen_keypair, Signature},
    helpers::{Height, Round, ValidatorId},
    proto::{self},
};

use super::{
    BinaryValue, BlockResponse, Message, Precommit, SignedMessage, Status, TransactionsResponse,
    Verified, SIGNED_MESSAGE_MIN_SIZE, TX_RES_EMPTY_SIZE, TX_RES_PB_OVERHEAD_PAYLOAD,
};

#[test]
fn test_signed_message_min_size() {
    let (public_key, secret_key) = gen_keypair();
    let msg = SignedMessage::new(vec![], public_key, &secret_key);
    assert_eq!(SIGNED_MESSAGE_MIN_SIZE, msg.into_bytes().len())
}

#[test]
fn test_tx_response_empty_size() {
    let (public_key, secret_key) = gen_keypair();
    let msg = TransactionsResponse::new(public_key, vec![]);
    let msg = Verified::from_value(msg, public_key, &secret_key);
    assert_eq!(TX_RES_EMPTY_SIZE, msg.into_bytes().len())
}

#[test]
fn test_tx_response_with_txs_size() {
    let (public_key, secret_key) = gen_keypair();
    let txs = vec![
        vec![1_u8; 8],
        vec![2_u8; 16],
        vec![3_u8; 64],
        vec![4_u8; 256],
        vec![5_u8; 4096],
    ];
    let txs_size = txs.iter().fold(0, |acc, tx| acc + tx.len());
    let pb_max_overhead = TX_RES_PB_OVERHEAD_PAYLOAD * txs.len();

    let msg = TransactionsResponse::new(public_key, txs);
    let msg = Verified::from_value(msg, public_key, &secret_key);
    assert!(TX_RES_EMPTY_SIZE + txs_size + pb_max_overhead >= msg.into_bytes().len())
}

#[test]
fn test_message_roundtrip() {
    let (pub_key, secret_key) = gen_keypair();
    let ts = Utc::now();

    let msg = Verified::from_value(
        Precommit::new(
            ValidatorId(123),
            Height(15),
            Round(25),
            crypto::hash(&[1, 2, 3]),
            crypto::hash(&[3, 2, 1]),
            ts,
        ),
        pub_key,
        &secret_key,
    );

    let bytes = msg.to_bytes();
    let msg_enum = Message::from_signed(
        SignedMessage::from_bytes(bytes.into()).expect("SignedMessage decode."),
    )
    .expect("Message deserialize");
    let msg_roundtrip = Verified::<Precommit>::try_from(msg_enum).expect("Message type");
    assert_eq!(msg, msg_roundtrip)
}

#[test]
fn test_signed_message_unusual_protobuf() {
    let (pub_key, secret_key) = gen_keypair();

    let mut ex_msg = proto::ExonumMessage::new();
    let precommit_msg = Precommit::new(
        ValidatorId(123),
        Height(15),
        Round(25),
        crypto::hash(&[1, 2, 3]),
        crypto::hash(&[3, 2, 1]),
        Utc::now(),
    );
    ex_msg.set_precommit(precommit_msg.to_pb());
    let mut payload = ex_msg.write_to_bytes().unwrap();
    // Duplicate pb serialization to create unusual but correct protobuf message.
    payload.append(&mut payload.clone());

    let signed = SignedMessage::new(payload, pub_key, &secret_key);

    let signed_bytes = signed.into_bytes();
    let msg_enum = Message::from_raw_buffer(signed_bytes).expect("Message deserialize");
    let deserialized_precommit = Verified::<Precommit>::try_from(msg_enum).expect("Message type");
    assert_eq!(precommit_msg, *deserialized_precommit.payload())
}

#[test]
fn test_block() {
    let (pub_key, secret_key) = gen_keypair();
    let ts = Utc::now();
    let txs = [2];
    let tx_count = txs.len() as u32;

    let content = Block::new(
        ValidatorId::zero(),
        Height(500),
        tx_count,
        crypto::hash(&[1]),
        crypto::hash(&txs),
        crypto::hash(&[3]),
    );

    let precommits = vec![
        Verified::from_value(
            Precommit::new(
                ValidatorId(123),
                Height(15),
                Round(25),
                crypto::hash(&[1, 2, 3]),
                crypto::hash(&[3, 2, 1]),
                ts,
            ),
            pub_key,
            &secret_key,
        ),
        Verified::from_value(
            Precommit::new(
                ValidatorId(13),
                Height(25),
                Round(35),
                crypto::hash(&[4, 2, 3]),
                crypto::hash(&[3, 3, 1]),
                ts,
            ),
            pub_key,
            &secret_key,
        ),
        Verified::from_value(
            Precommit::new(
                ValidatorId(323),
                Height(15),
                Round(25),
                crypto::hash(&[1, 1, 3]),
                crypto::hash(&[5, 2, 1]),
                ts,
            ),
            pub_key,
            &secret_key,
        ),
    ];
    let transactions = [
        Verified::from_value(
            Status::new(Height(2), crypto::hash(&[]), 0),
            pub_key,
            &secret_key,
        ),
        Verified::from_value(
            Status::new(Height(4), crypto::hash(&[2]), 0),
            pub_key,
            &secret_key,
        ),
        Verified::from_value(
            Status::new(Height(7), crypto::hash(&[3]), 0),
            pub_key,
            &secret_key,
        ),
    ]
    .iter()
    .map(ObjectHash::object_hash)
    .collect::<Vec<_>>();

    let precommits_buf: Vec<_> = precommits.iter().map(BinaryValue::to_bytes).collect();
    let block = Verified::from_value(
        BlockResponse::new(
            pub_key,
            content.clone(),
            precommits_buf.clone(),
            transactions.iter().cloned(),
        ),
        pub_key,
        &secret_key,
    );

    assert_eq!(block.author(), pub_key);
    assert_eq!(block.payload().to, pub_key);
    assert_eq!(block.payload().block, content);
    assert_eq!(block.payload().precommits, precommits_buf);
    assert_eq!(block.payload().transactions, transactions);

    let block2: Verified<BlockResponse> = SignedMessage::from_bytes(block.to_bytes().into())
        .unwrap()
        .into_verified()
        .unwrap();

    assert_eq!(block2.author(), pub_key);
    assert_eq!(block2.payload().to, pub_key);
    assert_eq!(block2.payload().block, content);
    assert_eq!(block2.payload().precommits, precommits_buf);
    assert_eq!(block2.payload().transactions, transactions);
    let block_proof = BlockProof {
        block: content.clone(),
        precommits: precommits.clone(),
    };
    let json_str = serde_json::to_string(&block_proof).unwrap();
    let block_proof_1: BlockProof = serde_json::from_str(&json_str).unwrap();
    assert_eq!(block_proof, block_proof_1);
}

#[test]
fn test_precommit_serde_correct() {
    let (pub_key, secret_key) = gen_keypair();
    let ts = Utc::now();

    let precommit = Verified::from_value(
        Precommit::new(
            ValidatorId(123),
            Height(15),
            Round(25),
            crypto::hash(&[1, 2, 3]),
            crypto::hash(&[3, 2, 1]),
            ts,
        ),
        pub_key,
        &secret_key,
    );

    let precommit_json = serde_json::to_string(&precommit).unwrap();
    let precommit2: Verified<Precommit> = serde_json::from_str(&precommit_json).unwrap();
    assert_eq!(precommit2, precommit);
}

#[test]
#[should_panic(expected = "Failed to verify signature.")]
fn test_precommit_serde_wrong_signature() {
    let (pub_key, secret_key) = gen_keypair();
    let ts = Utc::now();

    let mut precommit = Verified::from_value(
        Precommit::new(
            ValidatorId(123),
            Height(15),
            Round(25),
            crypto::hash(&[1, 2, 3]),
            crypto::hash(&[3, 2, 1]),
            ts,
        ),
        pub_key,
        &secret_key,
    );
    // Break signature.
    precommit.raw.signature = Signature::zero();

    let precommit_json = serde_json::to_string(&precommit).unwrap();
    let precommit2: Verified<Precommit> = serde_json::from_str(&precommit_json).unwrap();
    assert_eq!(precommit2, precommit);
}
