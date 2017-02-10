use time::Timespec;

use super::super::crypto::{Hash, hash};
use super::super::messages::Field;
use super::super::storage::StorageValue;
use serde::{Serialize, Serializer, Deserialize, Deserializer};
use ::messages::utils::{U64, TimespecSerdeHelper}; 

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

#[derive(Serialize, Deserialize)]
struct BlockSerdeHelper {
   height: U64,  
   propose_round: u32, 
   time: TimespecSerdeHelper, 
   prev_hash: Hash, 
   tx_hash: Hash, 
   state_hash: Hash, 
}

impl Serialize for Block {
    fn serialize<S>(&self, ser: &mut S) -> Result<(), S::Error>
        where S: Serializer
    {
        let helper = BlockSerdeHelper{
            height: U64(self.height()), 
            propose_round: self.propose_round(), 
            time: TimespecSerdeHelper(self.time()), 
            prev_hash: *self.prev_hash(), 
            tx_hash: *self.tx_hash(), 
            state_hash: *self.state_hash(), 
        }; 
        helper.serialize(ser)
    }
}
impl Deserialize for Block {
    fn deserialize<D>(deserializer: &mut D) -> Result<Self, D::Error>
        where D: Deserializer
    {
        let helper = <BlockSerdeHelper>::deserialize(deserializer)?; 

        let block = Block::new(helper.height.0, helper.propose_round, helper.time.0, &helper.prev_hash, &helper.tx_hash, &helper.state_hash); 
        Ok(block)
    }
}
// TODO: add network_id, block version?

// TODO add generic implementation for whole storage values
impl<'a> Field<'a> for Block {
    fn field_size() -> usize {
        8
    }

    fn read(buffer: &'a [u8], from: usize, to: usize) -> Block {
        let data = <&[u8] as Field>::read(buffer, from, to);
        <Block as StorageValue>::deserialize(data.to_vec())
    }

    fn write(&self, buffer: &'a mut Vec<u8>, from: usize, to: usize) {
        <&[u8] as Field>::write(&self.clone().serialize().as_slice(), buffer, from, to);
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
