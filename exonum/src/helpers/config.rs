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

//! Loading and saving TOML-encoded configurations.

use failure::{Error, ResultExt};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use toml;

use std::{
    fs::{self, File}, io::{Read, Write}, mem::drop, path::Path, sync::mpsc, thread,
};

use node::{ConnectListConfig, NodeConfig};

/// Implements loading and saving TOML-encoded configurations.
#[derive(Debug)]
pub struct ConfigFile {}

impl ConfigFile {
    /// Loads TOML-encoded file.
    pub fn load<P, T>(path: P) -> Result<T, Error>
    where
        T: for<'r> Deserialize<'r>,
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        let res = do_load(path).context(format!("loading config from {}", path.display()))?;
        Ok(res)
    }

    /// Saves TOML-encoded file.
    pub fn save<P, T>(value: &T, path: P) -> Result<(), Error>
    where
        T: Serialize,
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        do_save(value, path).with_context(|_| format!("saving config to {}", path.display()))?;
        Ok(())
    }
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

/// Structure that handles work with config file in runtime.
#[derive(Debug)]
pub struct ConfigManager {
    handle: thread::JoinHandle<()>,
    tx: mpsc::Sender<ConfigRequest>,
}

/// Messages for ConfigReader.
#[derive(Debug)]
pub enum ConfigRequest {
    /// Request for connect list update in config file.
    UpdateConnectList(ConnectListConfig),
}

impl ConfigManager {
    /// Creates a new `ConfigManager` instance for the given path.
    pub fn new(path: String) -> Self {
        let (tx, rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            info!("ConfigManager started");
            for command in rx {
                match command {
                    ConfigRequest::UpdateConnectList(connect_list) => {
                        Self::update_connect_list(connect_list, &path)
                    }
                }
            }
            info!("ConfigManager stopped");
        });

        ConfigManager { handle, tx }
    }

    /// Stores updated connect list in file system.
    pub fn store_connect_list(&self, connect_list: ConnectListConfig) {
        self.tx
            .send(ConfigRequest::UpdateConnectList(connect_list))
            .expect("Can't write");
    }

    /// Stops `ConfigManager`.
    pub fn stop(self) {
        drop(self.tx);
        self.handle.join().expect("Can't stop thread");
    }

    fn update_connect_list(connect_list: ConnectListConfig, path: &String) {
        // TODO: remove expect.
        let mut current_config: NodeConfig =
            ConfigFile::load(path.clone()).expect("Can't load node config file");

        current_config.connect_list = connect_list;

        ConfigFile::save(&current_config, path.clone()).expect("Can't save node config file");
    }
}
