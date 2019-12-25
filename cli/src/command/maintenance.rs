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

//! Standard Exonum CLI command used to perform different maintenance actions.

use exonum::{
    exonum_merkledb::{Database, RocksDB},
    helpers::clear_consensus_messages_cache,
};
use failure::Error;
use serde_derive::{Deserialize, Serialize};
use structopt::StructOpt;

use std::path::PathBuf;

use crate::{
    command::{ExonumCommand, StandardResult},
    config::NodeConfig,
    io::load_config_file,
};

/// Perform different maintenance actions.
#[derive(StructOpt, Debug, Serialize, Deserialize)]
pub struct Maintenance {
    /// Path to a node configuration file.
    #[structopt(long, short = "c")]
    pub node_config: PathBuf,
    /// Path to a database directory.
    #[structopt(long, short = "d")]
    pub db_path: PathBuf,
    /// Action to be performed.
    #[structopt(subcommand)]
    pub action: Action,
}

/// Available maintenance actions.
#[derive(StructOpt, Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    /// Clear consensus messages cache.
    #[structopt(name = "clear-cache")]
    ClearCache,
}

impl Action {
    fn clear_cache(node_config: PathBuf, db_path: PathBuf) -> Result<(), Error> {
        let node_config: NodeConfig = load_config_file(node_config)?;
        let db: Box<dyn Database> = Box::new(RocksDB::open(
            db_path,
            &node_config.private_config.database,
        )?);
        let fork = db.fork();
        clear_consensus_messages_cache(&fork);
        db.merge_sync(fork.into_patch())?;
        Ok(())
    }
}

impl ExonumCommand for Maintenance {
    fn execute(self) -> Result<StandardResult, Error> {
        match self.action {
            Action::ClearCache => {
                Action::clear_cache(self.node_config.clone(), self.db_path.clone())?
            }
        }
        Ok(StandardResult::Maintenance {
            node_config_path: self.node_config,
            db_path: self.db_path,
            performed_action: self.action,
        })
    }
}
