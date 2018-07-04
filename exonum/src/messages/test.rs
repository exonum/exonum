use super::{BlockResponse, Message, TRANSACTION_RESPONSE_EMPTY_SIZE};

#[test]
fn test_blockresponse_empty_size() {
    use crypto::gen_keypair_from_seed;
    /*
    use ::helpers::{ValidatorId, Height,},
    use ::blockchain::Block;
    let block = Block::new(0, ValidatorId(0), Height(0), 0, &Hash::zero(), &Hash::zero(), &Hash::zero())

    let (public_key, secret_key) = ::crypto::gen_keypair_from_seed(&Seed::new([1; 32]));
    let msg = BlockResponse::new(public_key, block, vec![], []);
    */
    let msg = TransactionsResponse::new(public_key, vec![]);
    let msg = Message::new(msg, public_key, &secret_key);
    assert_eq!(
        TRANSACTION_RESPONSE_EMPTY_SIZE,
        msg.into_parts().1.to_vec().len()
    )
}
