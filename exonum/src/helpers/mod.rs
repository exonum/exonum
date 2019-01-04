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

//! Different assorted utilities.

pub use self::types::{Height, Milliseconds, Round, ValidatorId};

pub mod config;
pub mod fabric;
pub mod user_agent;
#[macro_use]
pub mod metrics;
use crypto::gen_keypair;
use env_logger::Builder;
use log::SetLoggerError;

use blockchain::{GenesisConfig, ValidatorKeys};
use node::{ConnectListConfig, NodeConfig};

mod types;

/// Performs the logger initialization.
pub fn init_logger() -> Result<(), SetLoggerError> {
    Builder::from_default_env()
        .default_format_timestamp_nanos(true)
        .try_init()
}

/// Generates testnet configuration.
pub fn generate_testnet_config(count: u16, start_port: u16) -> Vec<NodeConfig> {
    let (validators, services): (Vec<_>, Vec<_>) = (0..count as usize)
        .map(|_| (gen_keypair(), gen_keypair()))
        .unzip();
    let genesis =
        GenesisConfig::new(
            validators
                .iter()
                .zip(services.iter())
                .map(|x| ValidatorKeys {
                    consensus_key: (x.0).0,
                    service_key: (x.1).0,
                }),
        );
    let peers = (0..validators.len())
        .map(|x| format!("127.0.0.1:{}", start_port + x as u16))
        .collect::<Vec<_>>();

    validators
        .into_iter()
        .zip(services.into_iter())
        .enumerate()
        .map(|(idx, (validator, service))| NodeConfig {
            listen_address: peers[idx].parse().unwrap(),
            external_address: peers[idx].clone(),
            network: Default::default(),
            consensus_public_key: validator.0,
            consensus_secret_key: validator.1,
            service_public_key: service.0,
            service_secret_key: service.1,
            genesis: genesis.clone(),
            connect_list: ConnectListConfig::from_validator_keys(&genesis.validator_keys, &peers),
            api: Default::default(),
            mempool: Default::default(),
            services_configs: Default::default(),
            database: Default::default(),
            thread_pool_size: Default::default(),
        })
        .collect::<Vec<_>>()
}
