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

#![deny(missing_docs)]

//! Helper crate for secure and convenient configuration of the Exonum nodes.

use exonum::blockchain::{GenesisConfig, InstanceCollection, ValidatorKeys};
use exonum::crypto::generate_keys_file;
use exonum::exonum_merkledb::RocksDB;
use exonum::node::{ConnectInfo, ConnectListConfig, Node, NodeApiConfig, NodeConfig};
use failure::Error;
use serde::{Deserialize, Serialize};
use structopt::StructOpt;

use std::collections::BTreeMap;
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::config::{load_config_file, save_config_file};
use crate::fabric::{
    CommonConfigTemplate, GeneralConfig, NodePrivateConfig, NodePublicConfig, SharedConfig,
};
use crate::password::{PassInputMethod, SecretKeyType, ZeroizeOnDrop};

pub mod config;
pub mod fabric;
pub mod password;

const CONSENSUS_SECRET_KEY_NAME: &str = "consensus.key.toml";
const SERVICE_SECRET_KEY_NAME: &str = "service.key.toml";
const PUB_CONFIG_FILE_NAME: &str = "pub.toml";
const SEC_CONFIG_FILE_NAME: &str = "sec.toml";

/// Reads user input from the stdin and runs the node with a provided parameters.
///
/// Enables Rust runtime only.
pub fn run_node(
    services: impl IntoIterator<Item = InstanceCollection>,
) -> Result<(), failure::Error> {
    let command = Command::from_args();
    if let StandardResult::Run(run_config) = command.execute()? {
        let database = Arc::new(RocksDB::open(
            run_config.db_path,
            &run_config.node_config.database,
        )?) as Arc<_>;
        let node = Node::new(database, services, run_config.node_config, None);
        node.run()
    } else {
        Ok(())
    }
}

/// Output of any of the standard Exonum Core configuration commands.
pub enum StandardResult {
    /// `generate-template` command output.
    GenerateTemplate {
        /// Path to a generated common template file.
        template_config_path: PathBuf,
    },
    /// `generate-config` command output.
    GenerateConfig {
        /// Path to a generated public config of the node.
        public_config_path: PathBuf,
        /// Path to a generated private config of the node.
        secret_config_path: PathBuf,
    },
    /// `finalize` command output.
    Finalize {
        /// Path to a generated final node config.
        node_config_path: PathBuf,
    },
    /// `run` command output.
    Run(NodeRunConfig),
}

/// Container for node configuration parameters produced by `Run` command.
pub struct NodeRunConfig {
    /// Final node configuration parameters.
    pub node_config: NodeConfig,
    /// Path to a directory containing database files, provided by user.
    pub db_path: PathBuf,
}

/// Interface of standard Exonum Core configuration commands.
pub trait ExonumCommand {
    /// Returns the result of the command execution.
    fn execute(self) -> Result<StandardResult, Error>;
}

/// Standard Exonum Core configuration commands.
#[derive(StructOpt, Debug, Serialize, Deserialize)]
pub enum Command {
    #[structopt(name = "generate-template")]
    /// Generate common part of the nodes configuration.
    GenerateTemplate(GenerateTemplate),
    #[structopt(name = "generate-config")]
    /// Generate public and private configs of the node.
    GenerateConfig(GenerateConfig),
    #[structopt(name = "finalize")]
    /// Generate final node configuration using public configs
    /// of other nodes in the network.
    Finalize(Finalize),
    #[structopt(name = "run")]
    /// Run the node with provided node config.
    Run(Run),
}

impl Command {
    /// Wrapper around `StructOpt::from_args` method.
    pub fn from_args() -> Self {
        <Self as StructOpt>::from_args()
    }
}

impl ExonumCommand for Command {
    fn execute(self) -> Result<StandardResult, Error> {
        match self {
            Command::GenerateTemplate(command) => command.execute(),
            Command::GenerateConfig(command) => command.execute(),
            Command::Finalize(command) => command.execute(),
            Command::Run(command) => command.execute(),
        }
    }
}

/// Generate common part of the nodes configuration.
#[derive(StructOpt, Debug, Serialize, Deserialize)]
#[structopt(rename_all = "kebab-case")]
pub struct GenerateTemplate {
    /// Path to a node configuration template file.
    pub common_config: PathBuf,
    #[structopt(long)]
    /// Number of validators in the network.
    pub validators_count: u32,
}

