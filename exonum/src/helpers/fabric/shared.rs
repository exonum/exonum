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

//! This module is used to collect structures that is shared into `CommandExtension` from `Command`.

use std::{collections::BTreeMap, net::SocketAddr, path::PathBuf};

use crate::blockchain::config::{ConsensusConfig, ValidatorKeys};
use crate::crypto::PublicKey;

/// Abstract configuration.
pub type AbstractConfig = BTreeMap<String, toml::Value>;

/// Node public configurations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodePublicConfig {
    /// Network address.
    pub address: String,
    /// Public keys of a validator.
    pub validator_keys: ValidatorKeys,
    /// Services configurations.
    #[serde(default)]
    pub services_public_configs: AbstractConfig,
}

/// `SharedConfig` contain all public information that should be shared in the handshake process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedConfig {
    /// Template for common configuration
    pub common: CommonConfigTemplate,
    /// Public node
    pub node: NodePublicConfig,
}

impl NodePublicConfig {
    /// Returns address.
    pub fn address(&self) -> &str {
        &self.address
    }

    /// Returns services configurations.
    pub fn services_public_configs(&self) -> &AbstractConfig {
        &self.services_public_configs
    }
}

/// Base config.
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, Default)]
pub struct CommonConfigTemplate {
    /// Consensus configuration.
    pub consensus_config: ConsensusConfig,
    /// Service state configuration.
    #[serde(default)]
    pub services: AbstractConfig,
    /// Services configuration.
    #[serde(default)]
    pub services_config: AbstractConfig,
    /// General configuration.
    pub general_config: AbstractConfig,
}

/// `NodePrivateConfig` collects all public and secret keys.
#[derive(Debug, Serialize, Deserialize)]
pub struct NodePrivateConfig {
    /// Listen address.
    pub listen_address: SocketAddr,
    /// External address.
    pub external_address: String,
    /// Consensus public key.
    pub consensus_public_key: PublicKey,
    /// Path to the consensus secret key file.
    pub consensus_secret_key: PathBuf,
    /// Service public key.
    pub service_public_key: PublicKey,
    /// Path to the service secret key file.
    pub service_secret_key: PathBuf,
    /// Additional service secret config.
    #[serde(default)]
    pub services_secret_configs: AbstractConfig,
}

/// Used for passing configuration for starting node from the command line that is not in the `NodeConfig`.
#[derive(Debug, Serialize, Deserialize)]
pub struct NodeRunConfig {
    pub consensus_pass_method: String,
    pub service_pass_method: String,
}
