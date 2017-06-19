use serde_json;

use std::collections::BTreeMap;

use storage::StorageValue;
use events::Milliseconds;
use crypto::{hash, PublicKey, Hash};

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
    pub round_timeout: Milliseconds,
    pub status_timeout: Milliseconds,
    pub peers_timeout: Milliseconds,
    pub min_propose_timeout: Milliseconds,
    pub max_propose_timeout: Milliseconds,
    pub txs_propose_timeout_threshold: u32,
    pub txs_block_limit: u32,
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        ConsensusConfig {
            round_timeout: 3000,
            min_propose_timeout: 500,
            max_propose_timeout: 2000,
            txs_propose_timeout_threshold: 10,
            status_timeout: 5000,
            peers_timeout: 10000,
            txs_block_limit: 1000,
        }
    }
}

impl StoredConfiguration {
    pub fn try_serialize(&self) -> Result<Vec<u8>, serde_json::error::Error> {
        serde_json::to_vec(&self)
    }

    pub fn try_deserialize(serialized: &[u8]) -> Result<StoredConfiguration, serde_json::error::Error> {
        serde_json::from_slice(serialized)
    }
}

impl StorageValue for StoredConfiguration {
    fn serialize(self) -> Vec<u8> {
        self.try_serialize().unwrap()
    }

    fn deserialize(v: Vec<u8>) -> Self {
        StoredConfiguration::try_deserialize(&v).unwrap()
    }

    fn hash(&self) -> Hash {
        let vec_bytes = self.try_serialize().unwrap();
        hash(&vec_bytes)
    }

}
