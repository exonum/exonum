use time::Timespec;

use super::super::messages::Field;
use super::super::crypto::{Hash, hash};
use super::super::storage::StorageValue;

pub const BLOCK_SIZE: usize = 112;

#[derive(Debug)]
pub struct Block {
    // Идентификатор сети, к которой принадлежит этот блок
    // Высота блока
    // Номер раунда, на котором был принят этот блок
    // Время создания блока предлагающим лидером*
    // Хеш предыдущего блока
    // Хеш состояния после применения этого блока
    raw: Vec<u8>,
}

// TODO: add network_id, propose_round, block version?
impl Block {
    pub fn new(height: u64,
               time: Timespec,
               prev_hash: &Hash,
               tx_hash: &Hash,
               state_hash: &Hash)
               -> Block {
        let mut block = Block { raw: vec![0; BLOCK_SIZE] };

        Field::write(&height, &mut block.raw, 0, 8);
        Field::write(&time, &mut block.raw, 8, 16);
        Field::write(&prev_hash, &mut block.raw, 16, 48);
        Field::write(&tx_hash, &mut block.raw, 48, 80);
        Field::write(&state_hash, &mut block.raw, 80, 112);

        block
    }

    pub fn from_raw(raw: Vec<u8>) -> Block {
        // TODO: error instead of panic?
        assert_eq!(raw.len(), BLOCK_SIZE);
        Block { raw: raw }
    }

    pub fn height(&self) -> u64 {
        Field::read(&self.raw, 0, 8)
    }

    pub fn time(&self) -> Timespec {
        Field::read(&self.raw, 8, 16)
    }

    pub fn prev_hash(&self) -> &Hash {
        Field::read(&self.raw, 16, 48)
    }

    pub fn tx_hash(&self) -> &Hash {
        Field::read(&self.raw, 48, 80)
    }

    pub fn state_hash(&self) -> &Hash {
        Field::read(&self.raw, 80, 112)
    }

    pub fn hash(&self) -> Hash {
        hash(&self.raw)
    }
}

impl StorageValue for Block {
    fn serialize(self) -> Vec<u8> {
        self.raw
    }

    fn deserialize(v: Vec<u8>) -> Self {
        Block::from_raw(v)
    }

    fn hash(&self) -> Hash {
        Block::hash(self)
    }
}

#[test]
fn test_block() {
    let height = 123_345;
    let time = ::time::get_time();
    let prev_hash = hash(&[1, 2, 3]);
    let tx_hash = hash(&[4, 5, 6]);
    let state_hash = hash(&[7, 8, 9]);
    let block = Block::new(height, time, &prev_hash, &tx_hash, &state_hash);

    assert_eq!(block.height(), height);
    assert_eq!(block.time(), time);
    assert_eq!(block.prev_hash(), &prev_hash);
    assert_eq!(block.tx_hash(), &tx_hash);
    assert_eq!(block.state_hash(), &state_hash);
}
