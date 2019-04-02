use super::{
    BinaryForm, BlockResponse, Message, Precommit, ProtocolMessage, Signed, SignedMessage, Status,
    TransactionsResponse, SIGNED_MESSAGE_MIN_SIZE, TX_RES_EMPTY_SIZE, TX_RES_PB_OVERHEAD_PAYLOAD,
};
use crate::blockchain::{Block, BlockProof};
use crate::crypto::{gen_keypair, hash, CryptoHash, Signature};
use crate::helpers::{Height, Round, ValidatorId};
use crate::proto::{self, ProtobufConvert};
use chrono::Utc;
use protobuf::Message as PbMessage;

#[test]
fn test_signed_message_min_size() {
    let (public_key, secret_key) = gen_keypair();
    let msg = SignedMessage::new(&[0; 0], public_key, &secret_key);
    assert_eq!(SIGNED_MESSAGE_MIN_SIZE, msg.encode().unwrap().len())
}

#[test]
fn test_tx_response_empty_size() {
    let (public_key, secret_key) = gen_keypair();
    let msg = TransactionsResponse::new(&public_key, vec![]);
    let msg = Message::concrete(msg, public_key, &secret_key);
    assert_eq!(TX_RES_EMPTY_SIZE, msg.encode().unwrap().len())
}

#[test]
fn test_tx_response_with_txs_size() {
    let (public_key, secret_key) = gen_keypair();
    let txs = vec![
        vec![1u8; 8],
        vec![2u8; 16],
        vec![3u8; 64],
        vec![4u8; 256],
        vec![5u8; 4096],
    ];
    let txs_size = txs.iter().fold(0, |acc, tx| acc + tx.len());
    let pb_max_overhead = TX_RES_PB_OVERHEAD_PAYLOAD * txs.len();

    let msg = TransactionsResponse::new(&public_key, txs);
    let msg = Message::concrete(msg, public_key, &secret_key);
    assert!(TX_RES_EMPTY_SIZE + txs_size + pb_max_overhead >= msg.encode().unwrap().len())
}

#[test]
fn test_message_roundtrip() {
    let (pub_key, secret_key) = gen_keypair();
    let ts = Utc::now();

    let msg = Message::concrete(
        Precommit::new(
            ValidatorId(123),
            Height(15),
            Round(25),
            &hash(&[1, 2, 3]),
            &hash(&[3, 2, 1]),
            ts,
        ),
        pub_key,
        &secret_key,
    );

    let bytes = msg.encode().expect("Signed<T> encode");
    let msg_enum =
        Message::deserialize(SignedMessage::decode(&bytes).expect("SignedMessage decode."))
            .expect("Message deserialize");
    let msg_roundtrip = Precommit::try_from(msg_enum).expect("Message type");
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
        &hash(&[1, 2, 3]),
        &hash(&[3, 2, 1]),
        Utc::now(),
    );
    ex_msg.set_precommit(precommit_msg.to_pb());
    let mut payload = ex_msg.write_to_bytes().unwrap();
    // Duplicate pb serialization to create unusual but correct protobuf message.
    payload.append(&mut payload.clone());

    let signed = SignedMessage::new(&payload, pub_key, &secret_key);

    let signed_bytes = signed.encode().expect("SignedMessage encode");
    let msg_enum =
        Message::deserialize(SignedMessage::decode(&signed_bytes).expect("SignedMessage decode."))
            .expect("Message deserialize");
    let deserialized_precommit = Precommit::try_from(msg_enum).expect("Message type");
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
        &hash(&[1]),
        &hash(&txs),
        &hash(&[3]),
    );

    let precommits = vec![
        Message::concrete(
            Precommit::new(
                ValidatorId(123),
                Height(15),
                Round(25),
                &hash(&[1, 2, 3]),
                &hash(&[3, 2, 1]),
                ts,
            ),
            pub_key,
            &secret_key,
        ),
        Message::concrete(
            Precommit::new(
                ValidatorId(13),
                Height(25),
                Round(35),
                &hash(&[4, 2, 3]),
                &hash(&[3, 3, 1]),
                ts,
            ),
            pub_key,
            &secret_key,
        ),
        Message::concrete(
            Precommit::new(
                ValidatorId(323),
                Height(15),
                Round(25),
                &hash(&[1, 1, 3]),
                &hash(&[5, 2, 1]),
                ts,
            ),
            pub_key,
            &secret_key,
        ),
    ];
    let transactions = vec![
        Message::concrete(Status::new(Height(2), &hash(&[])), pub_key, &secret_key).hash(),
        Message::concrete(Status::new(Height(4), &hash(&[2])), pub_key, &secret_key).hash(),
        Message::concrete(Status::new(Height(7), &hash(&[3])), pub_key, &secret_key).hash(),
    ];
    let precommits_buf: Vec<_> = precommits
        .iter()
        .map(|x| x.clone().encode().unwrap())
        .collect();
    let block = Message::concrete(
        BlockResponse::new(
            &pub_key,
            content.clone(),
            precommits_buf.clone(),
            &transactions,
        ),
        pub_key,
        &secret_key,
    );

    assert_eq!(block.author(), pub_key);
    assert_eq!(block.to(), &pub_key);
    assert_eq!(block.block(), &content);
    assert_eq!(block.precommits(), precommits_buf);
    assert_eq!(block.transactions().to_vec(), transactions);

    let block2: Signed<BlockResponse> = ProtocolMessage::try_from(
        Message::deserialize(SignedMessage::decode(&block.encode().unwrap()).unwrap()).unwrap(),
    )
    .unwrap();

    assert_eq!(block2.author(), pub_key);
    assert_eq!(block2.to(), &pub_key);
    assert_eq!(block2.block(), &content);
    assert_eq!(block2.precommits(), precommits_buf);
    assert_eq!(block2.transactions().to_vec(), transactions);
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

    let precommit = Message::concrete(
        Precommit::new(
            ValidatorId(123),
            Height(15),
            Round(25),
            &hash(&[1, 2, 3]),
            &hash(&[3, 2, 1]),
            ts,
        ),
        pub_key,
        &secret_key,
    );

    let precommit_json = serde_json::to_string(&precommit).unwrap();
    let precommit2: Signed<Precommit> = serde_json::from_str(&precommit_json).unwrap();
    assert_eq!(precommit2, precommit);
}

#[test]
#[should_panic(expected = "Failed to verify signature.")]
fn test_precommit_serde_wrong_signature() {
    let (pub_key, secret_key) = gen_keypair();
    let ts = Utc::now();

    let mut precommit = Message::concrete(
        Precommit::new(
            ValidatorId(123),
            Height(15),
            Round(25),
            &hash(&[1, 2, 3]),
            &hash(&[3, 2, 1]),
            ts,
        ),
        pub_key,
        &secret_key,
    );
    // Break signature.
    {
        let sign = precommit.signed_message_mut().signature_mut();
        *sign = Signature::zero();
    }
    let precommit_json = serde_json::to_string(&precommit).unwrap();
    let precommit2: Signed<Precommit> = serde_json::from_str(&precommit_json).unwrap();
    assert_eq!(precommit2, precommit);
}
