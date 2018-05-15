// Copyright 2018 The Exonum Team
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
use crypto::{hash, CryptoHash, Hash, PublicKey};
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
    /// For configuration in the genesis block `hash` is just an array of zeros.
    pub previous_cfg_hash: Hash,
    /// The height, starting from which this configuration becomes actual.
    pub actual_from: Height,
    /// List of validators' consensus and service public keys.
    pub validator_keys: Vec<ValidatorKeys>,
    /// Consensus algorithm parameters.
    pub consensus: ConsensusConfig,
    /// Number of votes required to commit new configuration.
    /// Should be greater than 2/3 and less or equal to the validators count.
    pub majority_count: Option<u16>,
    /// Services specific variables.
    /// Keys are `service_name` from `Service` trait and values are the serialized json.
    #[serde(default)]
    pub services: BTreeMap<String, serde_json::Value>,
}

/// Consensus algorithm parameters.
///
/// Default propose timeout values along with threshold are chosen for maximal performance. In order
/// to slow down blocks generation (hence consume less disk space) these values can be increased.
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
    /// Maximum message length (in bytes).
    pub max_message_len: u32,
    /// Minimal propose timeout.
    pub min_propose_timeout: Milliseconds,
    /// Maximal propose timeout.
    pub max_propose_timeout: Milliseconds,
    /// Transactions threshold starting from which the `min_propose_timeout` value is used.
    pub propose_timeout_threshold: u32,
}

impl ConsensusConfig {
    /// Default value for max_message_len.
    pub const DEFAULT_MAX_MESSAGE_LEN: u32 = 1024 * 1024; // 1 MB

    /// Checks if propose timeout is less than round timeout. Warns if fails.
    #[doc(hidden)]
    pub fn validate_configuration(&self) {
        if self.round_timeout <= 2 * self.max_propose_timeout {
            warn!(
                "It is recommended that round_timeout ({}) be at least twice as large \
                 as max_propose_timeout ({})",
                self.round_timeout, self.max_propose_timeout
            );
        }
    }
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        ConsensusConfig {
            round_timeout: 3000,
            status_timeout: 5000,
            peers_timeout: 10_000,
            txs_block_limit: 1000,
            max_message_len: Self::DEFAULT_MAX_MESSAGE_LEN,
            min_propose_timeout: 10,
            max_propose_timeout: 200,
            propose_timeout_threshold: 1,
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

        if config.consensus.min_propose_timeout > config.consensus.max_propose_timeout {
            return Err(JsonError::custom(format!(
                "Invalid propose timeouts: min_propose_timeout should be less or equal then \
                 max_propose_timeout: min = {}, max = {}",
                config.consensus.min_propose_timeout, config.consensus.max_propose_timeout
            )));
        }

        if config.consensus.round_timeout <= config.consensus.max_propose_timeout {
            return Err(JsonError::custom(format!(
                "round_timeout({}) must be strictly larger than max_propose_timeout({})",
                config.consensus.round_timeout, config.consensus.max_propose_timeout
            )));
        }

        Ok(config)
    }
}

impl CryptoHash for StoredConfiguration {
    fn hash(&self) -> Hash {
        let vec_bytes = self.try_serialize().unwrap();
        hash(&vec_bytes)
    }
}

impl StorageValue for StoredConfiguration {
    fn into_bytes(self) -> Vec<u8> {
        self.try_serialize().unwrap()
    }

    fn from_bytes(v: ::std::borrow::Cow<[u8]>) -> Self {
        StoredConfiguration::try_deserialize(v.as_ref()).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use toml;
    use serde::{Deserialize, Serialize};

    use std::fmt::Debug;

    use crypto::{gen_keypair_from_seed, Seed};
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
    fn stored_configuration_parse_from_toml() {
        let toml_content = r#"
            previous_cfg_hash = "0000000000000000000000000000000000000000000000000000000000000000"
            actual_from = 42

            [[validator_keys]]
            consensus_key = "8a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c"
            service_key = "43a72e714401762df66b68c26dfbdf2682aaec9f2474eca4613e424a0fbafd3c"

            [[validator_keys]]
            consensus_key = "8139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b394"
            service_key = "20828bf5c5bdcacb684863336c202fb5599da48be5596615742170705beca9f7"

            [[validator_keys]]
            consensus_key = "ed4928c628d1c2c6eae90338905995612959273a5c63f93636c14614ac8737d1"
            service_key = "acdb0e29743f0ccb8686d0a104cb96e05abefec1538765e7595869f7dc8c49aa"

            [consensus]
            round_timeout = 3000
            status_timeout = 5000
            peers_timeout = 10000
            txs_block_limit = 1000
            max_message_len = 1048576

            [consensus.timeout_adjuster]
            type = "Constant"
            timeout = 500
            "#;

        let origin = create_test_configuration();
        let from_toml = toml::from_str(toml_content).unwrap();
        assert_eq!(origin, from_toml);
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
    #[should_panic(expected = "Invalid propose timeouts: min_propose_timeout should be less or")]
    fn dynamic_adjuster_min_max() {
        let mut configuration = create_test_configuration();
        configuration.consensus.min_propose_timeout = 10;
        configuration.consensus.max_propose_timeout = 0;
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

    #[test]
    #[should_panic(expected = "round_timeout(50) must be strictly larger than propose_timeout(50)")]
    fn constant_adjuster_invalid_timeout() {
        let mut configuration = create_test_configuration();
        configuration.consensus.round_timeout = 50;
        configuration.consensus.timeout_adjuster = TimeoutAdjusterConfig::Constant { timeout: 50 };
        serialize_deserialize(&configuration);
    }

    #[test]
    #[should_panic(expected = "round_timeout(50) must be strictly larger than propose_timeout(50)")]
    fn dynamic_adjuster_invalid_timeout() {
        let mut configuration = create_test_configuration();
        configuration.consensus.round_timeout = 50;
        configuration.consensus.timeout_adjuster = TimeoutAdjusterConfig::Dynamic {
            min: 10,
            max: 50,
            threshold: 1,
        };
        serialize_deserialize(&configuration);
    }

    #[test]
    #[should_panic(expected = "round_timeout(50) must be strictly larger than propose_timeout(50)")]
    fn moving_average_adjuster_invalid_timeout() {
        let mut configuration = create_test_configuration();
        configuration.consensus.round_timeout = 50;
        configuration.consensus.timeout_adjuster = TimeoutAdjusterConfig::MovingAverage {
            min: 10,
            max: 50,
            adjustment_speed: 0.7,
            optimal_block_load: 0.2,
        };
        serialize_deserialize(&configuration);
    }

    fn create_test_configuration() -> StoredConfiguration {
        let validator_keys = (1..4)
            .map(|i| ValidatorKeys {
                consensus_key: gen_keypair_from_seed(&Seed::new([i; 32])).0,
                service_key: gen_keypair_from_seed(&Seed::new([i * 10; 32])).0,
            })
            .collect();

        StoredConfiguration {
            previous_cfg_hash: Hash::zero(),
            actual_from: Height(42),
            validator_keys,
            consensus: ConsensusConfig::default(),
            services: BTreeMap::new(),
            majority_count: None,
        }
    }

    fn serialize_deserialize(configuration: &StoredConfiguration) -> StoredConfiguration {
        let serialized = configuration.try_serialize().unwrap();
        StoredConfiguration::try_deserialize(&serialized).unwrap()
    }
}
