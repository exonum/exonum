use super::{Context, CommandName, Argument, CommandExtension};

use std::fmt;

//TODO: we could extend current feedback
/// Used to take some additional information from executed command
pub enum Feedback {
    RunNode(Context),
    None,
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
/// 2. Additinal this state is common for all commands.
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

    pub fn extend(&mut self, extender: Box<CommandExtension>) {
        let args = extender.args();
        self.args.extend(args.into_iter());
        self.exts.push(extender);
    }

    pub fn execute(&self, context: Context) -> Feedback {
        self.command.execute(context, &|context| {
            let mut new_context = context.clone();
            for ext in &self.exts {
                let local_context = context.clone();
                let out_context = ext.execute(local_context)
                                     .expect("Could not execute extension.");
                // TODO: check duplicates
                new_context.values.extend(out_context.values.into_iter());
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