impl ExonumCommand for GenerateTemplate {
    fn execute(self) -> Result<StandardResult, Error> {
        let config_template = CommonConfigTemplate {
            consensus_config: Default::default(),
            general_config: GeneralConfig {
                validators_count: self.validators_count,
            },
        };
        save_config_file(&config_template, &self.common_config)?;
        Ok(StandardResult::GenerateTemplate {
            template_config_path: self.common_config,
        })
    }
}

/// Generate public and private configs of the node.
#[derive(StructOpt, Debug, Serialize, Deserialize)]
#[structopt(rename_all = "kebab-case")]
pub struct GenerateConfig {
    /// Path to node configuration template file.
    pub common_config: PathBuf,
    /// Path to a directory where public and private node configuration files
    /// will be saved.
    pub output_dir: PathBuf,
    #[structopt(long, short = "a")]
    /// External IP address of the node used for communications between nodes.
    pub peer_address: SocketAddr,
    #[structopt(long, short = "l")]
    /// Listen IP address of the node used for communications between nodes.
    pub listen_address: Option<SocketAddr>,
    #[structopt(long, short = "n")]
    /// Don't prompt for passwords when generating private keys.
    pub no_password: bool,
    #[structopt(long)]
    /// Passphrase entry method for consensus key.
    ///
    /// Possible values are: `stdin`, `env{:ENV_VAR_NAME}`, `pass:PASSWORD`.
    /// Default Value is `stdin`.
    /// If `ENV_VAR_NAME` is not specified `$EXONUM_CONSENSUS_PASS` is used
    /// by default.
    pub consensus_key_pass: Option<PassInputMethod>,
    #[structopt(long)]
    /// Passphrase entry method for service key.
    ///
    /// Possible values are: `stdin`, `env{:ENV_VAR_NAME}`, `pass:PASSWORD`.
    /// Default Value is `stdin`.
    /// If `ENV_VAR_NAME` is not specified `$EXONUM_CONSENSUS_PASS` is used
    /// by default.
    pub service_key_pass: Option<PassInputMethod>,
}

impl GenerateConfig {
    fn get_passphrase(
        no_password: bool,
        method: PassInputMethod,
        secret_key_type: SecretKeyType,
    ) -> ZeroizeOnDrop<String> {
        if no_password {
            ZeroizeOnDrop::default()
        } else {
            method.get_passphrase(secret_key_type, false)
        }
    }
}

impl ExonumCommand for GenerateConfig {
    fn execute(self) -> Result<StandardResult, Error> {
        let common_config: CommonConfigTemplate = load_config_file(self.common_config.clone())?;

        let pub_config_path = self.output_dir.join(PUB_CONFIG_FILE_NAME);
        let private_config_path = self.output_dir.join(SEC_CONFIG_FILE_NAME);
        let consensus_secret_key_path = self.output_dir.join(CONSENSUS_SECRET_KEY_NAME);
        let service_secret_key_path = self.output_dir.join(SERVICE_SECRET_KEY_NAME);

        let listen_address: SocketAddr = self.listen_address.unwrap_or_else(|| {
            SocketAddr::new("0.0.0.0".parse().unwrap(), self.peer_address.port())
        });

        let consensus_public_key = {
            let passphrase = Self::get_passphrase(
                self.no_password,
                self.consensus_key_pass.unwrap_or_default(),
                SecretKeyType::Consensus,
            );
            create_secret_key_file(&consensus_secret_key_path, passphrase.as_bytes())
        };
        let service_public_key = {
            let passphrase = Self::get_passphrase(
                self.no_password,
                self.service_key_pass.unwrap_or_default(),
                SecretKeyType::Service,
            );
            create_secret_key_file(&service_secret_key_path, passphrase.as_bytes())
        };

        let validator_keys = ValidatorKeys {
            consensus_key: consensus_public_key,
            service_key: service_public_key,
        };
        let node_pub_config = NodePublicConfig {
            address: self.peer_address.to_string(),
            validator_keys,
        };
        let shared_config = SharedConfig {
            node: node_pub_config,
            common: common_config,
        };
        // Save public config separately.
        save_config_file(&shared_config, &pub_config_path)?;

        let private_config = NodePrivateConfig {
            listen_address,
            external_address: self.peer_address.to_string(),
            consensus_public_key,
            consensus_secret_key: CONSENSUS_SECRET_KEY_NAME.into(),
            service_public_key,
            service_secret_key: SERVICE_SECRET_KEY_NAME.into(),
        };

        save_config_file(&private_config, &private_config_path)?;

        Ok(StandardResult::GenerateConfig {
            public_config_path: pub_config_path,
            secret_config_path: private_config_path,
        })
    }
}

