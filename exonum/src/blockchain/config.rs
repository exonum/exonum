use serde_json;
use ::storage::StorageValue;
use std::collections::BTreeMap;


use ::crypto::{hash, PublicKey, Hash};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StoredConfiguration {
    pub previous_cfg_hash: Hash, 
    pub actual_from: u64,
    pub validators: Vec<PublicKey>,
    pub consensus: ConsensusConfig,
    pub services: BTreeMap<String, serde_json::Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ConsensusConfig {
    pub round_timeout: i64,
    pub status_timeout: i64,
    pub peers_timeout: i64,
    pub propose_timeout: i64,
    pub txs_block_limit: u32,
}

impl Default for ConsensusConfig {
    fn default() -> ConsensusConfig {
        ConsensusConfig {
            round_timeout: 3000,
            propose_timeout: 500,
            status_timeout: 5000,
            peers_timeout: 10000,
            txs_block_limit: 1000,
        }
    }
}

impl StoredConfiguration {
    pub fn serialize_err(&self) -> Result<Vec<u8>, serde_json::error::Error> {
        serde_json::to_vec(&self)
    }

    pub fn deserialize_err(serialized: &[u8]) -> Result<StoredConfiguration, serde_json::error::Error> {
        serde_json::from_slice(serialized)
    }
}

impl StorageValue for StoredConfiguration {
    fn serialize(self) -> Vec<u8> {
        self.serialize_err().unwrap()
    }

    fn deserialize(v: Vec<u8>) -> Self {
        StoredConfiguration::deserialize_err(&v).unwrap()
    }

    fn hash(&self) -> Hash {
        let vec_bytes = self.serialize_err().unwrap();
        hash(&vec_bytes)
    }

}
