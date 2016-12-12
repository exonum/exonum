use node::serde_json;
use events::Channel;
use node::{ExternalMessage, NodeHandler, NodeTimeout};
use super::super::messages::{ConfigPropose, ConfigVote};
use super::super::blockchain::{Blockchain, View};
use super::super::blockchain::HeightBytecode;
use super::super::crypto::PublicKey;
use super::super::storage::Map;
use super::super::messages::Message;
use byteorder::{ByteOrder, LittleEndian};

#[derive(Debug, Serialize, Deserialize)]
pub struct StoredConfiguration {
    actual_from: u64,
    pub validators: Vec<PublicKey>,
    pub consensus: ConsensusCfg
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConsensusCfg {
    pub round_timeout: i64,    // 2000
    pub status_timeout: i64,   // 5000
    pub peers_timeout: i64,    // 10000
    pub propose_timeout: i64,  // 500
    pub txs_block_limit: u32   // 500
}

impl StoredConfiguration {

    #[allow(dead_code)]
    pub fn serialize(&self) -> Vec<u8> {
        serde_json::to_vec(&self).unwrap()
    }

    #[allow(dead_code)]
    pub fn deserialize(serialized: &[u8]) -> Result<StoredConfiguration, &str> {
        let cfg: StoredConfiguration = serde_json::from_slice(serialized).unwrap();
        if cfg.is_valid() {
            return Ok(cfg);
        }
        Err("not valid")
    }

    pub fn height_to_slice(height: u64) -> HeightBytecode {
        let mut result = [0; 8];
        LittleEndian::write_u64(&mut result[0..], height);
        result
    }

    // RFU
    // fn height_from_slice(slice: [u8; 8]) -> u64 {
    //     LittleEndian::read_u64(&slice)
    // }

    fn is_valid(&self) -> bool {
        self.consensus.round_timeout < 10000
    }

}
