use serde_json;

use std::collections::{BTreeMap, HashSet};

use storage::StorageValue;
use events::Milliseconds;
use crypto::{hash, PublicKey, Hash};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StoredConfiguration {
    pub previous_cfg_hash: Hash, 
    pub actual_from: u64,
    /// List of validator's consensus and service public keys.
    pub validators: Vec<(PublicKey, PublicKey)>,
    pub consensus: ConsensusConfig,
    pub services: BTreeMap<String, serde_json::Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ConsensusConfig {
    pub round_timeout: Milliseconds,
    pub status_timeout: Milliseconds,
    pub peers_timeout: Milliseconds,
    pub propose_timeout: Milliseconds,
    pub txs_block_limit: u32,
}

impl Default for ConsensusConfig {
    fn default() -> Self {
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
    pub fn try_serialize(&self) -> Result<Vec<u8>, serde_json::error::Error> {
        serde_json::to_vec(&self)
    }

    pub fn try_deserialize(serialized: &[u8]) -> Result<StoredConfiguration, serde_json::error::Error> {
        let config: StoredConfiguration = serde_json::from_slice(serialized)?;

        // Check that there are no duplicated keys.
        let mut keys: HashSet<_> = config.validators.iter().map(|x| x.0).collect();
        keys.extend(config.validators.iter().map(|x| x.1));
        if keys.len() != config.validators.len() * 2 {
            panic!("Duplicated validator keys are found");
        }

        Ok(config)
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
