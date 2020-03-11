// Copyright 2020 The Exonum Team
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

use anyhow::{Context, Error};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use std::{
    fs::{self, File},
    io::{Read, Write},
    path::Path,
};

/// Loads TOML-encoded file.
pub fn load_config_file<P, T>(path: P) -> Result<T, Error>
where
    T: for<'r> Deserialize<'r>,
    P: AsRef<Path>,
{
    let path = path.as_ref();
    let res = do_load(path).with_context(|| format!("loading config from {}", path.display()))?;
    Ok(res)
}

/// Saves TOML-encoded file.
///
/// Creates directory if needed.
pub fn save_config_file<P, T>(value: &T, path: P) -> Result<(), Error>
where
    T: Serialize,
    P: AsRef<Path>,
{
    let path = path.as_ref();
    do_save(value, path).with_context(|| format!("saving config to {}", path.display()))?;
    Ok(())
}

fn do_load<T: DeserializeOwned>(path: &Path) -> Result<T, Error> {
    let mut file = File::open(path)?;
    let mut toml = String::new();
    file.read_to_string(&mut toml)?;
    Ok(toml::de::from_str(&toml)?)
}

fn do_save<T: Serialize>(value: &T, path: &Path) -> Result<(), Error> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut file = File::create(path)?;
    let value_toml = toml::Value::try_from(value)?;
    file.write_all(value_toml.to_string().as_bytes())?;
    Ok(())
}
