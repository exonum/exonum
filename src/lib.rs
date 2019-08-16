use serde::{Deserialize, Serialize};
use structopt::StructOpt;
use failure::{Error, ResultExt};

use std::net::SocketAddr;
use std::path::{PathBuf, Path};

use crate::password::{PassInputMethod, SecretKeyType, ZeroizeOnDrop};
use exonum::node::{NodeConfig, ConnectListConfig, NodeApiConfig};
use std::sync::Arc;
use exonum::helpers::fabric::{CommonConfigTemplate, GeneralConfig, NodePublicConfig, SharedConfig, NodePrivateConfig};
use exonum::crypto::generate_keys_file;
use crate::config::{save_config_file, load_config_file};
use std::fs;
use exonum::blockchain::{ValidatorKeys, GenesisConfig};
use std::collections::BTreeMap;

mod config;
mod password;

/// Default port value.
pub const DEFAULT_EXONUM_LISTEN_PORT: u16 = 6333;
pub const CONSENSUS_SECRET_KEY_NAME: &str = "consensus.key.toml";
pub const SERVICE_SECRET_KEY_NAME: &str = "service.key.toml";
pub const PUB_CONFIG_FILE_NAME: &str = "pub.toml";
pub const SEC_CONFIG_FILE_NAME: &str = "sec.toml";

pub enum StandardResult {
    GenerateTemplate {
        template_config_path: PathBuf,
    },
    GenerateConfig {
        public_config_path: PathBuf,
        secret_config_path: PathBuf,
    },
    Finalize {
        node_config_path: PathBuf,
    },
    Run(NodeRunConfig),
}

pub struct NodeRunConfig {
    pub node_config: NodeConfig,
    pub db_path: PathBuf,
}

pub trait ExonumCommand {
    fn execute(self) -> Result<StandardResult, Error>;
}

#[derive(StructOpt, Debug, Serialize, Deserialize)]
pub enum Command {
    #[structopt(name = "generate-template")]
    GenerateTemplate(GenerateTemplate),
    #[structopt(name = "generate-config")]
    GenerateConfig(GenerateConfig),
    #[structopt(name = "finalize")]
    Finalize(Finalize),
    #[structopt(name = "run")]
    Run(Run),
}

impl Command {
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

#[derive(StructOpt, Debug, Serialize, Deserialize)]
#[structopt(rename_all = "kebab-case")]
pub struct GenerateTemplate {
    pub common_config: PathBuf,
    #[structopt(long)]
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

#[derive(StructOpt, Debug, Serialize, Deserialize)]
#[structopt(rename_all = "kebab-case")]
pub struct GenerateConfig {
    pub common_config: PathBuf,
    pub output_dir: PathBuf,
    #[structopt(long, short = "a")]
    pub peer_address: SocketAddr,
    #[structopt(long, short = "l")]
    pub listen_address: Option<SocketAddr>,
    #[structopt(long, short = "n")]
    pub no_password: bool,
    #[structopt(long)]
    pub consensus_key_pass: Option<PassInputMethod>,
    #[structopt(long)]
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

        let listen_address: SocketAddr = self.listen_address.unwrap_or_else(|| SocketAddr::new("0.0.0.0".parse().unwrap(), self.peer_address.port()));

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

#[derive(StructOpt, Debug, Serialize, Deserialize)]
#[structopt(rename_all = "kebab-case")]
pub struct Finalize {
    pub secret_config_path: PathBuf,
    pub output_config_path: PathBuf,
    #[structopt(long, short = "p")]
    pub public_configs: Vec<PathBuf>,
    #[structopt(long)]
    pub public_api_address: Option<SocketAddr>,
    #[structopt(long)]
    pub private_api_address: Option<SocketAddr>,
    #[structopt(long)]
    pub public_allow_origin: Option<String>,
    #[structopt(long)]
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
}

impl ExonumCommand for Finalize {
    fn execute(self) -> Result<StandardResult, Error> {
        let secret_config: NodePrivateConfig =
            load_config_file(&self.secret_config_path)?;
        let secret_config_dir = std::env::current_dir()
            .expect("Failed to get current dir")
            .join(self.secret_config_path.parent().unwrap());
        let public_configs: Vec<SharedConfig> = self.public_configs
            .into_iter()
            .map(|path| load_config_file(path))
            .collect::<Result<_, _>>()?;

        let public_allow_origin = self.public_allow_origin.map(|s| s.parse().unwrap());
        let private_allow_origin = self.private_allow_origin.map(|s| s.parse().unwrap());

        let (common, list, our) = Self::reduce_configs(public_configs, &secret_config);

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

        let connect_list = ConnectListConfig::from_node_config(&list, &secret_config);

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

#[derive(StructOpt, Debug, Serialize, Deserialize)]
#[structopt(rename_all = "kebab-case")]

pub struct Run {
    #[structopt(long, short = "c")]
    pub node_config: PathBuf,
    #[structopt(long, short = "d")]
    pub db_path: PathBuf,
    #[structopt(long)]
    pub public_api_address: Option<SocketAddr>,
    #[structopt(long)]
    pub private_api_address: Option<SocketAddr>,
    #[structopt(long)]
    pub consensus_key_pass: Option<PassInputMethod>,
    #[structopt(long)]
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

        let consensus_passphrase = self.consensus_key_pass.unwrap_or_default().get_passphrase(SecretKeyType::Consensus, true);
        let service_passphrase = self.service_key_pass.unwrap_or_default().get_passphrase(SecretKeyType::Service, true);

        let config = config.read_secret_keys(&config_path, consensus_passphrase.as_bytes(), service_passphrase.as_bytes());

        let run_config = NodeRunConfig {
            node_config: config,
            db_path: self.db_path,
        };

        Ok(StandardResult::Run(run_config))
    }
}
