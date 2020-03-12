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

//! Standard Exonum CLI command used to perform different maintenance actions.

use anyhow::Error;
use exonum::merkledb::{migration::rollback_migration, Database, RocksDB};
use exonum::runtime::remove_local_migration_result;
use exonum_node::helpers::clear_consensus_messages_cache;
use serde_derive::{Deserialize, Serialize};
use structopt::StructOpt;

use std::path::{Path, PathBuf};

use crate::{
    command::{ExonumCommand, StandardResult},
    config::NodeConfig,
    io::load_config_file,
};

/// Perform different maintenance actions.
#[derive(StructOpt, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Maintenance {
    /// Path to a node configuration file.
    #[structopt(long, short = "c")]
    pub node_config: PathBuf,

    /// Path to a database directory.
    #[structopt(long, short = "d")]
    pub db_path: PathBuf,

    /// Action to be performed.
    #[structopt(subcommand)]
    pub action: MaintenanceAction,
}

/// Available maintenance actions.
#[derive(StructOpt, Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum MaintenanceAction {
    /// Clear consensus messages cache.
    #[structopt(name = "clear-cache")]
    ClearCache,

    /// Restart migration script.
    #[structopt(name = "restart-migration")]
    RestartMigration {
        /// Name of the service for migration restart, e.g. "explorer" or "my-service".
        service_name: String,
    },
}

impl MaintenanceAction {
    fn clear_cache(node_config: &Path, db_path: &Path) -> Result<(), Error> {
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

    fn restart_migration(
        node_config: &Path,
        db_path: &Path,
        service_name: &str,
    ) -> Result<(), Error> {
        let node_config: NodeConfig = load_config_file(node_config)?;
        let db: Box<dyn Database> = Box::new(RocksDB::open(
            db_path,
            &node_config.private_config.database,
        )?);
        let mut fork = db.fork();
        rollback_migration(&mut fork, service_name);
        remove_local_migration_result(&fork, service_name);
        db.merge_sync(fork.into_patch())?;

        Ok(())
    }
}

impl ExonumCommand for Maintenance {
    fn execute(self) -> Result<StandardResult, Error> {
        match self.action {
            MaintenanceAction::ClearCache => {
                MaintenanceAction::clear_cache(&self.node_config, &self.db_path)?
            }
            MaintenanceAction::RestartMigration { ref service_name } => {
                MaintenanceAction::restart_migration(
                    &self.node_config,
                    &self.db_path,
                    service_name,
                )?
            }
        }

        Ok(StandardResult::Maintenance {
            node_config_path: self.node_config,
            db_path: self.db_path,
            performed_action: self.action,
        })
    }
}
