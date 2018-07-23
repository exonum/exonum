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

//! Exonum global variables which are stored in the blockchain as UTF-8 encoded
//! JSON.
//!
//! This module includes all the elements of the `StoredConfiguration` which is
//! used as the global configuration of the blockchain and should be the same for
//! all validators in the network. The configuration includes the public keys of
//! validators, consensus related parameters, hash of the previous configuration,
//! etc.

use serde::de::Error;
use serde_json::{self, Error as JsonError};

use std::collections::{BTreeMap, HashSet};

use crypto::{hash, CryptoHash, Hash, PublicKey, SIGNATURE_LENGTH};
use helpers::{Height, Milliseconds};
use messages::HEADER_LENGTH;
use storage::StorageValue;

/// Public keys of a validator. Each validator has two public keys: the
/// `consensus_key` is used for internal operations in the consensus process,
/// while the `service_key` is used in services.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ValidatorKeys {
    /// Consensus key is used for messages related to the consensus algorithm.
    pub consensus_key: PublicKey,
    /// Service key is used for services, for example, the configuration
    /// updater service, the anchoring service, etc.
    pub service_key: PublicKey,
}

/// Exonum blockchain global configuration. Services
/// and their parameters are also included into this configuration.
///
/// This configuration must be the same for any Exonum node in a certain
/// network on the given height.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StoredConfiguration {
    /// Hash of the previous configuration, which can be used to find that
    /// configuration. For the configuration in the genesis block,
    /// `hash` is just an array of zeros.
    pub previous_cfg_hash: Hash,
    /// The height, starting from which this configuration becomes actual. Note
    /// that this height should be big enough for the nodes to accept the new
    /// configuration before this height is reached. Otherwise, the new
    /// configuration will not take effect at all; the old configuration will
    /// remain actual.
    pub actual_from: Height,
    /// List of validators consensus and service public keys.
    pub validator_keys: Vec<ValidatorKeys>,
    /// Consensus algorithm parameters.
    pub consensus: ConsensusConfig,
    /// Number of votes required to commit the new configuration.
    /// This value should be greater than 2/3 and less or equal to the
    /// validators count.
    pub majority_count: Option<u16>,
    /// Services specific variables.
    /// Keys are `service_name` from the `Service` trait and values are the serialized JSON.
    #[serde(default)]
    pub services: BTreeMap<String, serde_json::Value>,
}

/// Consensus algorithm parameters.
///
/// This configuration is initially created with default recommended values,
/// which can later be edited as required.
/// The parameters in this configuration should be the same for all nodes in the network and can
/// be changed using the
/// [configuration updater service](https://exonum.com/doc/advanced/configuration-updater/).
///
/// Default propose timeout value, along with the threshold, is chosen for maximal performance. In order
/// to slow down block generation,hence consume less disk space, these values can be increased.
///
/// For additional information on the Exonum consensus algorithm, refer to
/// [Consensus in Exonum](https://exonum.com/doc/architecture/consensus/).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ConsensusConfig {
    /// Interval between rounds. This interval defines the time that passes
    /// between the moment a new block is committed to the blockchain and the
    /// time when a new round starts, regardless of whether a new block has
    /// been committed during this period or not.
    ///
    /// Note that rounds in Exonum
    /// do not have a defined end time. Nodes in a new round can
    /// continue to vote for proposals and process messages related to previous
    /// rounds.
    pub round_timeout: Milliseconds,
    /// Period of sending a Status message. This parameter defines the frequency
    /// with which a node broadcasts its status message to the network.
    pub status_timeout: Milliseconds,
    /// Peer exchange timeout. This parameter defines the frequency with which
    /// a node requests collected `Connect` messages from a random peer
    /// node in the network.
    pub peers_timeout: Milliseconds,
    /// Maximum number of transactions per block.
    pub txs_block_limit: u32,
    /// Maximum message length (in bytes). This parameter determines the maximum
    /// size of both consensus messages and transactions. The default value of the
    /// parameter is 1 MB (1024 * 1024 bytes). The range of possible values for this
    /// parameter is between 1MB and 2^32-1 bytes.
    pub max_message_len: u32,
    /// Minimal propose timeout.
    pub min_propose_timeout: Milliseconds,
    /// Maximal propose timeout.
    pub max_propose_timeout: Milliseconds,
    /// Amount of transactions in pool to start use `min_propose_timeout`.
    ///
    /// Default value is equal to half of the `txs_block_limit` in order to gather more transactions
    /// in a block if the transaction pool is almost empty, and create blocks faster when there are
    /// enough transactions in the pool.
    pub propose_timeout_threshold: u32,
}

impl ConsensusConfig {
    /// Default value for max_message_len.
    pub const DEFAULT_MAX_MESSAGE_LEN: u32 = 1024 * 1024; // 1 MB

