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

//! Standard Exonum CLI command used to run the node with default parameters
//! for developing purposes.

use exonum_supervisor::mode::Mode as SupervisorMode;
use failure::{Error, ResultExt};
use serde_derive::{Deserialize, Serialize};
use structopt::StructOpt;

use std::{fs, path::PathBuf, str::FromStr};

use crate::command::{
    finalize::Finalize,
    generate_config::{GenerateConfig, PRIVATE_CONFIG_FILE_NAME, PUBLIC_CONFIG_FILE_NAME},
    generate_template::GenerateTemplate,
    run::Run,
    ExonumCommand, StandardResult,
};

/// Run application in development mode (generate configuration and db files automatically).
#[derive(StructOpt, Debug, Serialize, Deserialize)]
pub struct RunDev {
    /// The path where configuration and db files will be generated.
    #[structopt(long, short = "a")]
    pub artifacts_dir: PathBuf,
}

impl RunDev {
    fn artifact_path(&self, artifact_name: &str) -> PathBuf {
        let mut path = self.artifacts_dir.clone();
        path.push(artifact_name);
        path
    }

    fn cleanup(&self) -> Result<(), Error> {
        let database_dir = self.artifact_path("db");
        if database_dir.exists() {
            fs::remove_dir_all(self.artifacts_dir.clone())
                .context("Expected DATABASE_PATH folder being removable.")?;
        }
        Ok(())
    }
}

impl ExonumCommand for RunDev {
    fn execute(self) -> Result<StandardResult, Error> {
        self.cleanup()?;

        let common_config = self.artifact_path("template.toml");

        let generate_template = GenerateTemplate {
            common_config: common_config.clone(),
            validators_count: 1,
            supervisor_mode: SupervisorMode::Simple,
        };
        generate_template.execute()?;

        let generate_config = GenerateConfig {
            common_config: common_config.clone(),
            output_dir: self.artifacts_dir.clone(),
            peer_address: "127.0.0.1:6200".parse().unwrap(),
            listen_address: None,
            no_password: true,
            master_key_pass: None,
            master_key_path: None,
        };
        generate_config.execute()?;

        let node_config_file_name = "node.toml";

        let finalize = Finalize {
            private_config_path: self.artifact_path(PRIVATE_CONFIG_FILE_NAME),
            output_config_path: self.artifact_path(node_config_file_name),
            public_configs: vec![self.artifact_path(PUBLIC_CONFIG_FILE_NAME)],
            public_api_address: Some("127.0.0.1:8080".parse().unwrap()),
            private_api_address: Some("127.0.0.1:8081".parse().unwrap()),
            public_allow_origin: Some("http://127.0.0.1:8080, http://localhost:8080".to_string()),
            private_allow_origin: Some("http://127.0.0.1:8081, http://localhost:8081".to_string()),
        };
        finalize.execute()?;

        let run = Run {
            node_config: self.artifact_path(node_config_file_name),
            db_path: self.artifact_path("db"),
            public_api_address: None,
            private_api_address: None,
            master_key_pass: Some(FromStr::from_str("pass:").unwrap()),
        };
        run.execute()
    }
}
