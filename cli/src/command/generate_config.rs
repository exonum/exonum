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

//! Standard Exonum CLI command used to generate public and secret config files
//! of the node using provided common configuration file.

use exonum::{
    blockchain::ValidatorKeys,
    keys::{generate_keys, Keys},
};
use failure::{bail, Error};
use serde_derive::{Deserialize, Serialize};
use structopt::StructOpt;

use std::{
    fs,
    net::{IpAddr, SocketAddr},
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
pub struct GenerateConfig {
    /// Path to node configuration template file.
    pub common_config: PathBuf,
    /// Path to a directory where public and private node configuration files
    /// will be saved.
    pub output_dir: PathBuf,
    /// External IP address of the node used for communications between nodes.
    ///
    /// If no port is provided, the default Exonum port 6333 is used.
    #[structopt(
        long,
        short = "a",
        parse(try_from_str = GenerateConfig::parse_external_address)
    )]
    pub peer_address: SocketAddr,
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
    fn parse_external_address(input: &str) -> Result<SocketAddr, Error> {
        if let Ok(address) = input.parse() {
            Ok(address)
        } else {
            let ip_address = input.parse()?;
            Ok(SocketAddr::new(ip_address, DEFAULT_EXONUM_LISTEN_PORT))
        }
    }

    /// Returns `provided` address or and address combined from all-zeros
    /// IP address and the port number from `external_address`.
    fn get_listen_address(
        provided: Option<SocketAddr>,
        external_address: SocketAddr,
    ) -> SocketAddr {
        if let Some(provided) = provided {
            provided
        } else {
            let ip_address = match external_address.ip() {
                IpAddr::V4(_) => "0.0.0.0".parse().unwrap(),
                IpAddr::V6(_) => "::".parse().unwrap(),
            };
            SocketAddr::new(ip_address, external_address.port())
        }
    }
}

impl ExonumCommand for GenerateConfig {
    fn execute(self) -> Result<StandardResult, Error> {
        let common_config: NodePublicConfig = load_config_file(&self.common_config)?;

        let public_config_path = self.output_dir.join(PUBLIC_CONFIG_FILE_NAME);
        let private_config_path = self.output_dir.join(PRIVATE_CONFIG_FILE_NAME);
        let master_key_path = get_master_key_path(self.master_key_path.clone())?;

        let listen_address = Self::get_listen_address(self.listen_address, self.peer_address);

        let keys = {
            let passphrase =
                Self::get_passphrase(self.no_password, self.master_key_pass.unwrap_or_default());
            create_keys_and_files(
                &self.output_dir.join(master_key_path.clone()),
                passphrase?.as_bytes(),
            )
        }?;

        let validator_keys = ValidatorKeys {
            consensus_key: keys.consensus_pk(),
            service_key: keys.service_pk(),
        };
        let public_config = NodePublicConfig {
            validator_keys: Some(validator_keys),
            ..common_config
        };
        // Save public config separately.
        save_config_file(&public_config, &public_config_path)?;

        let private_config = NodePrivateConfig {
            listen_address,
            external_address: self.peer_address.to_string(),
            master_key_path: master_key_path.clone(),
            api: Default::default(),
            network: Default::default(),
            mempool: Default::default(),
            database: Default::default(),
            thread_pool_size: Default::default(),
            connect_list: Default::default(),
            keys,
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
) -> Result<Keys, failure::Error> {
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
