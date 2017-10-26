// Copyright 2017 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Exonum global variables which stored in blockchain as utf8 encoded json.

use serde::de::Error;
use serde_json::{self, Error as JsonError};

use std::collections::{BTreeMap, HashSet};

use storage::StorageValue;
use crypto::{hash, PublicKey, Hash};
use helpers::{Height, Milliseconds};

/// Public keys of a validator.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    pub actual_from: Height,
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
    /// Maximum number of transactions per block.
    pub txs_block_limit: u32,
    /// `TimeoutAdjuster` configuration.
    pub timeout_adjuster: TimeoutAdjusterConfig,
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        ConsensusConfig {
            round_timeout: 3000,
            status_timeout: 5000,
            peers_timeout: 10_000,
            txs_block_limit: 1000,
            timeout_adjuster: TimeoutAdjusterConfig::Constant { timeout: 500 },
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
                return Err(JsonError::custom(
                    "Duplicated keys are found: each consensus and service key must be unique",
                ));
            }
        }

        // Check timeout adjuster.
        match config.consensus.timeout_adjuster {
            // There is no need to validate `Constant` timeout adjuster.
            TimeoutAdjusterConfig::Constant { .. } => (),
            TimeoutAdjusterConfig::Dynamic { min, max, .. } => {
                if min >= max {
                    return Err(JsonError::custom(format!(
                        "Dynamic adjuster: minimal timeout should be less then maximal: \
                        min = {}, max = {}",
                        min,
                        max
                    )));
                }
            }
            TimeoutAdjusterConfig::MovingAverage {
                min,
                max,
                adjustment_speed,
                optimal_block_load,
            } => {
                if min >= max {
                    return Err(JsonError::custom(format!(
                        "Moving average adjuster: minimal timeout must be less then maximal: \
                        min = {}, max = {}",
                        min,
                        max
                    )));
                }
                if adjustment_speed <= 0. || adjustment_speed > 1. {
                    return Err(JsonError::custom(format!(
                        "Moving average adjuster: adjustment speed must be in the (0..1] range: {}",
                        adjustment_speed,
                    )));
                }
                if optimal_block_load <= 0. || optimal_block_load > 1. {
                    return Err(JsonError::custom(format!(
                        "Moving average adjuster: block load must be in the (0..1] range: {}",
                        adjustment_speed,
                    )));
                }
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

/// `TimeoutAdjuster` config.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type")]
pub enum TimeoutAdjusterConfig {
    /// Constant timeout adjuster config.
    Constant {
        /// Timeout value.
        timeout: Milliseconds,
    },
    /// Dynamic timeout adjuster configuration.
    Dynamic {
        /// Minimal timeout.
        min: Milliseconds,
        /// Maximal timeout.
        max: Milliseconds,
        /// Transactions threshold starting from which the adjuster returns the minimal timeout.
        threshold: u32,
    },
    /// Moving average timeout adjuster configuration.
    MovingAverage {
        /// Minimal timeout.
        min: Milliseconds,
        /// Maximal timeout.
        max: Milliseconds,
        /// Speed of the adjustment.
        adjustment_speed: f64,
        /// Optimal block load depending on the `txs_block_limit` from the `ConsensusConfig`.
        optimal_block_load: f64,
    },
}

#[cfg(test)]
mod tests {
    use toml;
    use serde::{Serialize, Deserialize};

    use std::fmt::Debug;

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
        assert_eq!(configuration, serialize_deserialize(&configuration));
    }

    #[test]
    #[should_panic(expected = "Duplicated keys are found")]
    fn stored_configuration_duplicated_keys() {
        let mut configuration = create_test_configuration();
        configuration.validator_keys.push(ValidatorKeys {
            consensus_key: PublicKey::zero(),
            service_key: PublicKey::zero(),
        });
        serialize_deserialize(&configuration);
    }

    #[test]
    fn constant_adjuster_config_toml() {
        let config = TimeoutAdjusterConfig::Constant { timeout: 500 };
        check_toml_roundtrip(&config);
    }

    #[test]
    fn dynamic_adjuster_config_toml() {
        let config = TimeoutAdjusterConfig::Dynamic {
            min: 1,
            max: 1000,
            threshold: 10,
        };
        check_toml_roundtrip(&config);
    }

    #[test]
    fn moving_average_adjuster_config_toml() {
        let config = TimeoutAdjusterConfig::MovingAverage {
            min: 1,
            max: 1000,
            adjustment_speed: 0.5,
            optimal_block_load: 0.75,
        };
        check_toml_roundtrip(&config);
    }

    #[test]
    #[should_panic(expected = "Dynamic adjuster: minimal timeout should be less then maximal")]
    fn dynamic_adjuster_min_max() {
        let mut configuration = create_test_configuration();
        configuration.consensus.timeout_adjuster = TimeoutAdjusterConfig::Dynamic {
            min: 10,
            max: 0,
            threshold: 1,
        };
        serialize_deserialize(&configuration);
    }

    #[test]
    #[should_panic(expected = "Moving average adjuster: minimal timeout must be less then maximal")]
    fn moving_average_adjuster_min_max() {
        let mut configuration = create_test_configuration();
        configuration.consensus.timeout_adjuster = TimeoutAdjusterConfig::MovingAverage {
            min: 10,
            max: 0,
            adjustment_speed: 0.7,
            optimal_block_load: 0.5,
        };
        serialize_deserialize(&configuration);
    }

    // TODO: Remove `#[rustfmt_skip]` after https://github.com/rust-lang-nursery/rustfmt/issues/1777
    // is fixed.
    #[cfg_attr(rustfmt, rustfmt_skip)]
    #[test]
    #[should_panic(expected = "Moving average adjuster: adjustment speed must be in the (0..1]")]
    fn moving_average_adjuster_negative_adjustment_speed() {
        let mut configuration = create_test_configuration();
        configuration.consensus.timeout_adjuster = TimeoutAdjusterConfig::MovingAverage {
            min: 1,
            max: 20,
            adjustment_speed: -0.7,
            optimal_block_load: 0.5,
        };
        serialize_deserialize(&configuration);
    }

    // TODO: Remove `#[rustfmt_skip]` after https://github.com/rust-lang-nursery/rustfmt/issues/1777
    // is fixed.
    #[cfg_attr(rustfmt, rustfmt_skip)]
    #[test]
    #[should_panic(expected = "Moving average adjuster: adjustment speed must be in the (0..1]")]
    fn moving_average_adjuster_invalid_adjustment_speed() {
        let mut configuration = create_test_configuration();
        configuration.consensus.timeout_adjuster = TimeoutAdjusterConfig::MovingAverage {
            min: 10,
            max: 20,
            adjustment_speed: 1.5,
            optimal_block_load: 0.5,
        };
        serialize_deserialize(&configuration);
    }

    // TODO: Remove `#[rustfmt_skip]` after https://github.com/rust-lang-nursery/rustfmt/issues/1777
    // is fixed.
    #[cfg_attr(rustfmt, rustfmt_skip)]
    #[test]
    #[should_panic(expected = "Moving average adjuster: block load must be in the (0..1] range")]
    fn moving_average_adjuster_negative_block_load() {
        let mut configuration = create_test_configuration();
        configuration.consensus.timeout_adjuster = TimeoutAdjusterConfig::MovingAverage {
            min: 10,
            max: 20,
            adjustment_speed: 0.7,
            optimal_block_load: -0.5,
        };
        serialize_deserialize(&configuration);
    }

    // TODO: Remove `#[rustfmt_skip]` after https://github.com/rust-lang-nursery/rustfmt/issues/1777
    // is fixed.
    #[cfg_attr(rustfmt, rustfmt_skip)]
    #[test]
    #[should_panic(expected = "Moving average adjuster: block load must be in the (0..1] range")]
    fn moving_average_adjuster_invalid_block_load() {
        let mut configuration = create_test_configuration();
        configuration.consensus.timeout_adjuster = TimeoutAdjusterConfig::MovingAverage {
            min: 10,
            max: 20,
            adjustment_speed: 0.7,
            optimal_block_load: 2.0,
        };
        serialize_deserialize(&configuration);
    }

    fn create_test_configuration() -> StoredConfiguration {
        let validator_keys = (1..4)
            .map(|i| {
                ValidatorKeys {
                    consensus_key: gen_keypair_from_seed(&Seed::new([i; 32])).0,
                    service_key: gen_keypair_from_seed(&Seed::new([i * 10; 32])).0,
                }
            })
            .collect();

        StoredConfiguration {
            previous_cfg_hash: Hash::zero(),
            actual_from: Height(42),
            validator_keys,
            consensus: ConsensusConfig::default(),
            services: BTreeMap::new(),
        }
    }

    fn serialize_deserialize(configuration: &StoredConfiguration) -> StoredConfiguration {
        let serialized = configuration.try_serialize().unwrap();
        StoredConfiguration::try_deserialize(&serialized).unwrap()
    }

    fn check_toml_roundtrip<T>(original: &T)
    where
        for<'de> T: Serialize + Deserialize<'de> + PartialEq + Debug,
    {
        let toml = toml::to_string(original).unwrap();
        let deserialized: T = toml::from_str(&toml).unwrap();
        assert_eq!(*original, deserialized);
    }
}
