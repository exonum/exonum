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

//! Standard Exonum CLI command used to combine a private and all the public parts of the
//! node configuration in a single file.

use exonum::{
    blockchain::ConsensusConfig,
    crypto::PublicKey,
    node::{ConnectInfo, ConnectListConfig, NodeApiConfig},
};
use failure::{bail, ensure, format_err, Error};
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
    /// Path to a private part of a node configuration.
    pub private_config_path: PathBuf,
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
    fn validate_configs(configs: Vec<NodePublicConfig>) -> Result<ValidatedConfigs, Error> {
        let mut config_iter = configs.into_iter();
        let mut public_configs = BTreeMap::new();
        let first = config_iter
            .next()
            .ok_or_else(|| format_err!("Expected at least one config in <public-configs>"))?;
        let consensus_key = Self::get_consensus_key(&first)?;
        public_configs.insert(consensus_key, first.clone());

        for config in config_iter {
            ensure!(
                first.consensus == config.consensus,
                "Found public configs with different consensus configuration.\
                 Make sure the same template config was used for generation.\
                 {:#?} \nnot equal to\n {:#?}",
                first.consensus,
                config.consensus
            );
            ensure!(
                first.general == config.general,
                "Found public configs with different general configuration.\
                 Make sure the same template config was used for generation.\
                 {:#?} \nnot equal to\n {:#?}",
                first.general,
                config.general
            );

            let consensus_key = Self::get_consensus_key(&config)?;
            if public_configs.insert(consensus_key, config).is_some() {
                bail!(
                    "Found duplicated consensus keys in <public-configs>: {:?}",
                    consensus_key
                );
            }
        }
        Ok(ValidatedConfigs {
            common: first,
            public_configs: public_configs.values().cloned().collect(),
        })
    }

    fn get_consensus_key(config: &NodePublicConfig) -> Result<PublicKey, failure::Error> {
        Ok(config
            .validator_keys
            .ok_or_else(|| format_err!("Expected validator keys in public config: {:#?}", config))?
            .consensus_key)
    }

    fn create_connect_list_config(
        public_configs: &[NodePublicConfig],
        private_config: &NodePrivateConfig,
    ) -> ConnectListConfig {
        let peers = public_configs
            .iter()
            .filter(|config| {
                Self::get_consensus_key(config).unwrap() != private_config.keys.consensus_pk()
            })
            .map(|config| ConnectInfo {
                public_key: Self::get_consensus_key(config).unwrap(),
                address: private_config.external_address.clone(),
            })
            .collect();

        ConnectListConfig { peers }
    }
}

impl ExonumCommand for Finalize {
    fn execute(self) -> Result<StandardResult, Error> {
        let private_config: NodePrivateConfig = load_config_file(&self.private_config_path)?;
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

        ensure!(
            validators_count == public_configs.len(),
            "The number of validators ({}) does not match the number of validators keys ({}).",
            validators_count,
            public_configs.len()
        );

        let consensus = ConsensusConfig {
            validator_keys: public_configs
                .iter()
                .flat_map(|c| c.validator_keys)
                .collect(),
            ..common.consensus
        };

        let connect_list = Self::create_connect_list_config(&public_configs, &private_config);

        let private_config = NodePrivateConfig {
            listen_address: private_config.listen_address,
            external_address: private_config.external_address,
            master_key_path: private_config.master_key_path,
            api: NodeApiConfig {
                public_api_address: self.public_api_address,
                private_api_address: self.private_api_address,
                public_allow_origin,
                private_allow_origin,
                ..private_config.api
            },
            network: private_config.network,
            mempool: private_config.mempool,
            database: private_config.database,
            thread_pool_size: private_config.thread_pool_size,
            connect_list,
            keys: private_config.keys,
        };
        let public_config = NodePublicConfig {
            consensus,
            general: common.general,
            validator_keys: None,
        };

        let config = NodeConfig {
            private_config,
            public_config,
        };

        save_config_file(&config, &self.output_config_path)?;

        Ok(StandardResult::Finalize {
            node_config_path: self.output_config_path,
        })
    }
}
