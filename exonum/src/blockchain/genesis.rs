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

use super::config::{ConsensusConfig, ValidatorKeys};

/// The initial configuration which is committed into the genesis block.
///
/// The genesis block is the first block in the blockchain which is created
/// when the blockchain is initially launched. This block can contain some service
/// data, but does not include transactions.
///
/// `GenesisConfig` includes consensus related configuration and the public keys of validators.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct GenesisConfig {
    /// Consensus configuration.
    pub consensus: ConsensusConfig,
    /// List of public keys of validators.
    pub validator_keys: Vec<ValidatorKeys>,
}

impl GenesisConfig {
    /// Creates a default configuration from the given list of public keys.
    pub fn new<I: Iterator<Item = ValidatorKeys>>(validators: I) -> Self {
        Self::new_with_consensus(ConsensusConfig::default(), validators)
    }

    /// Creates a configuration from the given consensus configuration and list of public keys.
    pub fn new_with_consensus<I>(consensus: ConsensusConfig, validator_keys: I) -> Self
    where
        I: Iterator<Item = ValidatorKeys>,
    {
        consensus.warn_if_nonoptimal();
        Self {
            consensus,
            validator_keys: validator_keys.collect(),
        }
    }
}
