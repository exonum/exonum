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

use std::env;
use std::io::{self, Write};
use std::time::{SystemTime, UNIX_EPOCH};

use log::{Level, Record, SetLoggerError};
use env_logger::{Builder, Formatter};
use colored::*;

use blockchain::{GenesisConfig, ValidatorKeys};
use node::NodeConfig;
use crypto::gen_keypair;

pub use self::types::{Height, Milliseconds, Round, ValidatorId};

mod types;

pub mod fabric;
pub mod config;
#[macro_use]
pub mod metrics;

/// Performs the logger initialization.
pub fn init_logger() -> Result<(), SetLoggerError> {
    let mut builder = Builder::new();
    builder.format(format_log_record);

    if env::var("RUST_LOG").is_ok() {
        builder.parse(&env::var("RUST_LOG").unwrap());
    }

    builder.try_init()
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

fn format_log_record(buf: &mut Formatter, record: &Record) -> io::Result<()> {
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let secs = ts.as_secs().to_string();
    let millis = (u64::from(ts.subsec_nanos()) / 1_000_000).to_string();

    let verbose_src_path = match env::var("RUST_VERBOSE_PATH") {
        Ok(val) => val.parse::<bool>().unwrap_or(false),
        Err(_) => false,
    };

    let module = record.module_path().unwrap_or("unknown_module");
    let source_path = if verbose_src_path {
        let file = record.file().unwrap_or("unknown_file");
        let line = record.line().unwrap_or(0);
        format!("{}:{}:{}", module, file, line)
    } else {
        module.to_string()
    };

    if has_colors() {
        let level = match record.level() {
            Level::Error => "ERROR".red(),
            Level::Warn => "WARN".yellow(),
            Level::Info => "INFO".green(),
            Level::Debug => "DEBUG".cyan(),
            Level::Trace => "TRACE".white(),
        };
        writeln!(
            buf,
            "[{} : {:03}] - [ {} ] - {} - {}",
            secs.bold(),
            millis.bold(),
            level,
            &source_path,
            record.args()
        )
    } else {
        let level = match record.level() {
            Level::Error => "ERROR",
            Level::Warn => "WARN",
            Level::Info => "INFO",
            Level::Debug => "DEBUG",
            Level::Trace => "TRACE",
        };
        writeln!(
            buf,
            "[{} : {:03}] - [ {} ] - {} - {}",
            secs,
            millis,
            level,
            &source_path,
            record.args()
        )
    }
}
