use crypto::{Hash};

pub const BLOCK_SIZE: usize = 108;

storage_value!(
    /// Exonum block data structure. Block is essentially a list of transactions, which is 
    /// a result of the consensus algorithm (thus authenticated by the supermajority
    /// of validators) and is applied atomically to the blockchain state.
    struct Block {
        const SIZE = BLOCK_SIZE;

        /// Height of the committed block
        field height:                 u64         [00 => 08]
        /// Round when the proposal of this block appeared.
        field propose_round:          u32         [08 => 12]
        /// Hash link to the previous block in blockchain.
        field prev_hash:              &Hash       [12 => 44]
        /// Root hash of [merkle tree](struct.Schema.html#method.block_txs) of current block 
        /// transactions. 
        field tx_hash:                &Hash       [44 => 76]
        /// Hash of the current `exonum` state after applying transactions in the block.
        field state_hash:             &Hash       [76 => 108]
    }
);


// TODO: add network_id, block version?

#[cfg(test)]
mod tests {
    use crypto::hash;

    use super::*;

    #[test]
    fn test_block() {
        let height = 123_345;
        let prev_hash = hash(&[1, 2, 3]);
        let tx_hash = hash(&[4, 5, 6]);
        let state_hash = hash(&[7, 8, 9]);
        let round = 2;
        let block = Block::new(height, round, &prev_hash, &tx_hash, &state_hash);

        assert_eq!(block.height(), height);
        assert_eq!(block.prev_hash(), &prev_hash);
        assert_eq!(block.tx_hash(), &tx_hash);
        assert_eq!(block.state_hash(), &state_hash);
        assert_eq!(block.propose_round(), round);
        let json_str = ::serde_json::to_string(&block).unwrap();
        let block1: Block = ::serde_json::from_str(&json_str).unwrap();
        assert_eq!(block1, block);
    }
}
