/*
 * Copyright 2017 The Exonum Team
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
*/
use std::fmt;
use std::ffi::OsString;

use blockchain::{Service, Blockchain};
use node::{Node, NodeConfig};

use super::internal::{CollectedCommand, Feedback};
use super::clap_backend::ClapBackend;
use super::{Context, ServiceFactory};
use super::details::{Run, Finalize, GenerateNodeConfig, GenerateCommonConfig, GenerateTestnet};

/// `NodeBuilder` is a high level object,
/// usable for fast prototyping and creating app from services list.
#[derive(Default)]
pub struct NodeBuilder {
    commands: Vec<CollectedCommand>,
    service_constructors: Vec<Box<FnMut(&Context) -> Box<Service>>>,
}

impl NodeBuilder {
    /// creates new empty `NodeBuilder`
    pub fn new() -> NodeBuilder {
        NodeBuilder {
            commands: vec![
                CollectedCommand::new(Box::new(GenerateTestnet)),
                CollectedCommand::new(Box::new(Run)),
                CollectedCommand::new(Box::new(GenerateNodeConfig)),
                CollectedCommand::new(Box::new(GenerateCommonConfig)),
                CollectedCommand::new(Box::new(Finalize)),
            ],
            service_constructors: Vec::new(),
        }
    }

    /// append service to `NodeBuilder` context
    pub fn with_service<S: ServiceFactory>(mut self) -> NodeBuilder {
        //TODO: take endpoints, etc...

        for command in &mut self.commands {
            let name = command.name();
            command.extend(S::command(name))
        }
        self.service_constructors.push(Box::new(S::make_service));
        self
    }

    #[doc(hiden)]
    pub fn parse_cmd_string<I, T>(self, cmd_line: I) -> bool
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        ClapBackend::execute_cmd_string(self.commands.as_slice(), cmd_line) != Feedback::None

    }

    /// Parse cmd args, return `Node`, if run command found
    pub fn parse_cmd(self) -> Option<Node> {
        match ClapBackend::execute(self.commands.as_slice()) {
            Feedback::RunNode(ref ctx) => {
                let db = Run::db_helper(ctx);
                let config: NodeConfig =
                    ctx.get("node_config").expect("could not find node_config");
                let services: Vec<Box<Service>> = self.service_constructors
                    .into_iter()
                    .map(|mut constructor| constructor(ctx))
                    .collect();
                let blockchain = Blockchain::new(db, services);
                let node = Node::new(blockchain, config);
                Some(node)
            }
            _ => None,
        }
    }

    /// Run application
    pub fn run(self) {
        if let Some(mut node) = self.parse_cmd() {
            node.run().expect("Node return error")
        }
    }
}

impl fmt::Debug for NodeBuilder {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "NodeBuilder {{ commands: {:?}, services_count: {} }}",
            self.commands,
            self.service_constructors.len()
        )
    }
}
