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

//! Standard Exonum CLI command used to combine a secret and all the public parts of the
//! node configuration in a single file.

use exonum::{
    blockchain::ConsensusConfig,
    node::{ConnectInfo, ConnectListConfig, NodeApiConfig},
};
use failure::{bail, format_err, Error};
use serde_derive::{Deserialize, Serialize};
use structopt::StructOpt;

use std::{collections::BTreeMap, net::SocketAddr, path::PathBuf};

use crate::{
    command::{ExonumCommand, StandardResult},
    config::{NodeConfig, NodePrivateConfig, NodePublicConfig},
    io::{load_config_file, save_config_file},
};

/// Generate final node configuration using public configs
/// of other nodes in the network.
#[derive(StructOpt, Debug, Serialize, Deserialize)]
pub struct Finalize {
    /// Path to a secret part of a node configuration.
    pub secret_config_path: PathBuf,
    /// Path to a node configuration file which will be created after
    /// running this command.
    pub output_config_path: PathBuf,
    /// List of paths to public parts of configuration of all the nodes
    /// in the network.
    #[structopt(long, short = "p")]
    pub public_configs: Vec<PathBuf>,
    /// Listen address for node public API.
    ///
    /// Public API is used mainly for sending API requests to user services.
    #[structopt(long)]
    pub public_api_address: Option<SocketAddr>,
    /// Listen address for node private API.
    ///
    /// Private API is used by node administrators for node monitoring and control.
    #[structopt(long)]
    pub private_api_address: Option<SocketAddr>,
    /// Cross-origin resource sharing options for responses returned by public API handlers.
    #[structopt(long)]
    pub public_allow_origin: Option<String>,
    /// Cross-origin resource sharing options for responses returned by private API handlers.
    #[structopt(long)]
    pub private_allow_origin: Option<String>,
}

struct ValidatedConfigs {
    common: NodePublicConfig,
    public_configs: Vec<NodePublicConfig>,
}

impl Finalize {
    fn validate_configs(public_configs: Vec<NodePublicConfig>) -> Result<ValidatedConfigs, Error> {
        let mut map = BTreeMap::new();
        let mut config_iter = public_configs.into_iter();
        let first = config_iter
            .next()
            .ok_or_else(|| format_err!("Expected at least one config in PUBLIC_CONFIGS"))?;
        map.insert(first.validator_keys.unwrap().consensus_key, first.clone());

        for config in config_iter {
            if first.consensus != config.consensus || first.general != config.general {
                bail!(
                    "Found config with different common part. {:?} != {:?}",
                    first,
                    config
                );
            };
            if map
                .insert(config.validator_keys.unwrap().consensus_key, config.clone())
                .is_some()
            {
                bail!("Found duplicate consensus keys in PUBLIC_CONFIGS");
            }
        }
        Ok(ValidatedConfigs {
            common: first,
            public_configs: map.values().cloned().collect(),
        })
    }

    fn create_connect_list_config(
        public_configs: &[NodePublicConfig],
        secret_config: &NodePrivateConfig,
    ) -> ConnectListConfig {
        let peers = public_configs
            .iter()
            .filter(|config| {
                config.validator_keys.unwrap().consensus_key != secret_config.keys.consensus_pk()
            })
            .map(|config| ConnectInfo {
                public_key: config.validator_keys.unwrap().consensus_key,
                address: secret_config.external_address.clone(),
            })
            .collect();

        ConnectListConfig { peers }
    }
}

impl ExonumCommand for Finalize {
    fn execute(self) -> Result<StandardResult, Error> {
        let secret_config: NodePrivateConfig = load_config_file(&self.secret_config_path)?;
        let public_configs: Vec<NodePublicConfig> = self
            .public_configs
            .into_iter()
            .map(load_config_file)
            .collect::<Result<_, _>>()?;

        let public_allow_origin = self.public_allow_origin.map(|s| s.parse().unwrap());
        let private_allow_origin = self.private_allow_origin.map(|s| s.parse().unwrap());

        let ValidatedConfigs {
            common,
            public_configs,
        } = Self::validate_configs(public_configs)?;

        let validators_count = common.general.validators_count as usize;

        if validators_count != public_configs.len() {
            bail!("The number of validators does not match the number of validators keys.");
        }

        let consensus = ConsensusConfig {
            validator_keys: public_configs
                .iter()
                .flat_map(|c| c.validator_keys.clone())
                .collect(),
            ..common.consensus
        };

        let connect_list = Self::create_connect_list_config(&public_configs, &secret_config);

        let config = NodeConfig {
            private_config: NodePrivateConfig {
                listen_address: secret_config.listen_address,
                external_address: secret_config.external_address,
                master_key_path: secret_config.master_key_path,
                api: NodeApiConfig {
                    public_api_address: self.public_api_address,
                    private_api_address: self.private_api_address,
                    public_allow_origin,
                    private_allow_origin,
                    ..secret_config.api
                },
                network: secret_config.network,
                mempool: secret_config.mempool,
                database: secret_config.database,
                thread_pool_size: secret_config.thread_pool_size,
                connect_list,
                keys: secret_config.keys,
            },
            public_config: NodePublicConfig {
                consensus,
                general: common.general,
                validator_keys: None,
            },
        };

        save_config_file(&config, &self.output_config_path)?;

        Ok(StandardResult::Finalize {
            node_config_path: self.output_config_path,
        })
    }
}