    /// Produces warnings if configuration contains non-optimal values.
    ///
    /// Validation for logical correctness is performed in the `StoredConfiguration::try_deserialize`
    /// method, but some values can decrease consensus performance.
    #[doc(hidden)]
    pub fn warn_if_nonoptimal(&self) {
        const MIN_TXS_BLOCK_LIMIT: u32 = 100;
        const MAX_TXS_BLOCK_LIMIT: u32 = 10_000;

        if self.round_timeout <= 2 * self.max_propose_timeout {
            warn!(
                "It is recommended that round_timeout ({}) be at least twice as large \
                 as max_propose_timeout ({})",
                self.round_timeout, self.max_propose_timeout
            );
        }

        if self.txs_block_limit < MIN_TXS_BLOCK_LIMIT || self.txs_block_limit > MAX_TXS_BLOCK_LIMIT
        {
            warn!(
                "It is recommended that txs_block_limit ({}) is in [{}..{}] range",
                self.txs_block_limit, MIN_TXS_BLOCK_LIMIT, MAX_TXS_BLOCK_LIMIT
            );
        }

        if self.max_message_len < Self::DEFAULT_MAX_MESSAGE_LEN {
            warn!(
                "It is recommended that max_message_len ({}) is at least {}.",
                self.max_message_len,
                Self::DEFAULT_MAX_MESSAGE_LEN
            );
        }
    }
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self {
            round_timeout: 3000,
            status_timeout: 5000,
            peers_timeout: 10_000,
            txs_block_limit: 1000,
            max_message_len: Self::DEFAULT_MAX_MESSAGE_LEN,
            min_propose_timeout: 10,
            max_propose_timeout: 200,
            propose_timeout_threshold: 500,
        }
    }
}

impl StoredConfiguration {
    /// Tries to serialize the given configuration into a UTF-8 encoded JSON.
    /// The method returns either the result of execution or an error.
    pub fn try_serialize(&self) -> Result<Vec<u8>, JsonError> {
        serde_json::to_vec(&self)
    }

    /// Tries to deserialize `StorageConfiguration` from the given UTF-8 encoded
    /// JSON. Additionally, this method performs a logic validation of the
    /// configuration. The method returns either the result of execution or an error.
    pub fn try_deserialize(serialized: &[u8]) -> Result<Self, JsonError> {
        const MINIMAL_BODY_SIZE: usize = 256;
        const MINIMAL_MESSAGE_LENGTH: u32 =
            (HEADER_LENGTH + MINIMAL_BODY_SIZE + SIGNATURE_LENGTH) as u32;

        let config: Self = serde_json::from_slice(serialized)?;

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

        // Check timeouts.
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

        // Check transactions limit.
        if config.consensus.txs_block_limit == 0 {
            return Err(JsonError::custom(
                "txs_block_limit should not be equal to zero",
            ));
        }

        // Check maximum message length for sanity.
        if config.consensus.max_message_len < MINIMAL_MESSAGE_LENGTH {
            return Err(JsonError::custom(format!(
                "max_message_len ({}) must be at least {}",
                config.consensus.max_message_len, MINIMAL_MESSAGE_LENGTH
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
        Self::try_deserialize(v.as_ref()).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use toml;

    use super::*;
    use crypto::{gen_keypair_from_seed, Seed, SEED_LENGTH};

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
            min_propose_timeout = 10
            max_propose_timeout = 200
            propose_timeout_threshold = 500
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
    fn duplicated_validators_keys() {
        let mut configuration = create_test_configuration();
        configuration.validator_keys.push(ValidatorKeys {
            consensus_key: PublicKey::zero(),
            service_key: PublicKey::zero(),
        });
        serialize_deserialize(&configuration);
    }

    #[test]
    #[should_panic(expected = "Invalid propose timeouts: min_propose_timeout should be less or")]
    fn min_max_propose_timeouts() {
        let mut configuration = create_test_configuration();
        configuration.consensus.min_propose_timeout = 10;
        configuration.consensus.max_propose_timeout = 0;
        serialize_deserialize(&configuration);
    }

    #[test]
    #[should_panic(
        expected = "round_timeout(50) must be strictly larger than max_propose_timeout(50)"
    )]
    fn invalid_round_timeout() {
        let mut configuration = create_test_configuration();
        configuration.consensus.round_timeout = 50;
        configuration.consensus.max_propose_timeout = 50;
        serialize_deserialize(&configuration);
    }

    #[test]
    #[should_panic(expected = "txs_block_limit should not be equal to zero")]
    fn invalid_txs_block_limit() {
        let mut configuration = create_test_configuration();
        configuration.consensus.txs_block_limit = 0;
        serialize_deserialize(&configuration);
    }

    #[test]
    #[should_panic(expected = "max_message_len (128) must be at least 330")]
    fn too_small_max_message_len() {
        let mut configuration = create_test_configuration();
        configuration.consensus.max_message_len = 128;
        serialize_deserialize(&configuration);
    }

    fn create_test_configuration() -> StoredConfiguration {
        let validator_keys = (1..4)
            .map(|i| ValidatorKeys {
                consensus_key: gen_keypair_from_seed(&Seed::new([i; SEED_LENGTH])).0,
                service_key: gen_keypair_from_seed(&Seed::new([i * 10; SEED_LENGTH])).0,
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
