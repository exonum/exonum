use chrono::Utc;
use hex::{self, FromHex};

use super::{
    BinaryForm, BlockResponse, Message, Precommit, Protocol, ProtocolMessage, RawTransaction,
    ServiceTransaction, SignedMessage, Status, TransactionsResponse, RAW_TRANSACTION_EMPTY_SIZE,
    TRANSACTION_RESPONSE_EMPTY_SIZE,
};
use blockchain::{Block, BlockProof};
use crypto::{gen_keypair, hash, PublicKey, SecretKey};
use helpers::{Height, Round, ValidatorId};

#[test]
fn test_block_response_empty_size() {
    use crypto::{gen_keypair_from_seed, Seed};
    let (public_key, secret_key) = gen_keypair_from_seed(&Seed::new([1; 32]));
    let msg = TransactionsResponse::new(&public_key, vec![]);
    let msg = Protocol::concrete(msg, public_key, &secret_key);
    assert_eq!(
        TRANSACTION_RESPONSE_EMPTY_SIZE,
        msg.signed_message().raw().len()
    )
}

encoding_struct! {
    struct CreateWallet {
        pk: &PublicKey,
        name: &str,
    }
}

#[test]
fn test_known_transaction() {
    let res = "57d4f9d3ebd09d09d6477546f2504b4da2e02c8dab89ece56a39e7e459e3be3d\
    00008000020057d4f9d3ebd09d09d6477546f2504b4da2e02c8dab89ece56a39e7e459e3be3d280000000b000000\
    746573745f77616c6c6574ff86a65814128dd86b2d267f7dd2de443c484139ae936e7c7405884c97619251f6a3d878d0ca140f026583a88777e074586d590388757159de3617f799959706";

    let pk = PublicKey::from_hex(
        "57d4f9d3ebd09d09d6477546f2504b4da2e02c8dab89ece56a39e7e459e3be3d",
    ).unwrap();
    let sk = SecretKey::from_hex(
        "d142addc3951d67a99f3fd25a4c1294ee088f7a907ed13c4cc6f7c74b5b3147f\
         57d4f9d3ebd09d09d6477546f2504b4da2e02c8dab89ece56a39e7e459e3be3d",
    ).unwrap();
    let data = CreateWallet::new(&pk, "test_wallet");

    let set = ServiceTransaction::from_raw_unchecked(2, data.raw);
    let msg = RawTransaction::new(128, set);
    let msg = Protocol::concrete(msg, pk, &sk);
    SignedMessage::from_raw_buffer(hex::decode(res).unwrap()).unwrap();
    assert_eq!(res, hex::encode(msg.signed_message().raw()));
}

#[test]
fn test_empty_tx_size() {
    use crypto::{gen_keypair_from_seed, Seed};
    let (public_key, secret_key) = gen_keypair_from_seed(&Seed::new([1; 32]));
    let set = ServiceTransaction::from_raw_unchecked(0, vec![]);
    let msg = RawTransaction::new(0, set);
    let msg = Protocol::concrete(msg, public_key, &secret_key);
    assert_eq!(RAW_TRANSACTION_EMPTY_SIZE, msg.signed_message().raw().len())
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
        Protocol::concrete(
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
        Protocol::concrete(
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
        Protocol::concrete(
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
        Protocol::concrete(Status::new(Height(2), &hash(&[])), pub_key, &secret_key).hash(),
        Protocol::concrete(Status::new(Height(4), &hash(&[2])), pub_key, &secret_key).hash(),
        Protocol::concrete(Status::new(Height(7), &hash(&[3])), pub_key, &secret_key).hash(),
    ];
    let precommits_buf: Vec<_> = precommits.iter().map(|x| x.clone().serialize()).collect();
    let block = Protocol::concrete(
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
    assert_eq!(block.block(), content);
    assert_eq!(block.precommits(), precommits_buf);
    assert_eq!(block.transactions().to_vec(), transactions);

    let block2: Message<BlockResponse> = ProtocolMessage::try_from(
        Protocol::deserialize(SignedMessage::from_raw_buffer(block.serialize()).unwrap()).unwrap(),
    ).unwrap();

    assert_eq!(block2.author(), pub_key);
    assert_eq!(block2.to(), &pub_key);
    assert_eq!(block2.block(), content);
    assert_eq!(block2.precommits(), precommits_buf);
    assert_eq!(block2.transactions().to_vec(), transactions);
    let block_proof = BlockProof {
        block: content.clone(),
        precommits: precommits.clone(),
    };
    let json_str = ::serde_json::to_string(&block_proof).unwrap();
    let block_proof_1: BlockProof = ::serde_json::from_str(&json_str).unwrap();
    assert_eq!(block_proof, block_proof_1);
}

#[test]
fn test_raw_transaction_small_size() {
    assert!(ServiceTransaction::decode(&vec![0; 1]).is_err());
    assert!(RawTransaction::decode(&vec![0; 1]).is_err());
    assert!(RawTransaction::decode(&vec![0; 3]).is_err());
    let tx = RawTransaction::decode(&vec![0; 4]).unwrap();
    assert_eq!(tx.service_id, 0);
    assert_eq!(tx.service_transaction.transaction_id, 0);
}
