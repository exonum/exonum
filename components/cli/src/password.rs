// Copyright 2019 The Exonum Team
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

//! This module contains utilities for passphrase entry.

use failure::{bail, Error, ResultExt};
use rpassword::read_password_from_tty;
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

use std::ops::{Deref, DerefMut};
use std::{env, str::FromStr};

/// Default name of the environment variable with a consensus key passphrase.
pub const DEFAULT_CONSENSUS_PASS_ENV_VAR: &str = "EXONUM_CONSENSUS_PASS";
/// Default name of the environment variable with a service key passphrase.
pub const DEFAULT_SERVICE_PASS_ENV_VAR: &str = "EXONUM_SERVICE_PASS";

/// A wrapper around `String` which securely erases itself on drop.
#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Passphrase(pub String);

impl Drop for Passphrase {
    fn drop(&mut self) {
        self.0.zeroize()
    }
}

impl Deref for Passphrase {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Passphrase {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Passphrase {
    /// Read the passphrase from stdin.
    pub fn read_from_tty(prompt: &str) -> Result<Self, Error> {
        Ok(Self(read_password_from_tty(Some(prompt))?))
    }
}

/// Passphrase input method.
///
/// Defaults to `Terminal`.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum PassInputMethod {
    /// Prompt passphrase from terminal.
    Terminal,
    /// Get passphrase from the environment variable with given name.
    ///
    /// Default values are `EXONUM_CONSENSUS_PASS` and `EXONUM_SERVICE_PASS` for
    /// consensus and service secret keys correspondingly.
    /// Defaults are used if `None` is provided.
    EnvVariable(Option<String>),
    /// Passphrase is passed as a command line parameter.
    CmdLineParameter(Passphrase),
}

impl Default for PassInputMethod {
    fn default() -> Self {
        PassInputMethod::Terminal
    }
}

/// Secret key types.
#[derive(Copy, Clone)]
pub enum SecretKeyType {
    /// Consensus key. Used in communication between nodes during consensus.
    Consensus,
    /// Service key. Used to sign transactions produced by the node.
    Service,
}

impl SecretKeyType {
    /// Returns default environment variable names for the corresponding `SecretKeyType`.
    pub fn default_env_var(self) -> &'static str {
        match self {
            SecretKeyType::Consensus => DEFAULT_CONSENSUS_PASS_ENV_VAR,
            SecretKeyType::Service => DEFAULT_SERVICE_PASS_ENV_VAR,
        }
    }

    /// Returns prompt messages for the corresponding `SecretKeyType`.
    pub fn prompt_message(self) -> &'static str {
        match self {
            SecretKeyType::Consensus => "Enter consensus key passphrase",
            SecretKeyType::Service => "Enter service key passphrase",
        }
    }
}

/// Determines the usage of the passphrase received from user.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PassphraseUsage {
    /// The user will be asked to enter the sane passphrase twice. Empty passphrase is not allowed.
    ///
    /// Unlimited tries are allowed.
    SettingUp,
    /// The user will be asked for a passphrase only once.
    Using,
}

impl PassInputMethod {
    /// Get passphrase using selected method.
    /// Details of this process differs for different secret key types and whether we run node
    /// or generate config files.
    pub fn get_passphrase(
        self,
        key_type: SecretKeyType,
        usage: PassphraseUsage,
    ) -> Result<Passphrase, Error> {
        match self {
            PassInputMethod::Terminal => {
                let prompt = key_type.prompt_message();
                match usage {
                    PassphraseUsage::SettingUp => prompt_passphrase(prompt),
                    PassphraseUsage::Using => Passphrase::read_from_tty(prompt),
                }
            }
            PassInputMethod::EnvVariable(name) => {
                let variable_name = name.unwrap_or_else(|| key_type.default_env_var().to_owned());
                let passphrase = env::var(&variable_name).with_context(|e| {
                    format!(
                        "Failed to get password from env variable {}: {}",
                        variable_name, e
                    )
                })?;
                Ok(Passphrase(passphrase))
            }
            PassInputMethod::CmdLineParameter(pass) => Ok(pass),
        }
    }
}

impl FromStr for PassInputMethod {
    type Err = failure::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Ok(Default::default());
        }

        if s == "stdin" {
            return Ok(PassInputMethod::Terminal);
        }

        if s.starts_with("env") {
            let env_var = s.split(':').nth(1).map(String::from);
            return Ok(PassInputMethod::EnvVariable(env_var));
        }

        if s.starts_with("pass") {
            let pass = s.split(':').nth(1).unwrap_or_default();
            return Ok(PassInputMethod::CmdLineParameter(Passphrase(
                pass.to_owned(),
            )));
        }

        bail!("Failed to parse passphrase input method")
    }
}

/// Prompt user for a passphrase. The user must enter the passphrase twice.
/// Passphrase must not be empty.
fn prompt_passphrase(prompt: &str) -> Result<Passphrase, Error> {
    loop {
        let password = Passphrase::read_from_tty(prompt)?;
        if password.is_empty() {
            eprintln!("Passphrase must not be empty. Try again.");
            continue;
        }

        let confirmation = Passphrase::read_from_tty("Enter same passphrase again: ")?;

        if password == confirmation {
            return Ok(password);
        } else {
            eprintln!("Passphrases do not match. Try again.");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::{PassInputMethod, Passphrase};

    #[test]
    fn test_pass_input_method_parse() {
        let correct_cases = vec![
            ("", <PassInputMethod as Default>::default()),
            ("", PassInputMethod::Terminal),
            ("stdin", PassInputMethod::Terminal),
            ("env", PassInputMethod::EnvVariable(None)),
            (
                "env:VAR",
                PassInputMethod::EnvVariable(Some("VAR".to_owned())),
            ),
            (
                "pass",
                PassInputMethod::CmdLineParameter(Passphrase("".to_owned())),
            ),
            (
                "pass:PASS",
                PassInputMethod::CmdLineParameter(Passphrase("PASS".to_owned())),
            ),
        ];

        for (inp, out) in correct_cases {
            let method = <PassInputMethod as FromStr>::from_str(inp);
            assert!(method.is_ok());
            assert_eq!(method.unwrap(), out)
        }
    }
}
