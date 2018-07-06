use super::{
    BlockResponse, Message, Precommit, RawTransaction, SignedMessage, Status, TransactionsResponse,
    UncheckedBuffer, RAW_TRANSACTION_EMPTY_SIZE, TRANSACTION_RESPONSE_EMPTY_SIZE,
};
use blockchain::{Block, BlockProof};
use chrono::Utc;
use crypto::{gen_keypair, hash};
use helpers::{Height, Round, ValidatorId};

#[test]
fn test_blockresponse_empty_size() {
    use crypto::{gen_keypair_from_seed, Seed};
    let (public_key, secret_key) = gen_keypair_from_seed(&Seed::new([1; 32]));
    let msg = TransactionsResponse::new(&public_key, vec![]);
    let msg = Message::new(msg, public_key, &secret_key);
    assert_eq!(
        TRANSACTION_RESPONSE_EMPTY_SIZE,
        msg.into_parts().1.to_vec().len()
    )
}

#[test]
fn test_empty_tx_size() {
    use crypto::{gen_keypair_from_seed, Seed};
    let (public_key, secret_key) = gen_keypair_from_seed(&Seed::new([1; 32]));

    let msg = RawTransaction::new(0, vec![]);
    let msg = Message::new(msg, public_key, &secret_key);
    assert_eq!(
        RAW_TRANSACTION_EMPTY_SIZE,
        msg.into_parts().1.to_vec().len()
    )
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
        Message::new(
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
        Message::new(
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
        Message::new(
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
        Message::new(Status::new(Height(2), &hash(&[])), pub_key, &secret_key).hash(),
        Message::new(Status::new(Height(4), &hash(&[2])), pub_key, &secret_key).hash(),
        Message::new(Status::new(Height(7), &hash(&[3])), pub_key, &secret_key).hash(),
    ];
    let precommits_buf: Vec<_> = precommits
        .iter()
        .map(|x| UncheckedBuffer::new(x.clone().into_parts().1.to_vec()))
        .collect();
    let block = Message::new(
        BlockResponse::new(
            &pub_key,
            content.clone(),
            precommits_buf.clone(),
            &transactions,
        ),
        pub_key,
        &secret_key,
    );

    assert_eq!(block.author(), &pub_key);
    assert_eq!(block.to(), &pub_key);
    assert_eq!(block.block(), content);
    assert_eq!(block.precommits(), precommits_buf);
    assert_eq!(block.transactions().to_vec(), transactions);

    let block2 = SignedMessage::verify_buffer(block.clone().into_parts().1.to_vec())
        .unwrap()
        .into_message()
        .map_into::<BlockResponse>()
        .unwrap();

    assert_eq!(block2.author(), &pub_key);
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
    println!("proof = {}", json_str);
    assert_eq!(block_proof, block_proof_1);
    panic!("Printl proof")
}
