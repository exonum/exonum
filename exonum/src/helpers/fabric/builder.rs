// Copyright 2017 The Exonum Team
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

use std::fmt;
use std::panic::{self, PanicInfo};
use std::ffi::OsString;

use blockchain::{Service, Blockchain};
use node::{NodeConfig, Node};

use super::internal::{CollectedCommand, Feedback};
use super::clap_backend::ClapBackend;
use super::ServiceFactory;
use super::details::{Run, Finalize, GenerateNodeConfig, GenerateCommonConfig, GenerateTestnet};

/// `NodeBuilder` is a high level object,
/// usable for fast prototyping and creating app from services list.
#[derive(Default)]
pub struct NodeBuilder {
    commands: Vec<CollectedCommand>,
    service_factories: Vec<Box<ServiceFactory>>,
}

impl NodeBuilder {
    /// Creates a new empty `NodeBuilder`.
    pub fn new() -> Self {
        NodeBuilder {
            commands: vec![
                CollectedCommand::new(Box::new(GenerateTestnet)),
                CollectedCommand::new(Box::new(Run)),
                CollectedCommand::new(Box::new(GenerateNodeConfig)),
                CollectedCommand::new(Box::new(GenerateCommonConfig)),
                CollectedCommand::new(Box::new(Finalize)),
            ],
            service_factories: Vec::new(),
        }
    }

    /// Appends service to the `NodeBuilder` context.
    pub fn with_service(mut self, mut factory: Box<ServiceFactory>) -> NodeBuilder {
        //TODO: take endpoints, etc... (ECR-164)

        for command in &mut self.commands {
            let name = command.name();
            command.extend(factory.command(name))
        }
        self.service_factories.push(factory);
        self
    }

    #[doc(hidden)]
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
                let services: Vec<Box<Service>> = self.service_factories
                    .into_iter()
                    .map(|mut factory| factory.make_service(ctx))
                    .collect();
                let blockchain = Blockchain::new(db, services);
                let node = Node::new(blockchain, config);
                Some(node)
            }
            _ => None,
        }
    }

    // handle error, and print it.
    fn panic_hook(info: &PanicInfo) {
        let msg = match info.payload().downcast_ref::<&'static str>() {
            Some(s) => *s,
            None => {
                match info.payload().downcast_ref::<String>() {
                    Some(s) => &s[..],
                    None => "Box<Any>",
                }
            }
        };
        println!("error: {}", msg);
    }

    /// Runs application.
    pub fn run(self) {
        let old_hook = panic::take_hook();
        panic::set_hook(Box::new(Self::panic_hook));
        let feedback = self.parse_cmd();
        panic::set_hook(old_hook);

        if let Some(node) = feedback {
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
            self.service_factories.len()
        )
    }
}
