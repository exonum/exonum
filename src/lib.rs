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

#![deny(missing_docs)]

//! Helper crate for secure and convenient configuration of the Exonum nodes.

use exonum::blockchain::InstanceCollection;
use exonum::exonum_merkledb::RocksDB;
use exonum::node::Node;
use exonum::runtime::rust::ServiceFactory;

use std::sync::Arc;

use crate::command::{Command, ExonumCommand, StandardResult};

pub mod command;
pub mod config;
pub mod io;
pub mod password;

/// Rust-runtime specific node builder used for constructing a node with a list
/// of provided services.
#[derive(Debug)]
pub struct NodeBuilder {
    services: Vec<Box<dyn ServiceFactory>>,
}

impl NodeBuilder {
    /// Creates new builder.
    pub fn new() -> Self {
        NodeBuilder {
            services: Default::default(),
        }
    }

    /// Adds new Rust service to the list of available services.
    pub fn with_service(mut self, service: impl Into<Box<dyn ServiceFactory>>) -> Self {
        self.services.push(service.into());
        self
    }

    /// Configures the node using parameters provided by user from stdin and then runs it.
    ///
    /// Rust runtime enabled only.
    pub fn run(self) -> Result<(), failure::Error> {
        let command = Command::from_args();
        if let StandardResult::Run(run_config) = command.execute()? {
            let database = Arc::new(RocksDB::open(
                run_config.db_path,
                &run_config.node_config.database,
            )?) as Arc<_>;
            let node = Node::new(
                database,
                self.services.into_iter().map(InstanceCollection::new),
                run_config.node_config,
                None,
            );
            node.run()
        } else {
            Ok(())
        }
    }
}
