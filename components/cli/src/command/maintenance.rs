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

use exonum::exonum_merkledb::{Database, RocksDB};
use exonum::helpers::clear_consensus_messages_cache;
use exonum::node::NodeConfig;
use failure::Error;
use serde::{Deserialize, Serialize};
use structopt::StructOpt;

use std::path::PathBuf;

use crate::command::{ExonumCommand, StandardResult};
use crate::io::load_config_file;

/// Perform different maintenance actions.
#[derive(StructOpt, Debug, Serialize, Deserialize)]
#[structopt(rename_all = "kebab-case")]
pub struct Maintenance {
    #[structopt(long, short = "c")]
    /// Path to a node configuration file.
    pub node_config: PathBuf,
    #[structopt(long, short = "d")]
    /// Path to a database directory.
    pub db_path: PathBuf,
    #[structopt(subcommand)]
    /// Action to be performed.
    pub action: Action,
}

/// Available maintenance actions.
#[derive(StructOpt, Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    #[structopt(name = "clear-cache")]
    /// Clear consensus messages cache.
    ClearCache,
}

impl Action {
    fn clear_cache(node_config: PathBuf, db_path: PathBuf) -> Result<(), Error> {
        let node_config: NodeConfig<PathBuf> = load_config_file(node_config)?;
        let db: Box<dyn Database> = Box::new(RocksDB::open(db_path, &node_config.database)?);
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
