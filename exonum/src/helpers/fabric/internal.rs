use super::{Context, CommandName, Argument, CommandExtension};

use std::fmt;
use std::error::Error;

/// Used to take some additional information from executed command
#[derive(Debug, PartialEq, Clone)]
pub enum Feedback {
    /// Run node with current context.
    RunNode(Context),
    /// Do nothing
    None,
}
#[derive(Clone, Debug, Copy)]
pub struct NotFoundInMap;

impl Error for NotFoundInMap {
    fn description(&self) -> &str {
        "Expected Some in getting context."
    }
}

impl fmt::Display for NotFoundInMap {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

/// `Command` trait useable to create subcommand for `NodeBuilder`
pub trait Command {
    fn args(&self) -> Vec<Argument>;
    fn name(&self) -> CommandName ;
    fn about(&self) -> &str;
    fn execute(&self,
               context: Context,
               extension: &Fn(Context) -> Context) -> Feedback;
}

/// We keep command internal state into `CollectedCommand`
/// motivation:
///
/// 1. `Command` by its nature should be stateless, but it's harder to make
/// abstracted dynamic object without trait objects.
/// 2. Additinaly this state is common for all commands.
pub struct CollectedCommand {
    command: Box<Command>,
    args: Vec<Argument>,
    exts: Vec<Box<CommandExtension>>
}

impl CollectedCommand {
    pub fn new(command: Box<Command>) -> CollectedCommand {
        CollectedCommand {
            args: command.args(),
            command: command,
            exts: Vec::new()
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

    pub fn extend(&mut self, extender: Option<Box<CommandExtension>>) {
        if let Some(extender) = extender {
            let args = extender.args();
            self.args.extend(args.into_iter());
            self.exts.push(extender);
        }
    }

    pub fn execute(&self, context: Context) -> Feedback {
        self.command.execute(context, &|context| {
            // TODO: check duplicates, in services context keys
            let mut new_context = context.clone();
            for ext in &self.exts {
                new_context = ext.execute(new_context)
                                     .expect("Could not execute extension.");
            };
            new_context
        })
    }
}


impl fmt::Debug for CollectedCommand {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "CollectedCommand {{ args: {:?}, ext_count: {} }}",
            self.args,
            self.exts.len()
        )
    }
}
