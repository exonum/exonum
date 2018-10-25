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

// spell-checker:ignore exts

use std::{collections::HashMap, fmt};

use super::{Argument, CommandExtension, CommandName, Context};

/// Used to take some additional information from executed command
#[derive(Debug, PartialEq, Clone)]
pub enum Feedback {
    /// Run node with current context.
    RunNode(Context),
    /// Do nothing
    None,
}

/// `Command` trait is used to create sub-command for `NodeBuilder`.
pub trait Command {
    /// List of command arguments.
    fn args(&self) -> Vec<Argument>;
    /// Command name.
    fn name(&self) -> CommandName;
    /// Short information about the command.
    fn about(&self) -> &str;
    /// Executes the command processing code.
    ///
    /// # Arguments:
    ///
    /// * `commands` - contains set of commands that can be performed on this node.
    /// * `context` - contains global execution context.
    /// * `exts` - allows to execute `CommandExtensions` for the command.
    fn execute(
        &self,
        commands: &HashMap<CommandName, CollectedCommand>,
        context: Context,
        exts: &dyn Fn(Context) -> Context,
    ) -> Feedback;
}

/// We keep command internal state into `CollectedCommand`
/// motivation:
///
/// 1. `Command` by its nature should be stateless, but it's harder to make
/// abstracted dynamic object without trait objects.
/// 2. Additionally this state is common for all commands.
pub struct CollectedCommand {
    command: Box<dyn Command>,
    args: Vec<Argument>,
    exts: Vec<Box<dyn CommandExtension>>,
}

impl CollectedCommand {
    pub fn new(command: Box<dyn Command>) -> Self {
        Self {
            args: command.args(),
            command,
            exts: Vec::new(),
        }
    }

    pub fn args(&self) -> &Vec<Argument> {
        &self.args
    }

    pub fn name(&self) -> CommandName {
        self.command.name()
    }

    pub fn about(&self) -> &str {
        self.command.about()
    }

    pub fn extend(&mut self, extender: Option<Box<dyn CommandExtension>>) {
        if let Some(extender) = extender {
            let args = extender.args();
            self.args.extend(args.into_iter());
            self.exts.push(extender);
        }
    }

    pub fn execute(&self, commands: &HashMap<CommandName, Self>, context: Context) -> Feedback {
        self.command.execute(commands, context, &|context| {
            // TODO: Check duplicates, in services context keys. (ECR-164)
            let mut new_context = context.clone();
            for ext in &self.exts {
                new_context = ext
                    .execute(new_context)
                    .expect("Could not execute extension.");
            }
            new_context
        })
    }
}

impl fmt::Debug for CollectedCommand {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "CollectedCommand {{ args: {:?}, ext_count: {} }}",
            self.args,
            self.exts.len()
        )
    }
}
