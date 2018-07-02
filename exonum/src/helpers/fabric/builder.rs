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

use std::{
    collections::HashMap, ffi::OsString, fmt, panic::{self, PanicInfo},
};

use super::{
    clap_backend::ClapBackend,
    details::{Finalize, GenerateCommonConfig, GenerateNodeConfig, GenerateTestnet, Run, RunDev},
    info::Info, internal::{CollectedCommand, Command, Feedback}, keys, maintenance::Maintenance,
    CommandName, ServiceFactory,
};
use blockchain::Service;
use node::Node;

/// `NodeBuilder` is a high level object,
/// usable for fast prototyping and creating app from services list.
#[derive(Default)]
pub struct NodeBuilder {
    commands: HashMap<CommandName, CollectedCommand>,
    service_factories: Vec<Box<ServiceFactory>>,
}

impl NodeBuilder {
    /// Creates a new empty `NodeBuilder`.
    pub fn new() -> Self {
        NodeBuilder {
            commands: Self::commands(),
            service_factories: Vec::new(),
        }
    }

    /// Appends service to the `NodeBuilder` context.
    pub fn with_service(mut self, mut factory: Box<ServiceFactory>) -> NodeBuilder {
        //TODO: Take endpoints, etc... (ECR-164)

        for (name, command) in &mut self.commands {
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
        let feedback = ClapBackend::execute_cmd_string(&self.commands, cmd_line);
        feedback != Feedback::None
    }

    /// Parse cmd args, return `Node`, if run command found
    pub fn parse_cmd(self) -> Option<Node> {
        match ClapBackend::execute(&self.commands) {
            Feedback::RunNode(ref ctx) => {
                let config = ctx.get(keys::NODE_CONFIG)
                    .expect("could not find node_config");
                let db = Run::db_helper(ctx, &config.database);
                let services: Vec<Box<Service>> = self.service_factories
                    .into_iter()
                    .map(|mut factory| factory.make_service(ctx))
                    .collect();
                let node = Node::new(db, services, config);
                Some(node)
            }
            _ => None,
        }
    }

    // handle error, and print it.
    fn panic_hook(info: &PanicInfo) {
        let msg = match info.payload().downcast_ref::<&'static str>() {
            Some(s) => *s,
            None => match info.payload().downcast_ref::<String>() {
                Some(s) => &s[..],
                None => "Box<Any>",
            },
        };
        println!("error: {}", msg);
    }

    /// Runs application.
    pub fn run(mut self) {
        // This should be moved into `commands` method, but services list can be obtained only here.
        {
            let services: Vec<_> = self.service_factories
                .iter()
                .map(|f| f.service_name().to_owned())
                .collect();
            let info: Box<Command> = Box::new(Info::new(services));
            self.commands
                .insert(info.name(), CollectedCommand::new(info));
        }

        let old_hook = panic::take_hook();
        panic::set_hook(Box::new(Self::panic_hook));
        let feedback = self.parse_cmd();
        panic::set_hook(old_hook);

        if let Some(node) = feedback {
            node.run().expect("Node return error")
        }
    }

    fn commands() -> HashMap<CommandName, CollectedCommand> {
        vec![
            Box::new(GenerateTestnet) as Box<Command>,
            Box::new(Run),
            Box::new(RunDev),
            Box::new(GenerateNodeConfig),
            Box::new(GenerateCommonConfig),
            Box::new(Finalize),
            Box::new(Maintenance),
        ].into_iter()
            .map(|c| (c.name(), CollectedCommand::new(c)))
            .collect()
    }
}

impl fmt::Debug for NodeBuilder {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "NodeBuilder {{ commands: {:?}, services_count: {} }}",
            self.commands.values(),
            self.service_factories.len()
        )
    }
}