fn create_secret_key_file(
    secret_key_path: impl AsRef<Path>,
    passphrase: impl AsRef<[u8]>,
) -> exonum::crypto::PublicKey {
    let secret_key_path = secret_key_path.as_ref();
    if secret_key_path.exists() {
        panic!(
            "Failed to create secret key file. File exists: {}",
            secret_key_path.to_string_lossy(),
        );
    } else {
        if let Some(dir) = secret_key_path.parent() {
            fs::create_dir_all(dir).unwrap();
        }
        generate_keys_file(&secret_key_path, &passphrase).unwrap()
    }
}

/// Generate final node configuration using public configs
/// of other nodes in the network.
#[derive(StructOpt, Debug, Serialize, Deserialize)]
#[structopt(rename_all = "kebab-case")]
pub struct Finalize {
    /// Path to a secret part of a node configuration.
    pub secret_config_path: PathBuf,
    /// Path to a node configuration file which will be created after
    /// running this command.
    pub output_config_path: PathBuf,
    #[structopt(long, short = "p")]
    /// List of paths to public parts of configuration of all the nodes
    /// in the network.
    pub public_configs: Vec<PathBuf>,
    #[structopt(long)]
    /// Listen address for node public API.
    ///
    /// Public API is used mainly for sending API requests to user services.
    pub public_api_address: Option<SocketAddr>,
    #[structopt(long)]
    /// Listen address for node private API.
    ///
    /// Private API is used by node administrators for node monitoring and control.
    pub private_api_address: Option<SocketAddr>,
    #[structopt(long)]
    /// Cross-origin resource sharing options for responses returned by public API handlers.
    pub public_allow_origin: Option<String>,
    #[structopt(long)]
    /// Cross-origin resource sharing options for responses returned by private API handlers.
    pub private_allow_origin: Option<String>,
}

impl Finalize {
    fn reduce_configs(
        public_configs: Vec<SharedConfig>,
        our_config: &NodePrivateConfig,
    ) -> (
        CommonConfigTemplate,
        Vec<NodePublicConfig>,
        Option<NodePublicConfig>,
    ) {
        let mut map = BTreeMap::new();
        let mut config_iter = public_configs.into_iter();
        let first = config_iter
            .next()
            .expect("Expected at least one config in PUBLIC_CONFIGS");
        let common = first.common;
        map.insert(first.node.validator_keys.consensus_key, first.node);

        for config in config_iter {
            if common != config.common {
                panic!("Found config with different common part.");
            };
            if map
                .insert(config.node.validator_keys.consensus_key, config.node)
                .is_some()
            {
                panic!("Found duplicate consensus keys in PUBLIC_CONFIGS");
            }
        }
        (
            common,
            map.iter().map(|(_, c)| c.clone()).collect(),
            map.get(&our_config.consensus_public_key).cloned(),
        )
    }

    fn create_connect_list_config(
        list: &[NodePublicConfig],
        node: &NodePrivateConfig,
    ) -> ConnectListConfig {
        let peers = list
            .iter()
            .filter(|config| config.validator_keys.consensus_key != node.consensus_public_key)
            .map(|config| ConnectInfo {
                public_key: config.validator_keys.consensus_key,
                address: config.address.clone(),
            })
            .collect();

        ConnectListConfig { peers }
    }
}

