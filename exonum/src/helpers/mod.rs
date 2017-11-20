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

//! Different assorted utilities.

use env_logger::LogBuilder;
use colored::*;

use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

use blockchain::{GenesisConfig, ValidatorKeys};
use node::NodeConfig;
use crypto::gen_keypair;

use slog::Drain;
use slog_term::{PlainSyncDecorator, FullFormat};

pub use self::types::{Height, Round, ValidatorId, Milliseconds};

mod types;

pub mod fabric;
pub mod config;
#[macro_use]
pub mod metrics;


// TODO: replace before merge
// Stub for future replacement
pub type ExonumLogger = Box<Drain>;
/// Performs the logger initialization.
pub fn root_logger() -> ExonumLogger {
    let plain = PlainSyncDecorator::new(::std::io::stdout());
    Box::new(FullFormat::new(plain).build().fuse(), slog_o!())
}

/// Generates testnet configuration.
pub fn generate_testnet_config(count: u8, start_port: u16) -> Vec<NodeConfig> {
    let (validators, services): (Vec<_>, Vec<_>) = (0..count as usize)
        .map(|_| (gen_keypair(), gen_keypair()))
        .unzip();
    let genesis = GenesisConfig::new(validators.iter().zip(services.iter()).map(|x| {
        ValidatorKeys {
            consensus_key: (x.0).0,
            service_key: (x.1).0,
        }
    }));
    let peers = (0..validators.len())
        .map(|x| {
            format!("127.0.0.1:{}", start_port + x as u16)
                .parse()
                .unwrap()
        })
        .collect::<Vec<_>>();

    validators
        .into_iter()
        .zip(services.into_iter())
        .enumerate()
        .map(|(idx, (validator, service))| {
            NodeConfig {
                listen_address: peers[idx],
                external_address: Some(peers[idx]),
                network: Default::default(),
                peers: peers.clone(),
                consensus_public_key: validator.0,
                consensus_secret_key: validator.1,
                service_public_key: service.0,
                service_secret_key: service.1,
                genesis: genesis.clone(),
                whitelist: Default::default(),
                api: Default::default(),
                mempool: Default::default(),
                services_configs: Default::default(),
            }
        })
        .collect::<Vec<_>>()
}

fn has_colors() -> bool {
    use term::terminfo::TerminfoTerminal;
    use term::Terminal;
    use std::io;
    use atty;

    let out = io::stderr();
    match TerminfoTerminal::new(out) {
        Some(ref term) if atty::is(atty::Stream::Stderr) => term.supports_color(),
        _ => false,
    }
}

