use clap;
use toml::Value;
use serde::{Serialize, Deserialize};

use std::str::FromStr;
use std::error::Error;
use std::collections::BTreeMap;

use blockchain::Service;
use self::internal::NotFoundInMap;



pub use self::builder::NodeBuilder;
pub use self::details::{RunCommand, AddValidatorCommand,
                    KeyGeneratorCommand, InitCommand,
                    GenerateTestnetCommand, GenerateTemplateCommand };

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
    args: BTreeMap<String, String>,
    variables: BTreeMap<String, Value>,
}

impl Context {

    fn new_from_args(args: &Vec<Argument>, matches: &clap::ArgMatches) -> Context {
        let mut context = Context::default();
        for arg in args {
            if let Some(value) = matches.value_of(&arg.name) {
                if context.args.insert(arg.name.to_owned(), value.to_string()).is_some() {
                    // TODO: replace by `unreachable!`
                    // after moving this check into `CollectedCommand`
                    panic!("Found args dupplicate, in args list.");
                }
            }
            else if arg.required {
                panic!("Argument {} not found.", arg.name)
            }
        }
        context
    }

    /// Get cmd argument value
    pub fn arg<T: FromStr>(&self, key: &str) -> Result<T, Box<Error>>
        where <T as FromStr>::Err: Error + 'static
    {
        if let Some(v) = self.args.get(key) {
            Ok(v.parse()?)
        }
        else{
            Err(Box::new(NotFoundInMap))
        }
    }

    /// Get variable from context.
    pub fn get<'de, T: Deserialize<'de>>(&self, key: &str) -> Result<T, Box<Error>> {
        if let Some(v) = self.variables.get(key) {
            Ok(v.clone().try_into()?)
        }
        else {
            Err(Box::new(NotFoundInMap))
        }
    }

    /// Set variable in context, return pervios value
    /// ## Panic:
    /// if value could not be serialized as `toml`
    pub fn set<T: Serialize>(&mut self,
                         key: &'static str,
                         value: T) -> Option<Value> {
        let value: Value = Value::try_from(value)
                            .expect("could not convert value into toml");
        self.variables.insert(key.to_owned(), value)
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
    #[allow(unused_variables)]
    fn command(command: CommandName) -> Option<Box<CommandExtension>> {
        None
    }
    /// create new service, from context, returned by `run` command.
    fn make_service( run_context: &Context) -> Box<Service>;
}

mod builder;
mod details;
mod internal;
mod clap_backend;
