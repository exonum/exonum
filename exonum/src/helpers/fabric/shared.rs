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

//! This module is used to collect structures that is shared into `CommandExtension` from `Command`.

use toml::Value;
use std::collections::BTreeMap;
use std::net::SocketAddr;

use crypto::{PublicKey, SecretKey};
use blockchain::config::ConsensusConfig;
use blockchain::config::ValidatorKeys;

/// Abstract configuration.
pub type AbstractConfig = BTreeMap<String, Value>;

/// Node public configurations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodePublicConfig {
    /// Socket address.
    pub addr: SocketAddr,
    /// Public keys of a validator.
    pub validator_keys: ValidatorKeys,
    /// Services configurations.
    pub services_public_configs: AbstractConfig,
}

/// `SharedConfig` contain all public information that should be shared in the handshake process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedConfig {
    pub common: CommonConfigTemplate,
    pub node: NodePublicConfig,
}

impl NodePublicConfig {
    /// Returns address.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Returns services configurations.
    pub fn services_public_configs(&self) -> &AbstractConfig {
        &self.services_public_configs
    }
}

/// Basepoint config.
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, Default)]
pub struct CommonConfigTemplate {
    /// Consensus configuration.
    pub consensus_config: ConsensusConfig,
    /// Services configuration.
    pub services_config: AbstractConfig,
}

/// `NodePrivConfig` collect all public and secret keys.
#[derive(Debug, Serialize, Deserialize)]
pub struct NodePrivateConfig {
    /// Listen address.
    pub listen_addr: SocketAddr,
    /// Consensus public key.
    pub consensus_public_key: PublicKey,
    /// Consensus secret key.
    pub consensus_secret_key: SecretKey,
    /// Service public key.
    pub service_public_key: PublicKey,
    /// Service secret key.
    pub service_secret_key: SecretKey,
    /// Additional service secret config.
    pub services_secret_configs: AbstractConfig,
}
