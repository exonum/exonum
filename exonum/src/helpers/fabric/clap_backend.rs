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

use std::ffi::OsString;

use super::{Context, ArgumentType};
use super::internal::{Feedback, CollectedCommand};

pub struct ClapBackend;

impl ClapBackend {
    // TODO: remove code duplication (ECR-164)
    #[doc(hidden)]
    pub fn execute_cmd_string<I, T>(commands: &[CollectedCommand], line: I) -> Feedback
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        let subcommands: Vec<_> = commands
            .iter()
            .map(|command| ClapBackend::command_into_subcommand(command))
            .collect();
        let matches = clap::App::new("Exonum application based on fabric configuration.")
            .version(crate_version!())
            .author(crate_authors!("\n"))
            .about(
                "It contain basic set of command, to deploy network on exonum.",
            )
            .subcommands(subcommands.into_iter())
            .get_matches_from_safe(line)
            .unwrap();

        let subcommand = matches.subcommand();
        for command in commands {
            if command.name() == subcommand.0 {
                return command.execute(Context::new_from_args(
                    command.args(),
                    subcommand.1.expect("Arguments not found."),
                ));
            }
        }

        panic!("Subcommand not found");
    }

    pub fn execute(commands: &[CollectedCommand]) -> Feedback {
        let subcommands: Vec<_> = commands
            .iter()
            .map(|command| ClapBackend::command_into_subcommand(command))
            .collect();

        let matches = clap::App::new("Exonum application based on fabric configuration.")
            .version(crate_version!())
            .author(crate_authors!("\n"))
            .about("Exonum application based on fabric configuration.")
            .subcommands(subcommands.into_iter())
            .get_matches();

        let subcommand = matches.subcommand();
        for command in commands {
            if command.name() == subcommand.0 {
                return command.execute(Context::new_from_args(
                    command.args(),
                    subcommand.1.expect("Arguments not found."),
                ));
            }
        }

        panic!("Subcommand not found");
    }

    fn command_into_subcommand(command: &CollectedCommand) -> clap::App {
        let mut index = 1;
        let command_args: Vec<_> = command
            .args()
            .iter()
            .map(|arg| {
                let clap_arg = clap::Arg::with_name(arg.name);
                let clap_arg = match arg.argument_type {
                    ArgumentType::Positional => {
                        let arg = clap_arg.index(index);
                        index += 1;
                        arg
                    }
                    ArgumentType::Named(detail) => {
                        let mut clap_arg = clap_arg.long(detail.long_name);
                        if let Some(short) = detail.short_name {
                            clap_arg = clap_arg.short(short);
                        }
                        clap_arg.multiple(detail.multiple).takes_value(true)
                    }
                };
                clap_arg.help(arg.help).required(arg.required)

            })
            .collect();

        let mut subcommand = clap::SubCommand::with_name(command.name()).about(command.about());

        subcommand = subcommand.args(&command_args);

        subcommand
    }
}
