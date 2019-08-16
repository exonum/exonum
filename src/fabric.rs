// Copyright 2019 The Exonum Team
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

use exonum::blockchain::{ConsensusConfig, ValidatorKeys};
use exonum::crypto::PublicKey;
use serde::{Deserialize, Serialize};

use std::net::SocketAddr;
use std::path::PathBuf;

/// Base config.
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, Default)]
pub struct CommonConfigTemplate {
    /// Consensus configuration.
    pub consensus_config: ConsensusConfig,
    /// General configuration.
    pub general_config: GeneralConfig,
}

/// TODO: General config
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, Default)]
pub struct GeneralConfig {
    /// TODO: validators count
    pub validators_count: u32,
}

/// Node public configurations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodePublicConfig {
    /// Network address.
    pub address: String,
    /// Public keys of a validator.
    pub validator_keys: ValidatorKeys,
}

/// `SharedConfig` contain all public information that should be shared in the handshake process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedConfig {
    /// Template for common configuration
    pub common: CommonConfigTemplate,
    /// Public node
    pub node: NodePublicConfig,
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
}
