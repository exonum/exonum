// Copyright 2020 The Exonum Team
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

//! Standard Exonum CLI command used to generate public and secret config files
//! of the node using provided common configuration file.

use anyhow::{bail, Error};
use exonum::{
    blockchain::ValidatorKeys,
    keys::{generate_keys, Keys},
    merkledb::DbOptions,
};
use exonum_node::{ConnectListConfig, MemoryPoolConfig, NetworkConfiguration, NodeApiConfig};
use serde_derive::{Deserialize, Serialize};
use structopt::StructOpt;

use std::{
    fs,
    io::ErrorKind,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs},
    path::{Path, PathBuf},
};

use crate::{
    command::{ExonumCommand, StandardResult},
    config::{NodePrivateConfig, NodePublicConfig},
    io::{load_config_file, save_config_file},
    password::{PassInputMethod, Passphrase, PassphraseUsage},
};

/// Name for a file containing the public part of the node configuration.
pub const PUBLIC_CONFIG_FILE_NAME: &str = "pub.toml";
/// Name for a file containing the secret part of the node configuration.
pub const PRIVATE_CONFIG_FILE_NAME: &str = "sec.toml";
/// Name for a encrypted file containing the node master key.
pub const MASTER_KEY_FILE_NAME: &str = "master.key.toml";

/// Default port number used by Exonum for communication between nodes.
pub const DEFAULT_EXONUM_LISTEN_PORT: u16 = 6333;

/// Generate public and private configs of the node.
#[derive(StructOpt, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct GenerateConfig {
    /// Path to node configuration template file.
    pub common_config: PathBuf,

    /// Path to a directory where public and private node configuration files
    /// will be saved.
    pub output_dir: PathBuf,

    /// External IPv4/IPv6 address or domain name of the node used for communications
    /// between nodes.
    ///
    /// If no port is provided, the default Exonum port 6333 is used.
    #[structopt(long, short = "a")]
    pub peer_address: String,

    /// Listen IP address of the node used for communications between nodes.
    ///
    /// If not provided it combined from all-zeros (0.0.0.0) IP address and
    /// the port number of the `peer-address`.
    #[structopt(long, short = "l")]
    pub listen_address: Option<SocketAddr>,

    /// Don't prompt for passwords when generating private keys.
    #[structopt(long, short = "n")]
    pub no_password: bool,

    /// Passphrase entry method for master key.
    ///
    /// Possible values are: `stdin`, `env{:ENV_VAR_NAME}`, `pass:PASSWORD`.
    /// Default Value is `stdin`.
    /// If `ENV_VAR_NAME` is not specified `$EXONUM_MASTER_PASS` is used
    /// by default.
    #[structopt(long)]
    pub master_key_pass: Option<PassInputMethod>,

    /// Path to the master key file. If empty, file will be placed to <output_dir>.
    #[structopt(long)]
    pub master_key_path: Option<PathBuf>,
}

impl GenerateConfig {
    fn get_passphrase(no_password: bool, method: PassInputMethod) -> Result<Passphrase, Error> {
        if no_password {
            Ok(Passphrase::default())
        } else {
            method.get_passphrase(PassphraseUsage::SettingUp)
        }
    }

    /// If no port is provided by user, uses `DEFAULT_EXONUM_LISTEN_PORT`.
    fn resolve_peer_address(hostname: &str, is_host_with_port: bool) -> Result<SocketAddr, Error> {
        match hostname.to_socket_addrs() {
            Ok(mut addrs) => addrs.next().ok_or_else(|| {
                Error::msg(format!(
                    "No one IP address is related to the domain: {}",
                    hostname
                ))
            }),
            Err(e) if e.kind() == ErrorKind::InvalidInput && !is_host_with_port => {
                Self::resolve_peer_address(
                    &format!("{}:{}", hostname, DEFAULT_EXONUM_LISTEN_PORT),
                    true,
                )
            }
            Err(e) => Err(Error::from(e)),
        }
    }

    /// Returns `provided` address or [`INADDR_ANY`](https://en.wikipedia.org/wiki/0.0.0.0) address
    /// combined with the port number obtained from `peer_address`.
    fn get_listen_address(provided: Option<SocketAddr>, peer_address: &str) -> SocketAddr {
        provided.unwrap_or_else(|| match Self::resolve_peer_address(peer_address, false) {
            Ok(address) => {
                let ip_address = match address.ip() {
                    IpAddr::V4(_) => Ipv4Addr::UNSPECIFIED.into(),
                    IpAddr::V6(_) => Ipv6Addr::UNSPECIFIED.into(),
                };
                SocketAddr::new(ip_address, address.port())
            }
            Err(e) => panic!(e),
        })
    }
}

impl ExonumCommand for GenerateConfig {
    fn execute(self) -> Result<StandardResult, Error> {
        let common_config: NodePublicConfig = load_config_file(&self.common_config)?;

        let public_config_path = self.output_dir.join(PUBLIC_CONFIG_FILE_NAME);
        let private_config_path = self.output_dir.join(PRIVATE_CONFIG_FILE_NAME);
        let master_key_path = get_master_key_path(self.master_key_path.clone())?;

        let listen_address = Self::get_listen_address(self.listen_address, &self.peer_address);

        let keys = {
            let passphrase =
                Self::get_passphrase(self.no_password, self.master_key_pass.unwrap_or_default())?;
            create_keys_and_files(
                &self.output_dir.join(master_key_path.clone()),
                passphrase.as_bytes(),
            )
        }?;

        let validator_keys = ValidatorKeys::new(keys.consensus_pk(), keys.service_pk());
        let public_config = NodePublicConfig {
            validator_keys: Some(validator_keys),
            address: Some(self.peer_address.to_string()),
            ..common_config
        };
        // Save public config separately.
        save_config_file(&public_config, &public_config_path)?;

        let private_config = NodePrivateConfig {
            listen_address,
            external_address: self.peer_address.to_string(),
            master_key_path: master_key_path.clone(),
            api: NodeApiConfig::default(),
            network: NetworkConfiguration::default(),
            mempool: MemoryPoolConfig::default(),
            database: DbOptions::default(),
            thread_pool_size: None,
            connect_list: ConnectListConfig::default(),
            consensus_public_key: keys.consensus_pk(),
        };

        save_config_file(&private_config, &private_config_path)?;

        Ok(StandardResult::GenerateConfig {
            public_config_path,
            private_config_path,
            master_key_path,
        })
    }
}

fn get_master_key_path(path: Option<PathBuf>) -> Result<PathBuf, Error> {
    let path = path.map_or_else(|| Ok(PathBuf::new()), |path| path.canonicalize())?;
    Ok(path.join(MASTER_KEY_FILE_NAME))
}

fn create_keys_and_files(
    secret_key_path: impl AsRef<Path>,
    passphrase: impl AsRef<[u8]>,
) -> anyhow::Result<Keys> {
    let secret_key_path = secret_key_path.as_ref();
    if secret_key_path.exists() {
        bail!(
            "Failed to create secret key file. File exists: {}",
            secret_key_path.to_string_lossy(),
        )
    } else {
        if let Some(dir) = secret_key_path.parent() {
            fs::create_dir_all(dir)?;
        }
        generate_keys(&secret_key_path, passphrase.as_ref())
    }
}
