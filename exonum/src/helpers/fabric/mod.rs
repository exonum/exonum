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

use clap;
use toml::Value;
use serde::{Serialize, Deserialize};

use std::str::FromStr;
use std::error::Error;
use std::collections::BTreeMap;

use blockchain::Service;
use self::internal::NotFoundInMap;

pub use self::builder::NodeBuilder;
pub use self::details::{Run, Finalize, GenerateNodeConfig, GenerateCommonConfig, GenerateTestnet};
pub use self::shared::{AbstractConfig, NodePublicConfig, CommonConfigTemplate, NodePrivateConfig};

mod shared;
mod builder;
mod details;
mod internal;
mod clap_backend;

pub const DEFAULT_EXONUM_LISTEN_PORT: u16 = 6333;

/// `Command` name type
pub type CommandName = &'static str;

#[derive(Clone, Copy, Debug)]
/// `Argument` with name helper structure
pub struct NamedArgument {
    /// Short argument name, for example `-a`.
    pub short_name: Option<&'static str>,
    /// Long argument name, for example `--long-arg`.
    pub long_name: &'static str,
    pub multiple: bool,
}

#[derive(Clone, Copy, Debug)]
/// Possible types of argument
pub enum ArgumentType {
    /// Unnamed positional argument.
    Positional,
    /// argument with `long` and optionally `short` name
    Named(NamedArgument),
}

#[derive(Clone, Copy, Debug)]
/// Abstraction to represent arguments in command line
pub struct Argument {
    /// Name of the current argument.
    /// This name is used in `context.arg(name)`.
    pub name: &'static str,
    /// Explains how this argument is represented.
    pub argument_type: ArgumentType,
    /// Explains if the argument required or not.
    pub required: bool,
    /// Help string.
    pub help: &'static str,
}

impl Argument {
    /// Create new argument with `long` and optionally `short` names.
    pub fn new_named<T>(
        name: &'static str,
        required: bool,
        help: &'static str,
        short_name: T,
        long_name: &'static str,
        multiple: bool,
    ) -> Argument
    where
        T: Into<Option<&'static str>>,
    {
        Argument {
            argument_type: ArgumentType::Named(NamedArgument {
                short_name: short_name.into(),
                long_name,
                multiple,
            }),
            name,
            help,
            required,
        }
    }

    /// Create new positional argument.
    pub fn new_positional(name: &'static str, required: bool, help: &'static str) -> Argument {
        Argument {
            argument_type: ArgumentType::Positional,
            name,
            help,
            required,
        }
    }
}

#[derive(PartialEq, Debug, Clone, Default)]
/// `Context` is a type, used to keep some values from `Command` into
/// `CommandExtension` and vice verse.
pub struct Context {
    args: BTreeMap<String, String>,
    multiple_args: BTreeMap<String, Vec<String>>,
    variables: BTreeMap<String, Value>,
}

impl Context {
    fn new_from_args(args: &[Argument], matches: &clap::ArgMatches) -> Context {
        let mut context = Context::default();
        for arg in args {
            // processing multiple value arguments make code ugly =(
            match arg.argument_type {
                ArgumentType::Named(detail) if detail.multiple => {
                    if let Some(values) = matches.values_of(&arg.name) {
                        let values: Vec<String> = values.map(|e| e.to_owned()).collect();
                        if context
                            .multiple_args
                            .insert(arg.name.to_owned(), values)
                            .is_some()
                        {
                            panic!("Duplicated argument: {}", arg.name);
                        }
                        continue;
                    }
                }
                _ => (),
            };

            if let Some(value) = matches.value_of(&arg.name) {
                if context
                    .args
                    .insert(arg.name.to_owned(), value.to_string())
                    .is_some()
                {
                    panic!("Duplicated argument: {}", arg.name);
                }


            } else if arg.required {
                panic!("Required argument is not found: {}", arg.name)
            }
        }
        context
    }

    /// Get cmd argument value
    pub fn arg<T: FromStr>(&self, key: &str) -> Result<T, Box<Error>>
    where
        <T as FromStr>::Err: Error + 'static,
    {
        if let Some(v) = self.args.get(key) {
            Ok(v.parse()?)
        } else {
            Err(Box::new(NotFoundInMap))
        }
    }

    /// Get cmd argument multiple values
    pub fn arg_multiple<T: FromStr>(&self, key: &str) -> Result<Vec<T>, Box<Error>>
    where
        <T as FromStr>::Err: Error + 'static,
    {
        if let Some(values) = self.multiple_args.get(key) {
            values.iter().map(|v| Ok(v.parse()?)).collect()
        } else {
            Err(Box::new(NotFoundInMap))
        }
    }

    /// Get variable from context.
    pub fn get<'de, T: Deserialize<'de>>(&self, key: &str) -> Result<T, Box<Error>> {
        if let Some(v) = self.variables.get(key) {
            Ok(v.clone().try_into()?)
        } else {
            Err(Box::new(NotFoundInMap))
        }
    }

    /// Sets the variable in the context and returns the previous value.
    /// ## Panic:
    /// if value could not be serialized as `toml`
    pub fn set<T: Serialize>(&mut self, key: &'static str, value: T) -> Option<Value> {
        let value: Value = Value::try_from(value).expect("could not convert value into toml");
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
    fn make_service(run_context: &Context) -> Box<Service>;
}
