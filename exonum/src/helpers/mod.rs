use log::{LogRecord, LogLevel, SetLoggerError};
use env_logger::LogBuilder;
use colored::*;

use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

use blockchain::{GenesisConfig, ValidatorKeys};
use node::NodeConfig;
use crypto::gen_keypair;

pub mod fabric;

pub fn init_logger() -> Result<(), SetLoggerError> {
    let mut builder = LogBuilder::new();
    builder.format(format_log_record);

    if env::var("RUST_LOG").is_ok() {
        builder.parse(&env::var("RUST_LOG").unwrap());
    }

    builder.init()
}

pub fn generate_testnet_config(count: u8, start_port: u16) -> Vec<NodeConfig> {
    let (validators, services): (Vec<_>, Vec<_>) = (0..count as usize)
        .map(|_| (gen_keypair(), gen_keypair())).unzip();
    let genesis = GenesisConfig::new(validators.iter().zip(services.iter()).map(|x| ValidatorKeys {
        consensus_key: (x.0).0,
        service_key: (x.1).0,
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

    let out = io::stderr();
    if let Some(term) = TerminfoTerminal::new(out) {
        term.supports_color()
    } else {
        false
    }
}

fn format_log_record(record: &LogRecord) -> String {
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let secs = ts.as_secs().to_string();
    let millis = (ts.subsec_nanos() as u64 / 1000000).to_string();

    let module = record.location().module_path();
    let file = record.location().file();
    let line = record.location().line();

    let source_path;
    let verbose_src_path = match env::var("RUST_VERBOSE_PATH") {
        Ok(val) => val.parse::<bool>().unwrap_or(false),
        Err(_) => false,
    };
    if verbose_src_path {
        source_path = format!("{}:{}:{}", module, file, line);
    } else {
        source_path = module.to_string();
    }

    if has_colors() {
        let level = match record.level() {
            LogLevel::Error => "ERROR".red(),
            LogLevel::Warn => "WARN".yellow(),
            LogLevel::Info => "INFO".green(),
            LogLevel::Debug => "DEBUG".cyan(),
            LogLevel::Trace => "TRACE".white(),
        };
        format!("[{} : {}] - [ {} ] - {} - {}",
                secs.bold(),
                millis.bold(),
                level,
                &source_path,
                record.args())
    } else {
        let level = match record.level() {
            LogLevel::Error => "ERROR",
            LogLevel::Warn => "WARN",
            LogLevel::Info => "INFO",
            LogLevel::Debug => "DEBUG",
            LogLevel::Trace => "TRACE",
        };
        format!("[{} : {}] - [ {} ] - {} - {}",
                secs,
                millis,
                level,
                &source_path,
                record.args())
    }
}