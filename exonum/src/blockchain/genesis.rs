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

use std::collections::BTreeMap;

use super::config::{ConsensusConfig, ValidatorKeys};

/// The initial configuration which is committed into the genesis block.
///
/// The genesis block is the first block in the blockchain which is created
/// when the blockchain is initially launched. This block can contain some service
/// data, but does not include transactions.
///
/// `GenesisConfig` includes consensus related configuration, the public keys of validators,
/// and initial service state.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct GenesisConfig {
    /// Consensus configuration.
    pub consensus: ConsensusConfig,
    /// List of public keys of validators.
    pub validator_keys: Vec<ValidatorKeys>,
    /// Initial state of services.
    #[serde(default)]
    pub services: BTreeMap<String, ServiceState>,
}

/// Initial service state.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ServiceState {
    /// Service is enabled.
    pub enabled: bool,
}

/// A builder for [`GenesisConfig`].
///
/// [`GenesisConfig`]: struct.GenesisConfig.html
#[derive(Debug)]
pub struct GenesisConfigBuilder {
    config: GenesisConfig,
}

impl GenesisConfigBuilder {
    /// Initializes a new builder with an empty configuration.
    pub fn new() -> Self {
        GenesisConfigBuilder {
            config: GenesisConfig {
                consensus: ConsensusConfig::default(),
                validator_keys: Vec::new(),
                services: BTreeMap::new(),
            },
        }
    }

    /// Sets validator keys.
    pub fn validators(mut self, validator_keys: impl Iterator<Item = ValidatorKeys>) -> Self {
        self.config.validator_keys = validator_keys.collect();
        self
    }

    /// Sets consensus configuration.
    pub fn consensus(mut self, consensus: ConsensusConfig) -> Self {
        self.config.consensus = consensus;
        self
    }

    /// Sets service state configuration.
    pub fn service_state(mut self, services: BTreeMap<String, ServiceState>) -> Self {
        self.config.services = services;
        self
    }

    /// Returns a complete genesis configuration.
    pub fn finish(self) -> GenesisConfig {
        self.config.consensus.warn_if_nonoptimal();
        self.config
    }
}
