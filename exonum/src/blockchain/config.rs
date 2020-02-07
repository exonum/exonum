// Copyright 2020 The Exonum Team
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

//! Exonum blockchain configuration.
//!
//! This module includes the components of the global configuration of the blockchain
//! The configuration includes the public keys of validators, consensus related parameters,
//! and built-in services (services deployed at the blockchain start).

use exonum_derive::{BinaryValue, ObjectHash};
use exonum_proto::ProtobufConvert;
use failure::{bail, ensure};
use log::warn;

use std::collections::{HashMap, HashSet};

use crate::{
    crypto::PublicKey,
    helpers::{Milliseconds, ValidateInput, ValidatorId},
    keys::Keys,
    merkledb::BinaryValue,
    messages::SIGNED_MESSAGE_MIN_SIZE,
    proto::schema,
    runtime::{ArtifactId, ArtifactSpec, InstanceId, InstanceSpec},
};

/// Public keys of a validator. Each validator has two public keys: the
/// `consensus_key` is used for internal operations in the consensus process,
/// while the `service_key` is used in services.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert)]
#[protobuf_convert(source = "schema::blockchain::ValidatorKeys")]
pub struct ValidatorKeys {
    /// Consensus key is used for messages related to the consensus algorithm.
    pub consensus_key: PublicKey,
    /// Service key is used to sign transactions broadcast by the services.
    pub service_key: PublicKey,

    /// No-op field for forward compatibility.
    #[protobuf_convert(skip)]
    #[serde(default, skip)]
    non_exhaustive: (),
}

impl ValidatorKeys {
    /// Creates a new `ValidatorKeys` object.
    ///
    /// # Stability
    ///
    /// Since more keys may be added to `ValidatorKeys` in the future, this method is considered
    /// unstable.
    pub fn new(consensus_key: PublicKey, service_key: PublicKey) -> Self {
        Self {
            consensus_key,
            service_key,
            non_exhaustive: (),
        }
    }
}

impl ValidateInput for ValidatorKeys {
    type Error = failure::Error;

    fn validate(&self) -> Result<(), Self::Error> {
        if self.consensus_key == self.service_key {
            bail!("Consensus and service keys must be different.");
        }
        Ok(())
    }
}

/// Consensus algorithm parameters.
///
/// This configuration is initially created with default recommended values,
/// which can later be edited as required.
/// The parameters in this configuration should be the same for all nodes in the network and can
/// be changed using the
/// [configuration update service](https://exonum.com/doc/version/latest/advanced/configuration-updater/).
///
/// Default propose timeout value, along with the threshold, is chosen for maximal performance. In order
/// to slow down block generation,hence consume less disk space, these values can be increased.
///
/// For additional information on the Exonum consensus algorithm, refer to
/// [Consensus in Exonum](https://exonum.com/doc/version/latest/architecture/consensus/).
#[protobuf_convert(source = "schema::blockchain::Config")]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
pub struct ConsensusConfig {
    /// List of validators public keys.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub validator_keys: Vec<ValidatorKeys>,
    /// Interval between first two rounds. This interval defines the time that passes
    /// between the moment a new block is committed to the blockchain and the
    /// time when second round starts, regardless of whether a new block has
    /// been committed during this period or not.
    /// Each consecutive round will be longer then previous by constant factor determined
    /// by ConsensusConfig::TIMEOUT_LINEAR_INCREASE_PERCENT constant.
    ///
    /// Note that rounds in Exonum
    /// do not have a defined end time. Nodes in a new round can
    /// continue to vote for proposals and process messages related to previous
    /// rounds.
    pub first_round_timeout: Milliseconds,
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

    /// No-op field for forward compatibility.
    #[protobuf_convert(skip)]
    #[serde(default, skip)]
    non_exhaustive: (),
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self {
            validator_keys: Vec::default(),
            first_round_timeout: 3000,
            status_timeout: 5000,
            peers_timeout: 10_000,
            txs_block_limit: 1000,
            max_message_len: Self::DEFAULT_MAX_MESSAGE_LEN,
            min_propose_timeout: 10,
            max_propose_timeout: 200,
            propose_timeout_threshold: 500,
            non_exhaustive: (),
        }
    }
}

impl ConsensusConfig {
    /// Default value for `max_message_len`.
    pub const DEFAULT_MAX_MESSAGE_LEN: u32 = 1024 * 1024; // 1 MB
    /// Time that will be added to round timeout for each next round in terms of percent of `first_round_timeout`.
    pub const TIMEOUT_LINEAR_INCREASE_PERCENT: u64 = 10; // 10%

