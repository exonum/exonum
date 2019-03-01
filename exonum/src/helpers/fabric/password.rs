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

use failure::bail;
use rpassword::read_password_from_tty;

use std::{env, io, str::FromStr};

use crate::helpers::ZeroizeOnDrop;

/// Passphrase input methods
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum PassInputMethod {
    /// Prompt passphrase from terminal.
    Terminal,
    /// Get passphrase from environment variable (default if `None`).
    EnvVariable(Option<String>),
    /// Passphrase is passed as a command line parameter.
    CmdLineParameter(ZeroizeOnDrop<String>),
}

/// Secret key types.
#[derive(Copy, Clone)]
pub enum SecretKeyType {
    Consensus,
    Service,
}

impl PassInputMethod {
    /// Get passphrase using selected method.
    /// Details of this process differs for different secret key types and whether we run node
    /// or generate config files.
    pub fn get_passphrase(self, key_type: SecretKeyType, node_run: bool) -> ZeroizeOnDrop<String> {
        match self {
            PassInputMethod::Terminal => {
                let prompt = match key_type {
                    SecretKeyType::Consensus => "Enter consensus key passphrase",
                    SecretKeyType::Service => "Enter service key passphrase",
                };
                prompt_passphrase(prompt, node_run).expect("Failed to read password from stdin")
            }
            PassInputMethod::EnvVariable(name) => {
                let var = if let Some(ref name) = name {
                    name
                } else {
                    match key_type {
                        SecretKeyType::Consensus => "EXONUM_CONSENSUS_PASS",
                        SecretKeyType::Service => "EXONUM_SERVICE_PASS",
                    }
                };
                ZeroizeOnDrop(env::var(var).unwrap_or_else(|e| {
                    panic!("Failed to get password from env variable: {}, {}", var, e)
                }))
            }
            PassInputMethod::CmdLineParameter(pass) => pass,
        }
    }
}

impl Default for PassInputMethod {
    fn default() -> Self {
        PassInputMethod::Terminal
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
            return Ok(PassInputMethod::CmdLineParameter(ZeroizeOnDrop(
                pass.to_owned(),
            )));
        }

        bail!("Failed to parse passphrase input method")
    }
}

fn prompt_passphrase(prompt: &str, node_run: bool) -> io::Result<ZeroizeOnDrop<String>> {
    if node_run {
        return Ok(ZeroizeOnDrop(read_password_from_tty(Some(prompt))?));
    }

    loop {
        let password = ZeroizeOnDrop(read_password_from_tty(Some(prompt))?);
        if password.is_empty() {
            eprintln!("Passphrase must not be empty. Try again.");
            continue;
        }

        let confirmation = ZeroizeOnDrop(read_password_from_tty(Some(
            "Enter same passphrase again: ",
        ))?);

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

    use super::PassInputMethod;
    use crate::helpers::ZeroizeOnDrop;

    #[test]
    fn test_pass_input_method_parse() {
        let correct_cases = vec![
            ("", <PassInputMethod as Default>::default()),
            ("stdin", PassInputMethod::Terminal),
            ("env", PassInputMethod::EnvVariable(None)),
            (
                "env:VAR",
                PassInputMethod::EnvVariable(Some("VAR".to_owned())),
            ),
            (
                "pass",
                PassInputMethod::CmdLineParameter(ZeroizeOnDrop("".to_owned())),
            ),
            (
                "pass:PASS",
                PassInputMethod::CmdLineParameter(ZeroizeOnDrop("PASS".to_owned())),
            ),
        ];

        for (inp, out) in correct_cases {
            let method = <PassInputMethod as FromStr>::from_str(inp);
            assert!(method.is_ok());
            assert_eq!(method.unwrap(), out)
        }
    }
}
