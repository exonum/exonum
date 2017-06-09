use serde::de::Error;
use serde_json::{self, Error as JsonError};

use std::collections::{BTreeMap, HashSet};

use storage::StorageValue;
use events::Milliseconds;
use crypto::{hash, PublicKey, Hash};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StoredConfiguration {
    pub previous_cfg_hash: Hash, 
    pub actual_from: u64,
    /// List of validator's consensus and service public keys.
    pub validator_keys: Vec<PublicKey>,
    pub service_keys: Vec<PublicKey>,
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
    pub fn try_serialize(&self) -> Result<Vec<u8>, JsonError> {
        serde_json::to_vec(&self)
    }

    pub fn try_deserialize(serialized: &[u8]) -> Result<StoredConfiguration, JsonError> {
        let config: StoredConfiguration = serde_json::from_slice(serialized)?;

        if config.validator_keys.len() != config.service_keys.len() {
            return Err(
                JsonError::custom("The amount of validator and service keys should be equal"));
        }

        {
            // Check that there are no duplicated keys.
            let keys: HashSet<_> = config
                .validator_keys
                .iter()
                .chain(config.service_keys.iter())
                .collect();
            if keys.len() != config.validator_keys.len() * 2 {
                return Err(JsonError::custom("Duplicated validator keys are found"));
            }
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

#[cfg(test)]
mod tests {
    use toml;
    use super::*;

    // TOML doesn't support all rust types, but `StoredConfiguration` must be able to save as TOML.
    #[test]
    fn stored_configuration_toml() {
        let original = StoredConfiguration {
            previous_cfg_hash: Hash::zero(),
            actual_from: 42,
            validator_keys: vec![PublicKey::zero()],
            service_keys: vec![PublicKey::zero()],
            consensus: ConsensusConfig::default(),
            services: BTreeMap::new(),
        };
        let toml = toml::to_string(&original).unwrap();
        let deserialized: StoredConfiguration = toml::from_str(&toml).unwrap();
        assert_eq!(original, deserialized);
    }
}
