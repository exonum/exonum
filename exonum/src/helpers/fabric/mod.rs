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

//! Command line commands utilities.

pub use self::{
    builder::NodeBuilder, context_key::ContextKey,
    details::{Finalize, GenerateCommonConfig, GenerateNodeConfig, GenerateTestnet, Run, RunDev},
    maintenance::Maintenance,
    shared::{AbstractConfig, CommonConfigTemplate, NodePrivateConfig, NodePublicConfig},
};

use clap;
use failure;
use serde::{Deserialize, Serialize};
use toml::Value;

use std::{collections::BTreeMap, str::FromStr};

use blockchain::Service;

mod builder;
mod clap_backend;
mod details;
mod info;
mod internal;
mod maintenance;
mod shared;
#[macro_use]
mod context_key;

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

/// Keys describing various pieces of data one can get from `Context`.
pub mod keys {
    use std::collections::BTreeMap;

    use toml;

    use super::shared::{AbstractConfig, CommonConfigTemplate, NodePublicConfig};
    use super::ContextKey;
    use node::NodeConfig;

    /// Configuration for this node.
    /// Set by `finalize` and `run` commands.
    pub const NODE_CONFIG: ContextKey<NodeConfig> = context_key!("node_config");

    /// Configurations for all nodes.
    /// Set by `generate-testnet` command.
    pub const CONFIGS: ContextKey<Vec<NodeConfig>> = context_key!("configs");

    /// Services configuration.
    /// Set by `generate-testnet` command.
    pub const SERVICES_CONFIG: ContextKey<AbstractConfig> = context_key!("services_config");

    /// Common configuration.
    /// Set by `generate-config` and `finalize` commands.
    pub const COMMON_CONFIG: ContextKey<CommonConfigTemplate> = context_key!("common_config");

    /// Services public configuration.
    /// Set by `generate-config` command.
    pub const SERVICES_PUBLIC_CONFIGS: ContextKey<BTreeMap<String, toml::Value>> =
        context_key!("services_public_configs");

    /// Services secret configuration.
    /// Set by `generate-config` command.
    pub const SERVICES_SECRET_CONFIGS: ContextKey<BTreeMap<String, toml::Value>> =
        context_key!("services_secret_configs");

    /// Public configurations for all nodes.
    /// Set by `finalize` command.
    pub const PUBLIC_CONFIG_LIST: ContextKey<Vec<NodePublicConfig>> =
        context_key!("public_config_list");

    /// Auditor mode.
    /// Set by `finalize` command.
    pub const AUDITOR_MODE: ContextKey<bool> = context_key!("auditor_mode");
}

/// `Context` is a type, used to keep some values from `Command` into
/// `CommandExtension` and vice verse.
/// To access values stored inside Context, use `ContextKey`.
///
/// # Examples
///
/// ```
/// use exonum::node::NodeConfig;
/// use exonum::helpers::fabric::{keys, Context};
///
/// fn get_node_config(context: &Context) -> NodeConfig {
///     context.get(keys::NODE_CONFIG).unwrap()
/// }
/// ```
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
    pub fn arg<T: FromStr>(&self, key: &str) -> Result<T, failure::Error>
    where
        failure::Error: From<<T as FromStr>::Err>,
    {
        match self.args.get(key) {
            Some(v) => Ok(v.parse()?),
            None => bail!("expected `{}` argument", key),
        }
    }

    /// Inserts value to the command line arguments map.
    pub fn set_arg(&mut self, key: &str, value: String) {
        self.args.insert(key.into(), value);
    }

    /// Gets multiple values of the command line argument.
    pub fn arg_multiple<T: FromStr>(&self, key: &str) -> Result<Vec<T>, failure::Error>
    where
        failure::Error: From<<T as FromStr>::Err>,
    {
        match self.multiple_args.get(key) {
            Some(values) => values.iter().map(|v| Ok(v.parse()?)).collect(),
            None => bail!("expected `{}` argument", key),
        }
    }

    /// Inserts multiple values to the command line arguments map.
    pub fn set_arg_multiple(&mut self, key: &str, values: Vec<String>) {
        self.multiple_args.insert(key.into(), values);
    }

    /// Gets the variable from the context.
    pub fn get<'de, T: Deserialize<'de>>(&self, key: ContextKey<T>) -> Result<T, failure::Error> {
        self.get_raw(key.name())
    }

    /// Sets the variable in the context and returns the previous value.
    ///
    /// # Panic
    ///
    /// Panics if value could not be serialized as TOML.
    pub fn set<T: Serialize>(&mut self, key: ContextKey<T>, value: T) -> Option<Value> {
        self.set_raw(key.name(), value)
    }

    fn get_raw<'de, T: Deserialize<'de>>(&self, key: &str) -> Result<T, failure::Error> {
        match self.variables.get(key) {
            Some(v) => Ok(v.clone().try_into()?),
            _ => bail!("key `{}` not found", key),
        }
    }

    fn set_raw<T: Serialize>(&mut self, key: &str, value: T) -> Option<Value> {
        let value: Value = Value::try_from(value).expect("could not convert value into toml");
        self.variables.insert(key.to_owned(), value)
    }
}

/// `CommandExtension` is used for extending the existing commands.
pub trait CommandExtension {
    /// Returns arguments of the command.
    fn args(&self) -> Vec<Argument>;
    /// Executes command.
    fn execute(&self, context: Context) -> Result<Context, failure::Error>;
}

/// Factory for service creation.
///
/// Services should provide implementation of this trait.
pub trait ServiceFactory: 'static {
    /// Returns name of the service.
    fn service_name(&self) -> &str;
    /// Returns `CommandExtension` for the specific `CommandName`.
    #[allow(unused_variables)]
    fn command(&mut self, command: CommandName) -> Option<Box<CommandExtension>> {
        None
    }

    /// Creates a new service instance from the context returned by the `Run` command.
    fn make_service(&mut self, run_context: &Context) -> Box<Service>;
}
