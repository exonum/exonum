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

//! Standard Exonum CLI command used to run the node using prepared node
//! configuration file.

use failure::Error;
use serde_derive::{Deserialize, Serialize};
use structopt::StructOpt;

use std::{net::SocketAddr, path::PathBuf};

use crate::{
    command::{ExonumCommand, StandardResult},
    config::NodeConfig,
    io::load_config_file,
    password::{PassInputMethod, PassphraseUsage},
};

/// Container for node configuration parameters produced by `Run` command.
pub struct NodeRunConfig {
    /// Final node configuration parameters.
    pub node_config: NodeConfig,
    /// Path to a directory containing database files, provided by user.
    pub db_path: PathBuf,
    /// User-provided path to the node configuration file.
    pub node_config_path: PathBuf,
}

/// Run the node with provided node config.
#[derive(StructOpt, Debug, Serialize, Deserialize)]
pub struct Run {
    /// Path to a node configuration file.
    #[structopt(long, short = "c")]
    pub node_config: PathBuf,
    /// Path to a database directory.
    #[structopt(long, short = "d")]
    pub db_path: PathBuf,
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
    /// Passphrase entry method for master key.
    ///
    /// Possible values are: `stdin`, `env{:ENV_VAR_NAME}`, `pass:PASSWORD`.
    /// Default Value is `stdin`.
    /// If `ENV_VAR_NAME` is not specified `$EXONUM_MASTER_PASS` is used
    /// by default.
    #[structopt(long)]
    pub master_key_pass: Option<PassInputMethod>,
}

impl ExonumCommand for Run {
    fn execute(self) -> Result<StandardResult, Error> {
        let config_path = self.node_config.clone();

        let mut config: NodeConfig = load_config_file(&config_path)?;
        let public_addr = self.public_api_address;
        let private_addr = self.private_api_address;

        // Override api options
        if let Some(public_addr) = public_addr {
            config.private_config.api.public_api_address = Some(public_addr);
        }

        if let Some(private_api_address) = private_addr {
            config.private_config.api.private_api_address = Some(private_api_address);
        }

        let master_passphrase = self
            .master_key_pass
            .unwrap_or_default()
            .get_passphrase(PassphraseUsage::Using)?;

        config.read_secret_keys(&config_path, master_passphrase.as_bytes());

        let run_config = NodeRunConfig {
            node_config: config,
            db_path: self.db_path,
            node_config_path: self.node_config,
        };

        Ok(StandardResult::Run(run_config))
    }
}
