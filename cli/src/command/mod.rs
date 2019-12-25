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

//! Standard Exonum CLI node configuration commands.

pub mod finalize;
pub mod generate_config;
pub mod generate_template;
pub mod maintenance;
pub mod run;
pub mod run_dev;

use failure::Error;
use serde_derive::{Deserialize, Serialize};
use structopt::StructOpt;

use std::path::PathBuf;

use crate::command::{
    finalize::Finalize,
    generate_config::GenerateConfig,
    generate_template::GenerateTemplate,
    maintenance::{Action, Maintenance},
    run::{NodeRunConfig, Run},
    run_dev::RunDev,
};

/// Interface of standard Exonum Core configuration command.
pub trait ExonumCommand {
    /// Returns the result of the command execution.
    fn execute(self) -> Result<StandardResult, Error>;
}

/// Standard Exonum Core configuration command.
#[derive(StructOpt, Debug, Serialize, Deserialize)]
#[structopt(author, about)]
pub enum Command {
    /// Generate common part of the nodes configuration.
    #[structopt(name = "generate-template")]
    GenerateTemplate(GenerateTemplate),
    /// Generate public and private configs of the node.
    #[structopt(name = "generate-config")]
    GenerateConfig(GenerateConfig),
    /// Generate final node configuration using public configs
    /// of other nodes in the network.
    #[structopt(name = "finalize")]
    Finalize(Finalize),
    /// Run the node with provided node config.
    #[structopt(name = "run")]
    Run(Run),
    /// Run the node with auto-generated config.
    #[structopt(name = "run-dev")]
    RunDev(RunDev),
    /// Perform different maintenance actions.
    #[structopt(name = "maintenance")]
    Maintenance(Maintenance),
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
            Command::RunDev(command) => command.execute(),
            Command::Maintenance(command) => command.execute(),
        }
    }
}

/// Output of any of the standard Exonum Core configuration commands.
#[allow(clippy::large_enum_variant)]
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
        private_config_path: PathBuf,
        /// Path to a master key of the node.
        master_key_path: PathBuf,
    },
    /// `finalize` command output.
    Finalize {
        /// Path to a generated final node config.
        node_config_path: PathBuf,
    },
    /// `run` command output.
    Run(NodeRunConfig),
    /// `maintenance` command output.
    Maintenance {
        /// Path to a node configuration file.
        node_config_path: PathBuf,
        /// Path to a database directory.
        db_path: PathBuf,
        /// Performed action.
        performed_action: Action,
    },
}