impl ExonumCommand for Finalize {
    fn execute(self) -> Result<StandardResult, Error> {
        let secret_config: NodePrivateConfig = load_config_file(&self.secret_config_path)?;
        let secret_config_dir = std::env::current_dir()
            .expect("Failed to get current dir")
            .join(self.secret_config_path.parent().unwrap());
        let public_configs: Vec<SharedConfig> = self
            .public_configs
            .into_iter()
            .map(|path| load_config_file(path))
            .collect::<Result<_, _>>()?;

        let public_allow_origin = self.public_allow_origin.map(|s| s.parse().unwrap());
        let private_allow_origin = self.private_allow_origin.map(|s| s.parse().unwrap());

        let (common, list, _our) = Self::reduce_configs(public_configs, &secret_config);

        let validators_count = common.general_config.validators_count as usize;

        if validators_count != list.len() {
            panic!(
                "The number of validators configs does not match the number of validators keys."
            );
        }

        let genesis = GenesisConfig::new_with_consensus(
            common.clone().consensus_config,
            list.iter().map(|c| c.validator_keys),
        );

        let connect_list = Self::create_connect_list_config(&list, &secret_config);

        let config = {
            NodeConfig {
                listen_address: secret_config.listen_address,
                external_address: secret_config.external_address,
                network: Default::default(),
                consensus_public_key: secret_config.consensus_public_key,
                consensus_secret_key: secret_config_dir.join(&secret_config.consensus_secret_key),
                service_public_key: secret_config.service_public_key,
                service_secret_key: secret_config_dir.join(&secret_config.service_secret_key),
                genesis,
                api: NodeApiConfig {
                    public_api_address: self.public_api_address,
                    private_api_address: self.private_api_address,
                    public_allow_origin,
                    private_allow_origin,
                    ..Default::default()
                },
                mempool: Default::default(),
                services_configs: Default::default(),
                database: Default::default(),
                connect_list,
                thread_pool_size: Default::default(),
            }
        };

        save_config_file(&config, &self.output_config_path)?;

        Ok(StandardResult::Finalize {
            node_config_path: self.output_config_path,
        })
    }
}

/// Run the node with provided node config.
#[derive(StructOpt, Debug, Serialize, Deserialize)]
#[structopt(rename_all = "kebab-case")]
pub struct Run {
    #[structopt(long, short = "c")]
    /// Path to a node configuration file.
    pub node_config: PathBuf,
    #[structopt(long, short = "d")]
    /// Path to a database directory.
    pub db_path: PathBuf,
    #[structopt(long)]
    /// Listen address for node public API.
    ///
    /// Public API is used mainly for sending API requests to user services.
    pub public_api_address: Option<SocketAddr>,
    #[structopt(long)]
    /// Listen address for node private API.
    ///
    /// Private API is used by node administrators for node monitoring and control.
    pub private_api_address: Option<SocketAddr>,
    #[structopt(long)]
    /// Passphrase entry method for consensus key.
    ///
    /// Possible values are: `stdin`, `env{:ENV_VAR_NAME}`, `pass:PASSWORD`.
    /// Default Value is `stdin`.
    /// If `ENV_VAR_NAME` is not specified `$EXONUM_CONSENSUS_PASS` is used
    /// by default.
    pub consensus_key_pass: Option<PassInputMethod>,
    #[structopt(long)]
    /// Passphrase entry method for service key.
    ///
    /// Possible values are: `stdin`, `env{:ENV_VAR_NAME}`, `pass:PASSWORD`.
    /// Default Value is `stdin`.
    /// If `ENV_VAR_NAME` is not specified `$EXONUM_CONSENSUS_PASS` is used
    /// by default.
    pub service_key_pass: Option<PassInputMethod>,
}

impl ExonumCommand for Run {
    fn execute(self) -> Result<StandardResult, Error> {
        let config_path = self.node_config;

        let mut config: NodeConfig<PathBuf> = load_config_file(&config_path)?;
        let public_addr = self.public_api_address;
        let private_addr = self.private_api_address;

        // Override api options
        if let Some(public_addr) = public_addr {
            config.api.public_api_address = Some(public_addr);
        }

        if let Some(private_api_address) = private_addr {
            config.api.private_api_address = Some(private_api_address);
        }

        let consensus_passphrase = self
            .consensus_key_pass
            .unwrap_or_default()
            .get_passphrase(SecretKeyType::Consensus, true);
        let service_passphrase = self
            .service_key_pass
            .unwrap_or_default()
            .get_passphrase(SecretKeyType::Service, true);

        let config = config.read_secret_keys(
            &config_path,
            consensus_passphrase.as_bytes(),
            service_passphrase.as_bytes(),
        );

        let run_config = NodeRunConfig {
            node_config: config,
            db_path: self.db_path,
        };

        Ok(StandardResult::Run(run_config))
    }
}
