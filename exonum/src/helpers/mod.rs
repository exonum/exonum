use std::thread;
use std::net::SocketAddr;

use log::{LogRecord, LogLevel, SetLoggerError};
use env_logger::LogBuilder;
use colored::*;
use router::Router;
use iron::{Chain, Iron};

use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

use blockchain::{GenesisConfig, ApiContext};
use node::{NodeConfig, Node};
use crypto::gen_keypair;
use explorer::ExplorerApi;
use api::Api;

pub mod clap;

pub fn init_logger() -> Result<(), SetLoggerError> {
    let mut builder = LogBuilder::new();
    builder.format(format_log_record);

    if env::var("RUST_LOG").is_ok() {
        builder.parse(&env::var("RUST_LOG").unwrap());
    }

    builder.init()
}

pub fn generate_testnet_config(count: u8, start_port: u16) -> Vec<NodeConfig> {
    let validators = (0..count as usize)
        .map(|_| gen_keypair())
        .collect::<Vec<_>>();
    let genesis = GenesisConfig::new(validators.iter().map(|x| x.0));
    let peers = (0..validators.len())
        .map(|x| {
                 format!("127.0.0.1:{}", start_port + x as u16)
                     .parse()
                     .unwrap()
             })
        .collect::<Vec<_>>();

    validators
        .into_iter()
        .enumerate()
        .map(|(idx, validator)| {
            NodeConfig {
                listen_address: peers[idx],
                network: Default::default(),
                peers: peers.clone(),
                public_key: validator.0,
                secret_key: validator.1,
                genesis: genesis.clone(),
            }
        })
        .collect::<Vec<_>>()
}

/// Options for the `run_node_with_api` function.
pub struct NodeRunOptions {
    /// Enable api endpoints for the `blockchain_explorer` on public api address.
    pub enable_explorer: bool,
    /// Listen address for public api endpoints
    pub public_api_address: Option<SocketAddr>,
    /// Listen address for private api endpoints
    pub private_api_address: Option<SocketAddr>,
}

/// A generic implementation that launches `Node` and optionally creates threads 
/// for public and private api handlers.
pub fn run_node_with_api(mut node: Node, options: NodeRunOptions) {
    let blockchain = node.handler().blockchain.clone();
    let private_config_api_thread = match options.private_api_address {
        Some(listen_address) => {
            let blockchain_clone = blockchain.clone();
            let api_context = ApiContext::new(&node);
            let thread = thread::spawn(move || {
                info!("Private exonum api started on {}", listen_address);

                let mut router = Router::new();
                blockchain_clone.wire_private_api(&api_context, &mut router);
                let chain = Chain::new(router);
                Iron::new(chain).http(listen_address).unwrap();
            });
            Some(thread)
        }
        None => None,
    };

    let public_config_api_thread = match options.public_api_address {
        Some(listen_address) => {
            let blockchain_clone = blockchain.clone();
            let api_context = ApiContext::new(&node);
            let thread = thread::spawn(move || {
                info!("Public exonum api started on {}", listen_address);

                let mut router = Router::new();
                blockchain_clone.wire_public_api(&api_context, &mut router);
                if options.enable_explorer {
                    let explorer_api = ExplorerApi {
                        blockchain: blockchain_clone
                    };
                    explorer_api.wire(&mut router);
                }
                let chain = Chain::new(router);
                Iron::new(chain).http(listen_address).unwrap();
            });
            Some(thread)
        }
        None => None,
    };
    
    node.run().unwrap();
    if let Some(private_config_api_thread) = private_config_api_thread {
        private_config_api_thread.join().unwrap();
    }
    if let Some(public_config_api_thread) = public_config_api_thread {
        public_config_api_thread.join().unwrap();
    }
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