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

use chrono::{DateTime, Utc};
use colored::*;
use env_logger::{Builder, Formatter};
use log::{Level, Record, SetLoggerError};

use std::{
    env, io::{self, Write}, time::SystemTime,
};

use blockchain::{GenesisConfig, ValidatorKeys};
use crypto::gen_keypair;
use node::{ConnectListConfig, NodeConfig};

mod types;

/// Format for timestamps in logs.
///
/// It is similar to date/time format of RFC 2822, but with milliseconds:
/// "Mon, 16 Jul 2018 13:37:18.594 +0100"
const LOG_TIMESTAMP_FORMAT: &str = "%a, %e %b %Y %H:%M:%S%.3f %z";

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
        .map(|(idx, (validator, service))| NodeConfig {
            listen_address: peers[idx],
            external_address: peers[idx],
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
        })
        .collect::<Vec<_>>()
}

fn has_colors() -> bool {
    use atty;
    use std::io;
    use term::terminfo::TerminfoTerminal;
    use term::Terminal;

    let out = io::stderr();
    match TerminfoTerminal::new(out) {
        Some(ref term) if atty::is(atty::Stream::Stderr) => term.supports_color(),
        _ => false,
    }
}

fn format_time(time: SystemTime) -> String {
    DateTime::<Utc>::from(time)
        .format(LOG_TIMESTAMP_FORMAT)
        .to_string()
}

fn format_log_record(buf: &mut Formatter, record: &Record) -> io::Result<()> {
    let time = format_time(SystemTime::now());

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
            "{} {} {} {}",
            time.dimmed(),
            level,
            source_path.dimmed(),
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
        writeln!(buf, "{} {} {} {}", time, level, &source_path, record.args())
    }
}

#[cfg(test)]
mod tests {
    use std::time;

    use super::*;

    #[test]
    fn time_formatting() {
        assert_eq!(
            format_time(time::UNIX_EPOCH),
            "Thu,  1 Jan 1970 00:00:00.000 +0000"
        );
    }
}
