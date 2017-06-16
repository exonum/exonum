use crypto::Hash;


pub const BLOCK_SIZE: usize = 112;

pub const SCHEMA_MAJOR_VERSION: u16 = 0;

encoding_struct!(
    struct Block {
        const SIZE = BLOCK_SIZE;

        field schema_version:         u16         [00 => 02]
        field proposer_id:            u16         [02 => 04]
        field height:                 u64         [04 => 12]
        field tx_count:               u32         [12 => 16]
        field prev_hash:              &Hash       [16 => 48]
        field tx_hash:                &Hash       [48 => 80]
        field state_hash:             &Hash       [80 => 112]
    }
);


#[cfg(test)]
mod tests {
    use crypto::hash;

    use super::*;

    #[test]
    fn test_block() {
        let proposer_id = 1024;
        let txs = [4, 5, 6];
        let height = 123_345;
        let prev_hash = hash(&[1, 2, 3]);
        let tx_hash = hash(&txs);
        let tx_count = txs.len() as u32;
        let state_hash = hash(&[7, 8, 9]);
        let block = Block::new(SCHEMA_MAJOR_VERSION,
                               proposer_id,
                               height,
                               tx_count,
                               &prev_hash,
                               &tx_hash,
                               &state_hash);

        assert_eq!(block.schema_version(), SCHEMA_MAJOR_VERSION);
        assert_eq!(block.proposer_id(), proposer_id);
        assert_eq!(block.height(), height);
        assert_eq!(block.tx_count(), tx_count);
        assert_eq!(block.prev_hash(), &prev_hash);
        assert_eq!(block.tx_hash(), &tx_hash);
        assert_eq!(block.state_hash(), &state_hash);
        let json_str = ::serde_json::to_string(&block).unwrap();
        let block1: Block = ::serde_json::from_str(&json_str).unwrap();
        assert_eq!(block1, block);
    }
}
