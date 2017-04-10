use std::time::SystemTime;

use super::super::crypto::{Hash, hash};
use serde::{Serialize, Serializer, Deserialize, Deserializer};
use ::messages::utils::{U64, SystemTimeSerdeHelper};

pub const BLOCK_SIZE: usize = 120;

storage_value!(
    Block {
        const SIZE = BLOCK_SIZE;

        height:                 u64         [00 => 08]
        propose_round:          u32         [08 => 12]
        prev_hash:              &Hash       [12 => 44]
        tx_hash:                &Hash       [44 => 76]
        state_hash:             &Hash       [76 => 108]
        time:                   SystemTime  [108 => 120]
    }
);

#[derive(Serialize, Deserialize)]
struct BlockSerdeHelper {
    height: U64,
    propose_round: u32,
    prev_hash: Hash,
    tx_hash: Hash,
    state_hash: Hash,
    time: SystemTimeSerdeHelper,
}

impl Serialize for Block {
    fn serialize<S>(&self, ser: &mut S) -> Result<(), S::Error>
        where S: Serializer
    {
        let helper = BlockSerdeHelper{
            height: U64(self.height()), 
            propose_round: self.propose_round(),
            prev_hash: *self.prev_hash(), 
            tx_hash: *self.tx_hash(), 
            state_hash: *self.state_hash(),
            time: SystemTimeSerdeHelper(self.time()),
        }; 
        helper.serialize(ser)
    }
}
impl Deserialize for Block {
    fn deserialize<D>(deserializer: &mut D) -> Result<Self, D::Error>
        where D: Deserializer
    {
        let helper = <BlockSerdeHelper>::deserialize(deserializer)?; 

        let block = Block::new(helper.height.0, helper.propose_round, &helper.prev_hash, &helper.tx_hash, &helper.state_hash, helper.time.0);
        Ok(block)
    }
}
// TODO: add network_id, block version?


#[test]
fn test_block() {
    let height = 123_345;
    let prev_hash = hash(&[1, 2, 3]);
    let tx_hash = hash(&[4, 5, 6]);
    let state_hash = hash(&[7, 8, 9]);
    let round = 2;
    let time = SystemTime::now();
    let block = Block::new(height, round, &prev_hash, &tx_hash, &state_hash, time);

    assert_eq!(block.height(), height);
    assert_eq!(block.prev_hash(), &prev_hash);
    assert_eq!(block.tx_hash(), &tx_hash);
    assert_eq!(block.state_hash(), &state_hash);
    assert_eq!(block.propose_round(), round);
    assert_eq!(block.time(), time);
    use serde_json;
    let json_str = serde_json::to_string(&block).unwrap();
    let block1 : Block = serde_json::from_str(&json_str).unwrap(); 
    assert_eq!(block1,block);
}
