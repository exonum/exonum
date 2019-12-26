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

//! Contains various config structures used during configuration process.

use exonum::{
    blockchain::{ConsensusConfig, ValidatorKeys},
    events::NetworkConfiguration,
    exonum_merkledb::DbOptions,
    keys::{read_keys_from_file, Keys},
    node::{ConnectListConfig, MemoryPoolConfig, NodeApiConfig, NodeConfig as CoreNodeConfig},
};
use exonum_supervisor::mode::Mode as SupervisorMode;
use serde_derive::{Deserialize, Serialize};

use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
};

/// Part of the template configuration.
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub struct GeneralConfig {
    /// Count of the validator nodes in the network.
    pub validators_count: u32,
    /// Supervisor service mode.
    pub supervisor_mode: SupervisorMode,
}

/// Public configuration of the node. Is shared among validators.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodePublicConfig {
    /// Consensus configuration.
    pub consensus: ConsensusConfig,
    /// General configuration.
    pub general: GeneralConfig,
    /// Public keys of the node.
    ///
    /// `None` when not yet generated. The keys are generated at
    /// `generate-config` configuration step. The keys are required for the
    /// `finalize` step.
    pub validator_keys: Option<ValidatorKeys>,
}

/// Private configuration of the node.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodePrivateConfig {
    /// Network listening address.
    pub listen_address: SocketAddr,
    /// The address advertised by the node for peers to connect to.
    pub external_address: String,
    /// Path to the master key file.
    pub master_key_path: PathBuf,
    /// API configuration.
    pub api: NodeApiConfig,
    /// Network configuration.
    pub network: NetworkConfiguration,
    /// Memory pool configuration.
    pub mempool: MemoryPoolConfig,
    /// Optional database configuration.
    #[serde(default)]
    pub database: DbOptions,
    /// Amount of threads used for transactions verification.
    pub thread_pool_size: Option<u8>,
    /// Information about peers within network.
    pub connect_list: ConnectListConfig,
    /// Validator keys.
    #[serde(skip)]
    pub keys: Keys,
}

/// Configuration for the `Node`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct NodeConfig {
    /// Private configuration of the node.
    pub private_config: NodePrivateConfig,
    /// Public configuration of the node.
    pub public_config: NodePublicConfig,
}

impl Into<CoreNodeConfig> for NodeConfig {
    fn into(self) -> CoreNodeConfig {
        CoreNodeConfig {
            consensus: self.public_config.consensus,
            listen_address: self.private_config.listen_address,
            external_address: self.private_config.external_address,
            network: self.private_config.network,
            api: self.private_config.api,
            mempool: self.private_config.mempool,
            services_configs: Default::default(),
            database: self.private_config.database,
            connect_list: self.private_config.connect_list,
            thread_pool_size: self.private_config.thread_pool_size,
            master_key_path: self.private_config.master_key_path,
            keys: self.private_config.keys,
        }
    }
}

impl NodeConfig {
    /// Read validator keys from the encrypted file.
    pub fn read_secret_keys(
        &mut self,
        config_file_path: impl AsRef<Path>,
        master_key_passphrase: &[u8],
    ) {
        let config_folder = config_file_path.as_ref().parent().unwrap();
        let master_key_path = self.private_config.master_key_path.clone();
        let master_key_path = if master_key_path.is_absolute() {
            master_key_path.clone()
        } else {
            config_folder.join(&master_key_path)
        };

        let keys = read_keys_from_file(&master_key_path, master_key_passphrase)
            .expect("Could not read master_key_path from file");

        self.private_config = NodePrivateConfig {
            keys,
            ..self.private_config.clone()
        };
    }
}
