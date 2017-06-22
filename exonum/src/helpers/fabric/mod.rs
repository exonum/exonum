use clap;
use toml::Value;
use serde::{Serialize, Deserialize};

use std::error::Error;
use std::collections::BTreeMap;

use blockchain::Service;

pub use self::builder::NodeBuilder;

/// `Command` `name` type
pub type CommandName = &'static str;

#[derive(Clone, Copy, Debug)]
/// `Argument` with name helper structure
pub struct NamedArgument {
    pub short_name: Option<&'static str>,
    pub long_name: &'static str,
}

#[derive(Clone, Copy, Debug)]
/// Possible types of argument
pub enum ArgumentType {
    /// argument without name, index based
    Positional,
    /// argument with `long` and optionally `short` name
    Named(NamedArgument)
}

#[derive(Clone, Copy, Debug)]
/// Abstraction to represent arguments in command line
pub struct Argument {
    pub name: &'static str,
    pub argument: ArgumentType,
    pub required: bool,
    pub help: &'static str,
}

impl Argument {

    /// Create new argument with `long` and optionally `short` names.
    pub fn new_named<T>(name: &'static str,
                    required: bool,
                    help: &'static str,
                    short_name: T,
                    long_name: &'static str) -> Argument
    where T: Into<Option<&'static str>>
    {
        Argument {
            argument: ArgumentType::Named (
                NamedArgument { 
                    short_name: short_name.into(),
                    long_name
                }
            ),
            name, help, required,
            
        }
    }

    /// Create new positional argument.
    pub fn new_positional(name: &'static str,
                    required: bool,
                    help: &'static str) -> Argument
    {
        Argument {
            argument: ArgumentType::Positional,
            name, help, required,
            
        }
    }
}


#[derive(Debug, Clone, Default)]
/// `Context` is a type, used to keep some values from `Command` into
/// `CommandExtension` and vice verse.
pub struct Context {
    values: BTreeMap<&'static str, Value>
}

impl Context {

    fn new_from_args(args: &Vec<Argument>, matches: &clap::ArgMatches) -> Context {
        let mut context = Context::default();
        for arg in args {
            if let Some(value) = matches.value_of(&arg.name) {
                println!("value with name {}, found {}", arg.name, value);
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

    /// Get value from context.
    /// Warning: values from command line are parsed as string,
    /// and can't be converted directly into int, because of `toml`
    /// parsing specifics. Use `context.get<String>(key)?.parse()` instead.
    pub fn get<'de, T: Deserialize<'de>>(&self, key: &str) -> Result<T, Box<Error>> {
        Ok(self.values.get(key)
                   .map_or_else(
                        | | Err(::serde::de::Error::custom("Expected Some in getting context.")),
                        |v| v.clone()
                            .try_into()
                   )?)
                   
                   
    }

    /// write some value into context
    pub fn set<T: Serialize>(&mut self,
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