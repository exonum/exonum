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

//! Standard Exonum CLI command used to generate common configuration file.

use exonum_supervisor::mode::Mode as SupervisorMode;
use failure::Error;
use serde_derive::{Deserialize, Serialize};
use structopt::StructOpt;

use std::path::PathBuf;

use crate::{
    command::{ExonumCommand, StandardResult},
    config::{GeneralConfig, NodePublicConfig},
    io::save_config_file,
};

/// Generate common part of the nodes configuration.
#[derive(StructOpt, Debug, Serialize, Deserialize)]
pub struct GenerateTemplate {
    /// Path to a node configuration template file.
    pub common_config: PathBuf,
    /// Number of validators in the network.
    #[structopt(long)]
    pub validators_count: u32,
    /// Supervisor service mode. Possible options are "simple" and "decentralized".
    #[structopt(long)]
    pub supervisor_mode: SupervisorMode,
}

impl ExonumCommand for GenerateTemplate {
    fn execute(self) -> Result<StandardResult, Error> {
        let config = NodePublicConfig {
            consensus: Default::default(),
            general: GeneralConfig {
                validators_count: self.validators_count,
                supervisor_mode: self.supervisor_mode,
            },
            validator_keys: None,
        };
        save_config_file(&config, &self.common_config)?;
        Ok(StandardResult::GenerateTemplate {
            template_config_path: self.common_config,
        })
    }
}
