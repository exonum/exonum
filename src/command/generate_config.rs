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

use exonum::blockchain::ValidatorKeys;
use exonum::crypto::generate_keys_file;
use failure::Error;
use serde::{Deserialize, Serialize};
use structopt::StructOpt;

use std::fs;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};

use crate::command::{ExonumCommand, StandardResult};
use crate::config::{CommonConfigTemplate, NodePrivateConfig, NodePublicConfig, SharedConfig};
use crate::io::{load_config_file, save_config_file};
use crate::password::{PassInputMethod, Passphrase, SecretKeyType, PassphraseUsage};

/// Name for a file containing consensus secret key.
pub const CONSENSUS_SECRET_KEY_NAME: &str = "consensus.key.toml";
/// Name for a file containing service secret key.
pub const SERVICE_SECRET_KEY_NAME: &str = "service.key.toml";
/// Name for a file containing the public part of the node configuration.
pub const PUB_CONFIG_FILE_NAME: &str = "pub.toml";
/// Name for a file containing the secret part of the node configuration.
pub const SEC_CONFIG_FILE_NAME: &str = "sec.toml";

/// Default port number used by Exonum for communication between nodes.
pub const DEFAULT_EXONUM_LISTEN_PORT: u16 = 6333;

/// Generate public and private configs of the node.
#[derive(StructOpt, Debug, Serialize, Deserialize)]
#[structopt(rename_all = "kebab-case")]
pub struct GenerateConfig {
    /// Path to node configuration template file.
    pub common_config: PathBuf,
    /// Path to a directory where public and private node configuration files
    /// will be saved.
    pub output_dir: PathBuf,
    #[structopt(
        long,
        short = "a",
        parse(try_from_str = "GenerateConfig::parse_external_address")
    )]
    /// External IP address of the node used for communications between nodes.
    ///
    /// If no port is provided, the default Exonum port 6333 is used.
    pub peer_address: SocketAddr,
    #[structopt(long, short = "l")]
    /// Listen IP address of the node used for communications between nodes.
    ///
    /// If not provided it combined from all-zeros (0.0.0.0) IP address and
    /// the port number of the `peer-address`.
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
    /// If `ENV_VAR_NAME` is not specified `$EXONUM_SERVICE_PASS` is used
    /// by default.
    pub service_key_pass: Option<PassInputMethod>,
}

impl GenerateConfig {
    fn get_passphrase(
        no_password: bool,
        method: PassInputMethod,
        secret_key_type: SecretKeyType,
    ) -> Result<Passphrase, Error> {
        if no_password {
            Ok(Passphrase::default())
        } else {
            method.get_passphrase(secret_key_type, PassphraseUsage::SettingUp)
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
        if provided.is_some() {
            provided.unwrap()
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
        let common_config: CommonConfigTemplate = load_config_file(self.common_config.clone())?;

        let pub_config_path = self.output_dir.join(PUB_CONFIG_FILE_NAME);
        let private_config_path = self.output_dir.join(SEC_CONFIG_FILE_NAME);
        let consensus_secret_key_path = self.output_dir.join(CONSENSUS_SECRET_KEY_NAME);
        let service_secret_key_path = self.output_dir.join(SERVICE_SECRET_KEY_NAME);

        let listen_address = Self::get_listen_address(self.listen_address, self.peer_address);

        let consensus_public_key = {
            let passphrase = Self::get_passphrase(
                self.no_password,
                self.consensus_key_pass.unwrap_or_default(),
                SecretKeyType::Consensus,
            )?;
            create_secret_key_file(&consensus_secret_key_path, passphrase.as_bytes())
        };
        let service_public_key = {
            let passphrase = Self::get_passphrase(
                self.no_password,
                self.service_key_pass.unwrap_or_default(),
                SecretKeyType::Service,
            )?;
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
