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

//! Command line commands utilities.

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

/// Default port value.
pub const DEFAULT_EXONUM_LISTEN_PORT: u16 = 6333;

/// Name of the `Command`.
pub type CommandName = &'static str;

/// `Argument` with name.
#[derive(Clone, Copy, Debug)]
pub struct NamedArgument {
    /// Short argument name, for example `-a`.
    pub short_name: Option<&'static str>,
    /// Long argument name, for example `--long-arg`.
    pub long_name: &'static str,
    /// If `multiple` is true, then argument has more than one value.
    pub multiple: bool,
}

/// Possible types of the arguments.
#[derive(Clone, Copy, Debug)]
pub enum ArgumentType {
    /// Unnamed positional argument.
    Positional,
    /// Named argument.
    Named(NamedArgument),
}

/// Abstraction to represent arguments in command line.
#[derive(Clone, Copy, Debug)]
pub struct Argument {
    /// Name of the current argument. This name is used in `context.arg(name)`.
    pub name: &'static str,
    /// Explains how this argument is represented.
    pub argument_type: ArgumentType,
    /// Explains if the argument required or not.
    pub required: bool,
    /// Help message.
    pub help: &'static str,
}

impl Argument {
    /// Creates a new argument with `long` and optionally `short` names.
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

    /// Creates a new positional argument.
    pub fn new_positional(name: &'static str, required: bool, help: &'static str) -> Argument {
        Argument {
            argument_type: ArgumentType::Positional,
            name,
            help,
            required,
        }
    }
}

/// `Context` is a type, used to keep some values from `Command` into
/// `CommandExtension` and vice verse.
#[derive(PartialEq, Debug, Clone, Default)]
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

    /// Gets value of the command line argument.
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

    /// Gets multiple values of the command line argument.
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

    /// Gets the variable from the context.
    pub fn get<'de, T: Deserialize<'de>>(&self, key: &str) -> Result<T, Box<Error>> {
        if let Some(v) = self.variables.get(key) {
            Ok(v.clone().try_into()?)
        } else {
            Err(Box::new(NotFoundInMap))
        }
    }

    /// Sets the variable in the context and returns the previous value.
    ///
    /// # Panic
    ///
    /// Panics if value could not be serialized as TOML.
    pub fn set<T: Serialize>(&mut self, key: &'static str, value: T) -> Option<Value> {
        let value: Value = Value::try_from(value).expect("could not convert value into toml");
        self.variables.insert(key.to_owned(), value)
    }
}

/// `CommandExtension` is used for extending the existing commands.
pub trait CommandExtension {
    /// Returns arguments of the command.
    fn args(&self) -> Vec<Argument>;
    /// Executes command.
    fn execute(&self, context: Context) -> Result<Context, Box<Error>>;
}

/// Factory for service creation.
///
/// Services should provide implementation of this trait.
pub trait ServiceFactory: 'static {
    //TODO: we could move
    // `service_name` and `service_id` from `Service` trait into this one
    //fn name() -> &'static str;
    // ECR-76?

    /// Returns `CommandExtension` for the specific `CommandName`.
    #[allow(unused_variables)]
    fn command(&mut self, command: CommandName) -> Option<Box<CommandExtension>> {
        None
    }

    /// Creates a new service instance from the context returned by the `Run` command.
    fn make_service(&mut self, run_context: &Context) -> Box<Service>;
}
