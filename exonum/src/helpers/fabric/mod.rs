use clap;
use toml::Value;
use serde::{Serialize, Deserialize};

use std::error::Error;
use std::collections::BTreeMap;

use blockchain::Service;

pub use self::builder::NodeBuilder;

pub type CommandName = &'static str;

#[derive(Clone, Copy, Debug)]
pub struct NamedArgument {
    pub short_name: &'static str,
    pub long_name: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub enum ArgumentType {
    Positional,
    Named(NamedArgument)
}

#[derive(Clone, Copy, Debug)]
pub struct Argument {
    pub name: &'static str,
    pub argument: ArgumentType,
    pub required: bool,
    pub help: &'static str,
}


#[derive(Debug, Clone, Default)]
pub struct Context {
    values: BTreeMap<&'static str, Value>
}

impl Context {

    fn new_from_args(args: &Vec<Argument>, matches: &clap::ArgMatches) -> Context {
        let mut context = Context::default();
        for arg in args {
            if let Some(value) = matches.value_of(&arg.name) {
                if context.values.insert(arg.name.clone(), value.to_string().into()).is_some() {
                    // TODO: replace by `unreachable!` 
                    // after making it unreachable ;)
                    panic!("Found args dupplicate, in args list.");
                }
            }
            else {
                if arg.required {
                    panic!("Argument {} not found.", arg.name)
                }
            }
        }
        context
    }

    pub fn get<'de, T: Deserialize<'de>>(&self, key: &str) -> Option<T> {
        self.values.get(key)
                   .expect("Expected Some in getting context.")
                   .clone()
                   .try_into()
                   .ok()
    }

    fn set<T: Serialize>(&mut self,
                         key: &'static str,
                         value: T) -> Result<Option<Value>, Box<Error>> {
        let value: Value = Value::try_from(value)?;
        Ok(self.values.insert(key, value))
    }

}

pub trait CommandExtension {
    fn args(&self) -> Vec<Argument>;
    fn execute(&self, context: Context) -> Result<Context, Box<Error>>;
}

pub trait ServiceFactory: 'static {
    //TODO: we could move 
    // `service_name` and `service_id` from `Service` trait into this one
    //fn name() -> &'static str;
    /// return `CommandExtension` for specific `CommandName`
    fn command(command: CommandName) -> Box<CommandExtension>;
    /// create new service, from context, returned by `run` command.
    fn make_service(self, run_context: &Context) -> Box<Service>;
}

mod builder;
mod details;
mod internal;
mod clap_backend;