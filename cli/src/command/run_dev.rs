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

//! Standard Exonum CLI command used to run the node with default parameters
//! for developing purposes.

use anyhow::{Context, Error};
use exonum_supervisor::mode::Mode as SupervisorMode;
use serde_derive::{Deserialize, Serialize};
use structopt::StructOpt;

use std::{fs, net::SocketAddr, path::PathBuf, str::FromStr};

use crate::command::{
    finalize::Finalize,
    generate_config::{GenerateConfig, PRIVATE_CONFIG_FILE_NAME, PUBLIC_CONFIG_FILE_NAME},
    generate_template::GenerateTemplate,
    run::Run,
    ExonumCommand, StandardResult,
};

/// Run application in development mode (generate configuration and db files automatically).
#[derive(StructOpt, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RunDev {
    /// Path to a directory for blockchain database and configuration files.
    ///
    /// Database is located in <blockchain_path>/db directory, node configuration files
    /// are located in <blockchain_path>/config directory. Existing files and directories are
    /// reused. To generate new node configuration and start a new blockchain, the user must
    /// use --clean flag or specify an another directory.
    #[structopt(long, short = "-p")]
    pub blockchain_path: PathBuf,
    /// Listen address for node public API.
    ///
    /// Public API is used mainly for sending API requests to user services.
    #[structopt(long, default_value = "127.0.0.1:8080")]
    pub public_api_address: SocketAddr,
    /// Listen address for node private API.
    ///
    /// Private API is used by node administrators for node monitoring and control.
    #[structopt(long, default_value = "127.0.0.1:8081")]
    pub private_api_address: SocketAddr,
    /// Clean existing blockchain database and configuration files before run.
    #[structopt(long)]
    pub clean: bool,
}

impl RunDev {
    fn cleanup(&self) -> Result<(), Error> {
        let database_dir = self.blockchain_path.join("db");
        if database_dir.exists() {
            fs::remove_dir_all(&self.blockchain_path)
                .context("Expected DATABASE_PATH directory being removable")?;
        }
        Ok(())
    }

    fn allowed_origins(addr: SocketAddr, kind: &str) -> String {
        let mut allow_origin = format!("http://{}", addr);
        if addr.ip().is_loopback() {
            allow_origin += &format!(", http://localhost:{}", addr.port());
        } else {
            log::warn!(
                "Non-loopback {} API address used for `run-dev` command: {}",
                kind,
                addr
            );
        }
        allow_origin
    }
}

impl ExonumCommand for RunDev {
    fn execute(self) -> Result<StandardResult, Error> {
        if self.clean {
            self.cleanup()?;
        }

        let config_dir = self.blockchain_path.join("config");
        let node_config_path = config_dir.join("node.toml");
        let common_config_path = config_dir.join("template.toml");
        let public_config_path = config_dir.join(PUBLIC_CONFIG_FILE_NAME);
        let private_config_path = config_dir.join(PRIVATE_CONFIG_FILE_NAME);
        let db_path = self.blockchain_path.join("db");

        if !node_config_path.exists() {
            let generate_template = GenerateTemplate {
                common_config: common_config_path.clone(),
                validators_count: 1,
                supervisor_mode: SupervisorMode::Simple,
            };
            generate_template.execute()?;

            let generate_config = GenerateConfig {
                common_config: common_config_path,
                output_dir: config_dir,
                peer_address: "127.0.0.1:6200".parse().unwrap(),
                listen_address: None,
                no_password: true,
                master_key_pass: None,
                master_key_path: None,
            };
            generate_config.execute()?;

            let public_origins = Self::allowed_origins(self.public_api_address, "public");
            let private_origins = Self::allowed_origins(self.private_api_address, "private");
            let finalize = Finalize {
                private_config_path,
                output_config_path: node_config_path.clone(),
                public_configs: vec![public_config_path],
                public_api_address: Some(self.public_api_address),
                private_api_address: Some(self.private_api_address),
                public_allow_origin: Some(public_origins),
                private_allow_origin: Some(private_origins),
            };
            finalize.execute()?;
        }

        let run = Run {
            node_config: node_config_path,
            db_path,
            public_api_address: None,
            private_api_address: None,
            master_key_pass: Some(FromStr::from_str("pass:").unwrap()),
        };
        run.execute()
    }
}
