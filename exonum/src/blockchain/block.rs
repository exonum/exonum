use time::Timespec;

use super::super::crypto::{Hash, hash};
use super::super::messages::Field;
use super::super::storage::StorageValue;

pub const BLOCK_SIZE: usize = 116;

storage_value!(
    Block {
        const SIZE = BLOCK_SIZE;

        height:                 u64         [00 => 08]
        propose_round:          u32         [08 => 12]
        time:                   Timespec    [12 => 20]
        prev_hash:              &Hash       [20 => 52]
        tx_hash:                &Hash       [52 => 84]
        state_hash:             &Hash       [84 => 116]
    }
);

// TODO: add network_id, block version?

// TODO add generic implementation for whole storage values
impl<'a> Field<'a> for Block {
    fn field_size() -> usize {
        8
    }

    fn read(buffer: &'a [u8], from: usize, to: usize) -> Block {
        let data = <&[u8] as Field>::read(buffer, from, to);
        Block::deserialize(data.to_vec())
    }

    fn write(&self, buffer: &'a mut Vec<u8>, from: usize, to: usize) {
        <&[u8] as Field>::write(&self.serialize(Vec::new()).as_slice(), buffer, from, to);
    }
}


#[test]
fn test_block() {
    let height = 123_345;
    let time = ::time::get_time();
    let prev_hash = hash(&[1, 2, 3]);
    let tx_hash = hash(&[4, 5, 6]);
    let state_hash = hash(&[7, 8, 9]);
    let round = 2;
    let block = Block::new(height, round, time, &prev_hash, &tx_hash, &state_hash);

    assert_eq!(block.height(), height);
    assert_eq!(block.time(), time);
    assert_eq!(block.prev_hash(), &prev_hash);
    assert_eq!(block.tx_hash(), &tx_hash);
    assert_eq!(block.state_hash(), &state_hash);
    assert_eq!(block.propose_round(), round);
}
