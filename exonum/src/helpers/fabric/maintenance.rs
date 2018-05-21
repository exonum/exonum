// Copyright 2018 The Exonum Team
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

#![allow(missing_debug_implementations)]

//! This module implements node maintenance actions.
// spell-checker:ignore exts

use std::path::Path;
use std::collections::HashMap;

use blockchain::Schema;
use helpers::config::ConfigFile;
use storage::{Database, DbOptions, RocksDB};
use node::NodeConfig;
use super::internal::{CollectedCommand, Command, Feedback};
use super::{Argument, CommandName, Context};

// Context entry for the path to the node config.
const NODE_CONFIG_PATH: &str = "NODE_CONFIG_PATH";
// Context entry for the path to the database.
const DATABASE_PATH: &str = "DATABASE_PATH";
// Context entry for the type of action to be performed.
const MAINTENANCE_ACTION_PATH: &str = "MAINTENANCE_ACTION_PATH";

/// Maintenance command. Supported actions:
///
/// - `clear-cache` - clear message cache.
pub struct Maintenance;

impl Maintenance {
    /// Returns the name of the `Maintenance` command.
    pub fn name() -> CommandName {
        "maintenance"
    }

    fn node_config(ctx: &Context) -> NodeConfig {
        let path = ctx.arg::<String>(NODE_CONFIG_PATH)
            .expect(&format!("{} not found.", NODE_CONFIG_PATH));
        ConfigFile::load(path).expect("Can't load node config file")
    }

    fn database(ctx: &Context, options: &DbOptions) -> Box<Database> {
        let path = ctx.arg::<String>(DATABASE_PATH)
            .expect(&format!("{} not found.", DATABASE_PATH));
        Box::new(RocksDB::open(Path::new(&path), options).expect("Can't load database file"))
    }

    fn clear_cache(context: &Context) {
        info!("Clearing node cache");

        let config = Self::node_config(context);
        let db = Self::database(context, &config.database);
        let mut fork = db.fork();
        {
            let mut schema = Schema::new(&mut fork);
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
        Self::name()
    }

    fn about(&self) -> &str {
        "Maintenance module. Available actions: clear-cache."
    }

    fn execute(
        &self,
        _commands: &HashMap<CommandName, CollectedCommand>,
        context: Context,
        _exts: &Fn(Context) -> Context,
    ) -> Feedback {
        let action = context
            .arg::<String>(MAINTENANCE_ACTION_PATH)
            .expect(&format!("{} not found.", MAINTENANCE_ACTION_PATH));

        match action.as_ref() {
            "clear-cache" => Self::clear_cache(&context),
            _ => println!("Unsupported maintenance action: {}", action),
        }

        Feedback::None
    }
}
