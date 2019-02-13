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

use std::{
    collections::HashMap,
    ffi::OsString,
    fmt,
    panic::{self, PanicInfo},
    str::FromStr,
};

use super::{
    clap_backend::ClapBackend,
    details::{Finalize, GenerateCommonConfig, GenerateNodeConfig, GenerateTestnet, Run, RunDev},
    info::Info,
    internal::{CollectedCommand, Command, Feedback},
    keys,
    maintenance::Maintenance,
    password::{PassInputMethod, SecretKeyType},
    CommandName, Context, ServiceFactory,
};

use crate::blockchain::Service;
use crate::node::{ExternalMessage, Node};

/// `NodeBuilder` is a high level object,
/// usable for fast prototyping and creating app from services list.
#[derive(Default)]
pub struct NodeBuilder {
    commands: HashMap<CommandName, CollectedCommand>,
    service_factories: Vec<Box<dyn ServiceFactory>>,
}

impl NodeBuilder {
    /// Creates a new empty `NodeBuilder`.
    pub fn new() -> Self {
        Self {
            commands: Self::commands(),
            service_factories: Vec::new(),
        }
    }

    /// Appends service to the `NodeBuilder` context.
    pub fn with_service(mut self, mut factory: Box<dyn ServiceFactory>) -> Self {
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
        if let Feedback::RunNode(ref ctx) = feedback {
            self.node_from_run_context(ctx);
        }
        feedback != Feedback::None
    }

    /// Parse cmd args, return `Node`, if run command found
    pub fn parse_cmd(self) -> Option<Node> {
        match ClapBackend::execute(&self.commands) {
            Feedback::RunNode(ref ctx) => {
                let node = self.node_from_run_context(ctx);
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
            let services: Vec<_> = self
                .service_factories
                .iter()
                .map(|f| f.service_name().to_owned())
                .collect();
            let info: Box<dyn Command> = Box::new(Info::new(services));
            self.commands
                .insert(info.name(), CollectedCommand::new(info));
        }

        let old_hook = panic::take_hook();
        panic::set_hook(Box::new(Self::panic_hook));
        let feedback = self.parse_cmd();
        panic::set_hook(old_hook);

        if let Some(node) = feedback {
            let channel = node.channel();
            ctrlc::set_handler(move || {
                println!("Shutting down...");
                let _ = channel.send_external_message(ExternalMessage::Shutdown);
            })
            .expect("Cannot set CTRL+C handler");

            node.run().expect("Node return error")
        }
    }

    fn commands() -> HashMap<CommandName, CollectedCommand> {
        vec![
            Box::new(GenerateTestnet) as Box<dyn Command>,
            Box::new(Run),
            Box::new(RunDev),
            Box::new(GenerateNodeConfig),
            Box::new(GenerateCommonConfig),
            Box::new(Finalize),
            Box::new(Maintenance),
        ]
        .into_iter()
        .map(|c| (c.name(), CollectedCommand::new(c)))
        .collect()
    }

    fn node_from_run_context(self, ctx: &Context) -> Node {
        let config_file_path = ctx
            .get(keys::NODE_CONFIG_PATH)
            .expect("Could not find node_config_path");
        let config = ctx
            .get(keys::NODE_CONFIG)
            .expect("could not find node_config");
        let db = Run::db_helper(ctx, &config.database);
        let services: Vec<Box<dyn Service>> = self
            .service_factories
            .into_iter()
            .map(|mut factory| factory.make_service(ctx))
            .collect();

        let config = {
            let run_config = ctx.get(keys::RUN_CONFIG).unwrap();
            let consensus_passphrase = PassInputMethod::from_str(&run_config.consensus_pass_method)
                .expect("Incorrect passphrase input method for consensus key.")
                .get_passphrase(SecretKeyType::Consensus, true);
            let service_passphrase = PassInputMethod::from_str(&run_config.service_pass_method)
                .expect("Incorrect passphrase input method for service key.")
                .get_passphrase(SecretKeyType::Service, true);

            config.read_secret_keys(
                &config_file_path,
                consensus_passphrase.as_bytes(),
                service_passphrase.as_bytes(),
            )
        };
        Node::new(db, services, config, Some(config_file_path))
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