    /// Replaces validator keys in existing object with provided ones.
    pub fn with_validator_keys(mut self, validator_keys: Vec<ValidatorKeys>) -> Self {
        self.validator_keys = validator_keys;
        self
    }

    /// Generates a consensus configuration for testing and returns it together with the keys
    /// for the first validator.
    pub fn for_tests(validator_count: u16) -> (Self, Keys) {
        assert!(
            validator_count > 0,
            "Cannot create network without validators"
        );

        let mut node_keys = None;
        let validator_keys = (0..validator_count)
            .map(|i| {
                let keys = Keys::random();
                let consensus_pk = keys.consensus_pk();
                let service_pk = keys.service_pk();
                if i == 0 {
                    node_keys = Some(keys);
                }
                ValidatorKeys::new(consensus_pk, service_pk)
            })
            .collect();
        let config = Self {
            validator_keys,
            ..Default::default()
        };
        (config, node_keys.unwrap())
    }

    /// Check that validator keys is correct. Configuration should have at least
    /// a single validator key. And each key should meet only once.
    fn validate_keys(&self) -> Result<(), failure::Error> {
        ensure!(
            !self.validator_keys.is_empty(),
            "Consensus configuration must have at least one validator."
        );

        let mut exist_keys = HashSet::with_capacity(self.validator_keys.len() * 2);
        for validator_keys in &self.validator_keys {
            validator_keys.validate()?;
            if exist_keys.contains(&validator_keys.consensus_key)
                || exist_keys.contains(&validator_keys.service_key)
            {
                bail!("Duplicated keys are found: each consensus and service key must be unique");
            }

            exist_keys.insert(validator_keys.consensus_key);
            exist_keys.insert(validator_keys.service_key);
        }

        Ok(())
    }

    /// Search for identifier of the validator which satisfies the condition in predicate.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::{
    ///     blockchain::{ConsensusConfig, ValidatorKeys},
    ///     crypto,
    ///     helpers::ValidatorId,
    /// };
    ///
    /// fn main() {
    ///     let config = ConsensusConfig::default()
    ///         .with_validator_keys(
    ///             (0..4)
    ///                 .map(|_| ValidatorKeys::new(
    ///                     crypto::gen_keypair().0,
    ///                     crypto::gen_keypair().0,
    ///                 ))
    ///                 .collect(),
    ///         );
    ///
    ///     let some_validator_consensus_key = config.validator_keys[2].consensus_key;
    ///     // Try to find validator ID for this key.
    ///     assert_eq!(
    ///         config.find_validator(|validator_keys| {
    ///             validator_keys.consensus_key == some_validator_consensus_key
    ///         }),
    ///         Some(ValidatorId(2)),
    ///     );
    /// }
    /// ```
    pub fn find_validator(
        &self,
        predicate: impl Fn(&ValidatorKeys) -> bool,
    ) -> Option<ValidatorId> {
        self.validator_keys
            .iter()
            .position(predicate)
            .map(|id| ValidatorId(id as u16))
    }

