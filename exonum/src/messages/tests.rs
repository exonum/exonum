use chrono::Utc;
use hex::{self, FromHex};
use serde_json;

use super::{
    BinaryForm, BlockResponse, Message, Precommit, ProtocolMessage, RawTransaction,
    ServiceTransaction, Signed, SignedMessage, Status, TransactionsResponse,
    RAW_TRANSACTION_EMPTY_SIZE, TRANSACTION_RESPONSE_EMPTY_SIZE,
};
use blockchain::{Block, BlockProof};
use crypto::{gen_keypair, hash, PublicKey, SecretKey};
use helpers::{Height, Round, ValidatorId};
use proto;

#[test]
fn test_block_response_empty_size() {
    use crypto::{gen_keypair_from_seed, Seed};
    let (public_key, secret_key) = gen_keypair_from_seed(&Seed::new([1; 32]));
    let msg = TransactionsResponse::new(&public_key, vec![]);
    let msg = Message::concrete(msg, public_key, &secret_key);
    assert_eq!(
        TRANSACTION_RESPONSE_EMPTY_SIZE,
        msg.signed_message().raw().len()
    )
}

#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug, Serialize, Deserialize, ProtobufConvert)]
#[exonum(pb = "proto::schema::tests::CreateWallet", crate = "crate")]
struct CreateWallet {
    pubkey: PublicKey,
    name: String,
}

impl CreateWallet {
    fn new(&pubkey: &PublicKey, name: &str) -> Self {
        Self {
            pubkey,
            name: name.to_owned(),
        }
    }
}

#[test]
fn test_known_transaction() {
    let res = "57d4f9d3ebd09d09d6477546f2504b4da2e02c8dab89ece56a39e7e459e3be3d\
    0000800000000a220a2057d4f9d3ebd09d09d6477546f2504b4da2e02c8dab89ece56a39e7e459e3be3d120b\
    746573745f77616c6c6574a3dac954f891ff93d0da5d4540773532e81aa90e813f77dc8b95105dea6dcf08a6291fc4210335fc4aa37ba4a80ebc8a57b4cb23602d1b2b1800f25362f77d02";

    let pk =
        PublicKey::from_hex("57d4f9d3ebd09d09d6477546f2504b4da2e02c8dab89ece56a39e7e459e3be3d")
            .unwrap();
    let sk = SecretKey::from_hex(
        "d142addc3951d67a99f3fd25a4c1294ee088f7a907ed13c4cc6f7c74b5b3147f\
         57d4f9d3ebd09d09d6477546f2504b4da2e02c8dab89ece56a39e7e459e3be3d",
    )
    .unwrap();
    let data = CreateWallet::new(&pk, "test_wallet");

    let set = ServiceTransaction::from_raw_unchecked(0, data.encode().unwrap());
    let msg = RawTransaction::new(128, set);
    let msg = Message::concrete(msg, pk, &sk);
    SignedMessage::from_raw_buffer(hex::decode(res).unwrap()).unwrap();
    assert_eq!(res, hex::encode(msg.signed_message().raw()));
}

#[test]
fn test_empty_tx_size() {
    use crypto::{gen_keypair_from_seed, Seed};
    let (public_key, secret_key) = gen_keypair_from_seed(&Seed::new([1; 32]));
    let set = ServiceTransaction::from_raw_unchecked(0, vec![]);
    let msg = RawTransaction::new(0, set);
    let msg = Message::concrete(msg, public_key, &secret_key);
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
    let precommits_buf: Vec<_> = precommits.iter().map(|x| x.clone().serialize()).collect();
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
    assert_eq!(block.block(), content);
    assert_eq!(block.precommits(), precommits_buf);
    assert_eq!(block.transactions().to_vec(), transactions);

    let block2: Signed<BlockResponse> = ProtocolMessage::try_from(
        Message::deserialize(SignedMessage::from_raw_buffer(block.serialize()).unwrap()).unwrap(),
    )
    .unwrap();

    assert_eq!(block2.author(), pub_key);
    assert_eq!(block2.to(), &pub_key);
    assert_eq!(block2.block(), content);
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
#[should_panic(expected = "Cannot verify message.")]
fn test_precommit_serde_wrong_signature() {
    use crypto::SIGNATURE_LENGTH;

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
        let raw_len = precommit.message.raw.len();
        let signature = &mut precommit.message.raw[raw_len - SIGNATURE_LENGTH..];
        signature.copy_from_slice(&[0u8; SIGNATURE_LENGTH]);
    }
    let precommit_json = serde_json::to_string(&precommit).unwrap();
    let precommit2: Signed<Precommit> = serde_json::from_str(&precommit_json).unwrap();
    assert_eq!(precommit2, precommit);
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
