//! Exonum global variables which stored in blockchain as utf8 encoded json.

use serde::de::Error;
use serde_json::{self, Error as JsonError};

use std::collections::{BTreeMap, HashSet};

use storage::StorageValue;
use events::Milliseconds;
use crypto::{hash, PublicKey, Hash};

/// Public keys of a validator.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ValidatorKeys {
    /// Consensus key is used for messages related to the consensus algorithm.
    #[doc(hidden)]
    pub consensus_key: PublicKey,
    /// Service key is used for services.
    pub service_key: PublicKey,
}

/// Exonum blockchain global configuration.
/// This configuration must be same for any exonum node in the certain network on given height.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StoredConfiguration {
    /// Link to the previous configuration. 
    /// For configuration in genesis block `hash` is just an array of zeroes.
    pub previous_cfg_hash: Hash,
    /// The height, starting from which this configuration becomes actual.
    pub actual_from: u64,
    /// List of validator's consensus and service public keys.
    pub validator_keys: Vec<ValidatorKeys>,
    /// Consensus algorithm parameters.
    pub consensus: ConsensusConfig,
    /// Services specific variables.
    /// Keys are `service_name` from `Service` trait and values are the serialized json.
    pub services: BTreeMap<String, serde_json::Value>,
}

/// Consensus algorithm parameters.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ConsensusConfig {
    /// Interval between rounds.
    pub round_timeout: Milliseconds,
    /// Period of sending a Status message.
    pub status_timeout: Milliseconds,
    /// Peer exchange timeout.
    pub peers_timeout: Milliseconds,
    /// Proposal timeout after committing a block.
    pub propose_timeout: Milliseconds,
    /// Maximum number of transactions per block.
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
    /// Tries to serialize given configuration into the utf8 encoded json.
    pub fn try_serialize(&self) -> Result<Vec<u8>, JsonError> {
        serde_json::to_vec(&self)
    }

    /// Tries to deserialize `StorageConfiguration` from the given utf8 encoded json.
    pub fn try_deserialize(serialized: &[u8]) -> Result<StoredConfiguration, JsonError> {
        let config: StoredConfiguration = serde_json::from_slice(serialized)?;

        // Check that there are no duplicated keys.
        {
            let mut keys = HashSet::with_capacity(config.validator_keys.len() * 2);
            for k in &config.validator_keys {
                keys.insert(k.consensus_key);
                keys.insert(k.service_key);
            }
            if keys.len() != config.validator_keys.len() * 2 {
                return Err(JsonError::custom("Duplicated validator keys are found"));
            }
        }

        Ok(config)
    }
}

impl StorageValue for StoredConfiguration {
    fn into_bytes(self) -> Vec<u8> {
        self.try_serialize().unwrap()
    }

    fn from_bytes(v: ::std::borrow::Cow<[u8]>) -> Self {
        StoredConfiguration::try_deserialize(v.as_ref()).unwrap()
    }

    fn hash(&self) -> Hash {
        let vec_bytes = self.try_serialize().unwrap();
        hash(&vec_bytes)
    }
}

#[cfg(test)]
mod tests {
    use toml;
    use crypto::{Seed, gen_keypair_from_seed};
    use super::*;

    // TOML doesn't support all rust types, but `StoredConfiguration` must be able to save as TOML.
    #[test]
    fn stored_configuration_toml() {
        let original = create_test_configuration();
        let toml = toml::to_string(&original).unwrap();
        let deserialized: StoredConfiguration = toml::from_str(&toml).unwrap();
        assert_eq!(original, deserialized);
    }

    #[test]
    fn stored_configuration_serialize_deserialize() {
        let configuration = create_test_configuration();
        serialize_deserialize(&configuration);
    }

    #[test]
    #[should_panic(expected = "Duplicated validator keys are found")]
    fn stored_configuration_duplicated_keys() {
        let mut configuration = create_test_configuration();
        configuration.validator_keys.push(ValidatorKeys {
            consensus_key: PublicKey::zero(),
            service_key: PublicKey::zero(),
        });
        serialize_deserialize(&configuration);
    }

    fn create_test_configuration() -> StoredConfiguration {
        let validator_keys = (1..4).map(|i| ValidatorKeys {
                consensus_key: gen_keypair_from_seed(&Seed::new([i; 32])).0,
                service_key: gen_keypair_from_seed(&Seed::new([i * 10; 32])).0,
            }).collect();

        StoredConfiguration {
            previous_cfg_hash: Hash::zero(),
            actual_from: 42,
            validator_keys,
            consensus: ConsensusConfig::default(),
            services: BTreeMap::new(),
        }
    }

    fn serialize_deserialize(configuration: &StoredConfiguration) {
        let serialized = configuration.try_serialize().unwrap();
        let _ = StoredConfiguration::try_deserialize(&serialized).unwrap();
    }
}