    /// Produce warnings if configuration contains non-optimal values.
    ///
    /// Validation for logical correctness is performed in the `StoredConfiguration::try_deserialize`
    /// method, but some values can decrease consensus performance.
    fn warn_if_nonoptimal(&self) {
        const MIN_TXS_BLOCK_LIMIT: u32 = 100;
        const MAX_TXS_BLOCK_LIMIT: u32 = 10_000;

        if self.first_round_timeout <= 2 * self.max_propose_timeout {
            warn!(
                "It is recommended that first_round_timeout ({}) be at least twice as large \
                 as max_propose_timeout ({})",
                self.first_round_timeout, self.max_propose_timeout
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

/// Builder for `ConsensusConfig`.
///
/// Initially, `ConsensusConfig` in this builder is generated via `ConsensusConfig::default()`.
/// Only the necessary fields can be updated before obtaining the build config via `build` method.
///
/// # Examples
///
/// ```
/// # use exonum::blockchain::{ConsensusConfig, ConsensusConfigBuilder};
/// let consensus_config = ConsensusConfigBuilder::new()
///     .first_round_timeout(3010)
///     .min_propose_timeout(20)
///     .build();
///
/// assert_eq!(consensus_config.first_round_timeout, 3010);
/// assert_eq!(consensus_config.min_propose_timeout, 20);
/// assert_eq!(consensus_config.status_timeout, ConsensusConfig::default().status_timeout);
/// ```
#[derive(Debug, Default)]
pub struct ConsensusConfigBuilder {
    config: ConsensusConfig,
}

impl ConsensusConfigBuilder {
    /// Creates a new `ConsensusConfigBuilder` with `ConsensusConfig` initialized to its default value.
    pub fn new() -> Self {
        Self {
            config: ConsensusConfig::default(),
        }
    }

    /// Finishes the building process, returning the `ConsensusConfig` object.
    pub fn build(self) -> ConsensusConfig {
        self.config
    }

    /// Sets the `validator_keys` field of `ConsensusConfig`.
    pub fn validator_keys(self, validator_keys: Vec<ValidatorKeys>) -> Self {
        let config = ConsensusConfig {
            validator_keys,
            ..self.config
        };

        Self { config }
    }

    /// Sets the `first_round_timeout` field of `ConsensusConfig`.
    pub fn first_round_timeout(self, first_round_timeout: Milliseconds) -> Self {
        let config = ConsensusConfig {
            first_round_timeout,
            ..self.config
        };

        Self { config }
    }

    /// Sets the `status_timeout` field of `ConsensusConfig`.
    pub fn status_timeout(self, status_timeout: Milliseconds) -> Self {
        let config = ConsensusConfig {
            status_timeout,
            ..self.config
        };

        Self { config }
    }

    /// Sets the `peers_timeout` field of `ConsensusConfig`.
    pub fn peers_timeout(self, peers_timeout: Milliseconds) -> Self {
        let config = ConsensusConfig {
            peers_timeout,
            ..self.config
        };

        Self { config }
    }

    /// Sets the `txs_block_limit` field of `ConsensusConfig`.
    pub fn txs_block_limit(self, txs_block_limit: u32) -> Self {
        let config = ConsensusConfig {
            txs_block_limit,
            ..self.config
        };

        Self { config }
    }

    /// Sets the `min_propose_timeout` field of `ConsensusConfig`.
    pub fn min_propose_timeout(self, min_propose_timeout: Milliseconds) -> Self {
        let config = ConsensusConfig {
            min_propose_timeout,
            ..self.config
        };

        Self { config }
    }

    /// Sets the `max_propose_timeout` field of `ConsensusConfig`.
    pub fn max_propose_timeout(self, max_propose_timeout: Milliseconds) -> Self {
        let config = ConsensusConfig {
            max_propose_timeout,
            ..self.config
        };

        Self { config }
    }

    /// Sets the `max_message_len` field of `ConsensusConfig`.
    pub fn max_message_len(self, max_message_len: u32) -> Self {
        let config = ConsensusConfig {
            max_message_len,
            ..self.config
        };

        Self { config }
    }

    /// Sets the `propose_timeout_threshold` field of `ConsensusConfig`.
    pub fn propose_timeout_threshold(self, propose_timeout_threshold: u32) -> Self {
        let config = ConsensusConfig {
            propose_timeout_threshold,
            ..self.config
        };

        Self { config }
    }
}

impl ValidateInput for ConsensusConfig {
    type Error = failure::Error;

    fn validate(&self) -> Result<(), Self::Error> {
        const MINIMAL_BODY_SIZE: usize = 256;
        const MINIMAL_MESSAGE_LENGTH: u32 = (MINIMAL_BODY_SIZE + SIGNED_MESSAGE_MIN_SIZE) as u32;

        self.validate_keys()?;

        // Check timeouts.
        if self.min_propose_timeout > self.max_propose_timeout {
            bail!(
                "Invalid propose timeouts: min_propose_timeout should be less or equal then \
                 max_propose_timeout: min = {}, max = {}",
                self.min_propose_timeout,
                self.max_propose_timeout
            );
        }

        if self.first_round_timeout <= self.max_propose_timeout {
            bail!(
                "first_round_timeout({}) must be strictly larger than max_propose_timeout({})",
                self.first_round_timeout,
                self.max_propose_timeout
            );
        }

        // Check transactions limit.
        if self.txs_block_limit == 0 {
            bail!("txs_block_limit should not be equal to zero",);
        }

        // Check maximum message length for sanity.
        if self.max_message_len < MINIMAL_MESSAGE_LENGTH {
            bail!(
                "max_message_len ({}) must be at least {}",
                self.max_message_len,
                MINIMAL_MESSAGE_LENGTH
            );
        }

        // Print warning if configuration is not optimal
        self.warn_if_nonoptimal();

        Ok(())
    }
}

/// Genesis config parameters.
///
/// Information from this entity get saved to the genesis block.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "schema::lifecycle::GenesisConfig")]
pub struct GenesisConfig {
    /// Blockchain configuration used to create the genesis block.
    pub consensus_config: ConsensusConfig,

    /// Artifacts specification of the built-in services.
    pub artifacts: Vec<ArtifactSpec>,

    /// List of services with their configuration parameters that are created directly
    /// in the genesis block.
    pub builtin_instances: Vec<InstanceInitParams>,

    /// No-op field for forward compatibility.
    #[protobuf_convert(skip)]
    #[serde(default, skip)]
    non_exhaustive: (),
}

/// Data that is required for initialization of a service instance.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "schema::lifecycle::InstanceInitParams")]
pub struct InstanceInitParams {
    /// Instance specification.
    pub instance_spec: InstanceSpec,
    /// Constructor argument for the instance.
    pub constructor: Vec<u8>,

    /// No-op field for forward compatibility.
    #[protobuf_convert(skip)]
    #[serde(default, skip)]
    non_exhaustive: (),
}

impl InstanceInitParams {
    /// Generic constructor.
    pub fn new(
        id: InstanceId,
        name: impl Into<String>,
        artifact: ArtifactId,
        constructor: impl BinaryValue,
    ) -> Self {
        InstanceInitParams {
            instance_spec: InstanceSpec::from_raw_parts(id, name.into(), artifact),
            constructor: constructor.into_bytes(),
            non_exhaustive: (),
        }
    }

    /// Converts into `InstanceInitParams` with specific constructor.
    pub fn with_constructor(self, constructor: impl BinaryValue) -> InstanceInitParams {
        InstanceInitParams {
            instance_spec: self.instance_spec,
            constructor: constructor.into_bytes(),
            non_exhaustive: (),
        }
    }
}

impl From<InstanceSpec> for InstanceInitParams {
    fn from(instance_spec: InstanceSpec) -> InstanceInitParams {
        Self {
            instance_spec,
            constructor: Vec::new(),
            non_exhaustive: (),
        }
    }
}

/// Creates `GenesisConfig` from components.
#[derive(Debug)]
pub struct GenesisConfigBuilder {
    /// Consensus config.
    consensus_config: ConsensusConfig,
    /// Artifacts specifications for builtin services.
    artifacts: HashMap<ArtifactId, Vec<u8>>,
    /// Instances of builtin services.
    builtin_instances: Vec<InstanceInitParams>,
}

impl GenesisConfigBuilder {
    /// Creates a new builder instance based on the `ConsensusConfig`.
    pub fn with_consensus_config(consensus_config: ConsensusConfig) -> Self {
        Self {
            consensus_config,
            artifacts: HashMap::new(),
            builtin_instances: vec![],
        }
    }

    /// Adds an artifact with no deploy argument. Does nothing in case artifact with given id is
    /// already added.
    pub fn with_artifact(self, artifact: impl Into<ArtifactId>) -> Self {
        self.with_parametric_artifact(artifact, ())
    }

    /// Adds an artifact with corresponding deploy argument. Does nothing in case artifact with
    /// given id is already added.
    pub fn with_parametric_artifact(
        mut self,
        artifact: impl Into<ArtifactId>,
        payload: impl BinaryValue,
    ) -> Self {
        let artifact = artifact.into();
        self.artifacts
            .entry(artifact)
            .or_insert_with(|| payload.into_bytes());
        self
    }

    /// Adds service instance initialization parameters.
    pub fn with_instance(mut self, instance_params: InstanceInitParams) -> Self {
        self.builtin_instances.push(instance_params);
        self
    }

    /// Produces `GenesisConfig` from collected components.
    pub fn build(self) -> GenesisConfig {
        let artifacts = self
            .artifacts
            .into_iter()
            .map(|(artifact, payload)| ArtifactSpec::new(artifact, payload))
            .collect::<Vec<_>>();
        GenesisConfig {
            consensus_config: self.consensus_config,
            artifacts,
            builtin_instances: self.builtin_instances,
            non_exhaustive: (),
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use std::fmt::Display;

    use super::*;
    use crate::crypto::{self, gen_keypair_from_seed, Seed, SEED_LENGTH};

    fn assert_err_contains(actual: impl Display, expected: impl AsRef<str>) {
        let actual = actual.to_string();
        let expected = expected.as_ref();
        assert!(
            actual.contains(expected),
            "Actual is {}, expected: {}",
            actual,
            expected
        );
    }

    fn gen_validator_keys(i: u8) -> ValidatorKeys {
        ValidatorKeys::new(
            gen_keypair_from_seed(&Seed::new([i; SEED_LENGTH])).0,
            gen_keypair_from_seed(&Seed::new([u8::max_value() - i; SEED_LENGTH])).0,
        )
    }

    fn gen_keys_pool(count: usize) -> Vec<PublicKey> {
        (0..count)
            .map(|_| crypto::gen_keypair().0)
            .collect::<Vec<_>>()
    }

    fn gen_consensus_config() -> ConsensusConfig {
        ConsensusConfig {
            validator_keys: (0..4).map(gen_validator_keys).collect(),
            ..ConsensusConfig::default()
        }
    }

    #[test]
    fn validate_validator_keys_err_same() {
        let pk = crypto::gen_keypair().0;

        let keys = ValidatorKeys::new(pk, pk);
        let e = keys.validate().unwrap_err();
        assert_err_contains(e, "Consensus and service keys must be different");
    }

    #[test]
    fn consensus_config_validate_ok() {
        let cfg = ConsensusConfig {
            validator_keys: (0..4).map(gen_validator_keys).collect(),
            ..ConsensusConfig::default()
        };

        cfg.validate().expect("Expected valid consensus config");
    }

    #[test]
    fn consensus_config_validate_err_round_trip() {
        let keys = gen_keys_pool(4);

        let cases = [
            (
                ConsensusConfig::default(),
                "Consensus configuration must have at least one validator",
            ),
            (
                ConsensusConfig {
                    validator_keys: vec![ValidatorKeys::new(keys[0], keys[0])],
                    ..ConsensusConfig::default()
                },
                "Consensus and service keys must be different",
            ),
            (
                ConsensusConfig {
                    validator_keys: vec![
                        ValidatorKeys::new(keys[0], keys[1]),
                        ValidatorKeys::new(keys[0], keys[2]),
                    ],
                    ..ConsensusConfig::default()
                },
                "Duplicated keys are found",
            ),
            (
                ConsensusConfig {
                    validator_keys: vec![
                        ValidatorKeys::new(keys[0], keys[1]),
                        ValidatorKeys::new(keys[2], keys[1]),
                    ],
                    ..ConsensusConfig::default()
                },
                "Duplicated keys are found",
            ),
            (
                ConsensusConfig {
                    min_propose_timeout: 10,
                    max_propose_timeout: 5,
                    ..gen_consensus_config()
                },
                "min_propose_timeout should be less or",
            ),
            (
                ConsensusConfig {
                    first_round_timeout: 10,
                    max_propose_timeout: 15,
                    ..gen_consensus_config()
                },
                "first_round_timeout(10) must be strictly larger than max_propose_timeout(15)",
            ),
            (
                ConsensusConfig {
                    txs_block_limit: 0,
                    ..gen_consensus_config()
                },
                "txs_block_limit should not be equal to zero",
            ),
            (
                ConsensusConfig {
                    max_message_len: 0,
                    ..gen_consensus_config()
                },
                "max_message_len (0) must be at least",
            ),
        ];

        for (cfg, expected_msg) in &cases {
            assert_err_contains(cfg.validate().unwrap_err(), expected_msg);
        }
    }

    #[test]
    fn genesis_config_creation() {
        let consensus = gen_consensus_config();
        let version = "1.0.0".parse().unwrap();
        let artifact1 = ArtifactId::from_raw_parts(42, "test_artifact1".into(), version);
        let version = "0.2.8".parse().unwrap();
        let artifact2 = ArtifactId::from_raw_parts(42, "test_artifact2".into(), version);

        let genesis_config = GenesisConfigBuilder::with_consensus_config(consensus.clone())
            .with_artifact(artifact1.clone())
            .with_parametric_artifact(artifact2.clone(), vec![1_u8, 2, 3])
            .with_instance(artifact1.clone().into_default_instance(1, "art1_inst1"))
            .with_instance(
                artifact1
                    .into_default_instance(2, "art1_inst2")
                    .with_constructor(vec![4_u8, 5, 6]),
            )
            .with_instance(artifact2.into_default_instance(1, "art2_inst1"))
            .build();

        assert_eq!(genesis_config.consensus_config, consensus);
        assert_eq!(genesis_config.artifacts.len(), 2);
        assert_eq!(genesis_config.builtin_instances.len(), 3);
    }

    #[test]
    fn genesis_config_check_artifacts_duplication() {
        let consensus = gen_consensus_config();
        let version = "1.1.5-rc.3".parse().unwrap();
        let artifact = ArtifactId::new(42_u32, "test_artifact", version).unwrap();
        let correct_payload = vec![1_u8, 2, 3];

        let genesis_config = GenesisConfigBuilder::with_consensus_config(consensus)
            .with_parametric_artifact(artifact.clone(), correct_payload.clone())
            .with_parametric_artifact(artifact, vec![4_u8, 5, 6])
            .build();

        assert_eq!(genesis_config.artifacts.len(), 1);
        assert_eq!(genesis_config.artifacts[0].payload, correct_payload);
    }
}
