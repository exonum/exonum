use time::Timespec;

use super::super::crypto::{Hash, hash};

pub const BLOCK_SIZE: usize = 116;

storage_value!(
    Block {
        const SIZE = BLOCK_SIZE;

        height:                 u64         [00 => 08]
        time:                   Timespec    [08 => 16]
        prev_hash:              &Hash       [16 => 48]
        tx_hash:                &Hash       [48 => 80]
        state_hash:             &Hash       [80 => 112]
        proposer:               u32         [112 => 116]
    }
);

// TODO: add network_id, propose_round, block version?


#[test]
fn test_block() {
    let height = 123_345;
    let time = ::time::get_time();
    let prev_hash = hash(&[1, 2, 3]);
    let tx_hash = hash(&[4, 5, 6]);
    let state_hash = hash(&[7, 8, 9]);
    let proposer = 10;
    let block = Block::new(height, time, &prev_hash, &tx_hash, &state_hash, proposer);

    assert_eq!(block.height(), height);
    assert_eq!(block.time(), time);
    assert_eq!(block.prev_hash(), &prev_hash);
    assert_eq!(block.tx_hash(), &tx_hash);
    assert_eq!(block.state_hash(), &state_hash);
    assert_eq!(block.proposer(), proposer);
}
