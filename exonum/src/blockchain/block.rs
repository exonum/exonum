use crypto::{Hash, hash};


pub const BLOCK_SIZE: usize = 108;

storage_value!(
    struct Block {
        const SIZE = BLOCK_SIZE;

        field height:                 u64         [00 => 08]
        field propose_round:          u32         [08 => 12]
        field prev_hash:              &Hash       [12 => 44]
        field tx_hash:                &Hash       [44 => 76]
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
