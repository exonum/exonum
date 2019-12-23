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

//! Updating node configuration on the fly.

use exonum::{helpers::config::ConfigManager, node::ConnectListConfig};
use failure;
use log::error;

use std::{path::Path, sync::mpsc, thread};

use crate::{
    config::NodeConfig,
    io::{load_config_file, save_config_file},
};

/// Structure that handles work with config file at runtime.
#[derive(Debug)]
pub struct DefaultConfigManager {
    tx: mpsc::Sender<UpdateRequest>,
}

/// Messages for ConfigManager.
#[derive(Debug)]
pub struct UpdateRequest(ConnectListConfig);

impl DefaultConfigManager {
    /// Creates a new `ConfigManager` instance for the given path.
    pub fn new<P>(path: P) -> Self
    where
        P: AsRef<Path> + Send + 'static,
    {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            for UpdateRequest(connect_list) in rx {
                let res = Self::update_connect_list(connect_list, &path);

                if let Err(ref error) = res {
                    error!("Unable to update config: {}", error);
                }
            }
        });

        Self { tx }
    }

    // Updates ConnectList on file system synchronously.
    // This method is public only for testing and should not be used explicitly.
    #[doc(hidden)]
    pub fn update_connect_list<P>(
        connect_list: ConnectListConfig,
        path: &P,
    ) -> Result<(), failure::Error>
    where
        P: AsRef<Path>,
    {
        let mut current_config: NodeConfig = load_config_file(path)?;
        current_config.private_config.connect_list = connect_list;
        save_config_file(&current_config, path)?;

        Ok(())
    }
}

impl ConfigManager for DefaultConfigManager {
    /// Stores updated connect list at file system.
    fn store_connect_list(&mut self, connect_list: ConnectListConfig) {
        self.tx
            .send(UpdateRequest(connect_list))
            .expect("Can't message to ConfigManager thread");
    }
}

#[cfg(test)]
mod tests {
    use exonum::{
        crypto::gen_keypair,
        node::{ConnectInfo, ConnectListConfig},
    };
    use exonum_supervisor::mode::Mode;
    use tempfile::tempdir;

    use super::DefaultConfigManager;
    use crate::config::{GeneralConfig, NodeConfig, NodePrivateConfig, NodePublicConfig};
    use crate::io::{load_config_file, save_config_file};

    #[test]
    fn test_update_config() {
        let config = NodeConfig {
            private_config: NodePrivateConfig {
                listen_address: "127.0.0.1:5400".parse().unwrap(),
                external_address: "127.0.0.1:5400".to_string(),
                master_key_path: Default::default(),
                api: Default::default(),
                network: Default::default(),
                mempool: Default::default(),
                database: Default::default(),
                thread_pool_size: None,
                connect_list: Default::default(),
                keys: Default::default(),
            },
            public_config: NodePublicConfig {
                consensus: Default::default(),
                general: GeneralConfig {
                    validators_count: 1,
                    supervisor_mode: Mode::Simple,
                },
                validator_keys: None,
            },
        };
        let tmp_dir = tempdir().unwrap();
        let config_path = tmp_dir.path().join("node.toml");
        save_config_file(&config, &config_path).unwrap();

        // Test config update.
        let peer = ConnectInfo {
            address: "0.0.0.1:8080".to_owned(),
            public_key: gen_keypair().0,
        };

        let connect_list = ConnectListConfig { peers: vec![peer] };

        DefaultConfigManager::update_connect_list(connect_list.clone(), &config_path)
            .expect("Unable to update connect list");
        let config: NodeConfig = load_config_file(&config_path).unwrap();

        let new_connect_list = config.private_config.connect_list;
        assert_eq!(new_connect_list.peers, connect_list.peers);
    }
}
