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

//! Loading and saving TOML-encoded configurations.

use serde::{Serialize, Deserialize};
use toml;

use std::path::Path;
use std::io::{self, Read, Write};
use std::fs::{self, File};
use std::error::Error;

/// Implements loading and saving TOML-encoded configurations.
#[derive(Debug)]
pub struct ConfigFile {}

impl ConfigFile {
    /// Loads TOML-encoded file.
    pub fn load<P, T>(path: P) -> Result<T, Box<Error>>
    where
        T: for<'r> Deserialize<'r>,
        P: AsRef<Path>,
    {
        let mut file = File::open(path.as_ref())?;
        let mut toml = String::new();
        file.read_to_string(&mut toml)?;
        toml::de::from_str(&toml).map_err(|_| {
            let e = io::Error::new(io::ErrorKind::InvalidData, "Unable to decode toml file");
            Box::new(e) as Box<Error>
        })
    }

    /// Saves TOML-encoded file.
    pub fn save<P, T>(value: &T, path: P) -> Result<(), Box<Error>>
    where
        T: Serialize,
        P: AsRef<Path>,
    {
        if let Some(dir) = path.as_ref().parent() {
            fs::create_dir_all(dir)?;
        }

        let mut file = File::create(path.as_ref())?;
        let value_toml = toml::Value::try_from(value)?;
        file.write_all(&format!("{}", value_toml).into_bytes())?;

        Ok(())
    }
}
