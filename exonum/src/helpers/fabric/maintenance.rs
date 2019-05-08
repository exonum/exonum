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

//! This module implements node maintenance actions.

use std::{collections::HashMap, path::Path};

use super::{
    internal::{CollectedCommand, Command, Feedback},
    Argument, CommandName, Context,
};
use crate::blockchain::Schema;
use crate::helpers::config::ConfigFile;
use crate::node::NodeConfig;
use exonum_merkledb::{Database, DbOptions, RocksDB};

// Context entry for the path to the node config.
const NODE_CONFIG_PATH: &str = "NODE_CONFIG_PATH";
// Context entry for the path to the database.
const DATABASE_PATH: &str = "DATABASE_PATH";
// Context entry for the type of action to be performed.
const MAINTENANCE_ACTION_PATH: &str = "MAINTENANCE_ACTION_PATH";

/// Maintenance command. Supported actions:
///
/// - `clear-cache` - clear message cache.
#[derive(Debug)]
pub struct Maintenance;

impl Maintenance {
    fn node_config(ctx: &Context) -> NodeConfig {
        let path = ctx
            .arg::<String>(NODE_CONFIG_PATH)
            .unwrap_or_else(|_| panic!("{} not found.", NODE_CONFIG_PATH));
        ConfigFile::load(path).expect("Can't load node config file")
    }

    fn database(ctx: &Context, options: &DbOptions) -> Box<dyn Database> {
        let path = ctx
            .arg::<String>(DATABASE_PATH)
            .unwrap_or_else(|_| panic!("{} not found.", DATABASE_PATH));
        Box::new(RocksDB::open(Path::new(&path), options).expect("Can't load database file"))
    }

    fn clear_cache(context: &Context) {
        info!("Clearing node cache");

        let config = Self::node_config(context);
        let db = Self::database(context, &config.database);
        let fork = db.fork();
        {
            let schema = Schema::new(&fork);
            schema.consensus_messages_cache_mut().clear();
        }
        db.merge_sync(fork.into_patch()).expect("Can't clear cache");

        info!("Cache cleared successfully");
    }
}

impl Command for Maintenance {
    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::new_named(
                NODE_CONFIG_PATH,
                true,
                "Path to node configuration file.",
                "c",
                "node-config",
                false,
            ),
            Argument::new_named(
                DATABASE_PATH,
                true,
                "Use database with the given path.",
                "d",
                "db-path",
                false,
            ),
            Argument::new_named(
                MAINTENANCE_ACTION_PATH,
                true,
                "Action to be performed during maintenance.",
                "a",
                "action",
                false,
            ),
        ]
    }

    fn name(&self) -> CommandName {
        "maintenance"
    }

    fn about(&self) -> &str {
        "Maintenance module. Available actions: clear-cache."
    }

    fn execute(
        &self,
        _commands: &HashMap<CommandName, CollectedCommand>,
        context: Context,
        _: &dyn Fn(Context) -> Context,
    ) -> Feedback {
        let action = context
            .arg::<String>(MAINTENANCE_ACTION_PATH)
            .unwrap_or_else(|_| panic!("{} not found.", MAINTENANCE_ACTION_PATH));

        if action == "clear-cache" {
            Self::clear_cache(&context);
        } else {
            println!("Unsupported maintenance action: {}", action);
        }

        Feedback::None
    }
}
