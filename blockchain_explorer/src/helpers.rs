use std::path::Path;
use std::marker::PhantomData;
use std::fs;
use std::env;

use time;
use clap::{SubCommand, App, Arg, ArgMatches};
use log::{LogRecord, LogLevel, SetLoggerError};
use env_logger::LogBuilder;
use colored::*;

use exonum::config::ConfigFile;
use exonum::blockchain::GenesisConfig;
use exonum::node::NodeConfig;
use exonum::crypto::gen_keypair;
use exonum::storage::Storage;

pub struct GenerateCommand<'a, 'b>
    where 'a: 'b
{
    _p: PhantomData<App<'a, 'b>>,
}

// TODO avoid unwraps here, implement Error chain

impl<'a, 'b> GenerateCommand<'a, 'b>
    where 'a: 'b
{
    pub fn new() -> App<'a, 'b> {
        SubCommand::with_name("generate")
            .about("Generates genesis configuration")
            .arg(Arg::with_name("OUTPUT_DIR")
                .short("o")
                .long("output-dir")
                .value_name("OUTPUT_DIR")
                .required(true)
                .takes_value(true))
            .arg(Arg::with_name("START_PORT")
                .short("p")
                .long("start-port")
                .value_name("START_PORT")
                .required(false)
                .takes_value(true))
            .arg(Arg::with_name("COUNT")
                .help("Validators count")
                .required(true)
                .index(1))
    }

    pub fn output_dir(matches: &'a ArgMatches<'a>) -> &'a Path {
        Path::new(matches.value_of("OUTPUT_DIR").unwrap())
    }

    pub fn validators_count(matches: &'a ArgMatches<'a>) -> u8 {
        matches.value_of("COUNT").unwrap().parse().unwrap()
    }

    pub fn start_port(matches: &'a ArgMatches<'a>) -> Option<u16> {
        matches.value_of("START_PORT").map(|p| p.parse().unwrap())
    }

    pub fn execute(matches: &'a ArgMatches<'a>) {
        let dir = Self::output_dir(matches);
        let count = Self::validators_count(matches);
        let start_port = Self::start_port(matches).unwrap_or_else(|| 2000);

        let dir = dir.join("validators");
        if !dir.exists() {
            fs::create_dir_all(&dir).unwrap();
        }

        let validators = (0..count)
            .map(|_| gen_keypair())
            .collect::<Vec<_>>();
        let genesis = GenesisConfig::new(validators.iter().map(|x| x.0));
        let peers = (0..validators.len())
            .map(|x| format!("127.0.0.1:{}", start_port + x as u16).parse().unwrap())
            .collect::<Vec<_>>();

        for (idx, validator) in validators.into_iter().enumerate() {
            let cfg = NodeConfig {
                listen_address: peers[idx],
                network: Default::default(),
                peers: peers.clone(),
                public_key: validator.0,
                secret_key: validator.1,
                genesis: genesis.clone(),
            };

            let file_name = format!("{}.toml", idx);
            ConfigFile::save(&cfg, &dir.join(file_name)).unwrap();
        }
    }
}

pub struct RunCommand<'a, 'b>
    where 'a: 'b
{
    _p: PhantomData<App<'a, 'b>>,
}

impl<'a, 'b> RunCommand<'a, 'b>
    where 'a: 'b
{
    pub fn new() -> App<'a, 'b> {
        SubCommand::with_name("run")
            .arg(Arg::with_name("NODE_CONFIG_PATH")
                .short("c")
                .long("node-config")
                .value_name("NODE_CONFIG_PATH")
                .help("Path to node configuration file")
                .required(true)
                .takes_value(true))
            .arg(Arg::with_name("LEVELDB_PATH")
                .short("d")
                .long("leveldb-path")
                .value_name("LEVELDB_PATH")
                .help("Use leveldb database with the given path")
                .required(false)
                .takes_value(true))
    }

    pub fn node_config(matches: &'a ArgMatches<'a>) -> NodeConfig {
        let path = Path::new(matches.value_of("NODE_CONFIG_PATH").unwrap());
        ConfigFile::load(path).unwrap()
    }

    pub fn leveldb_path(matches: &'a ArgMatches<'a>) -> Option<&'a Path> {
        matches.value_of("LEVELDB_PATH").map(Path::new)
    }

    #[cfg(not(feature="memorydb"))]
    pub fn db(matches: &'a ArgMatches<'a>) -> Storage {
        use exonum::storage::{LevelDB, LevelDBOptions};

        let path = Self::leveldb_path(matches).unwrap();
        let mut options = LevelDBOptions::new();
        options.create_if_missing = true;
        LevelDB::new(path, options).unwrap()
    }

    #[cfg(feature="memorydb")]
    pub fn db(_: &'a ArgMatches<'a>) -> Storage {
        use exonum::storage::MemoryDB;
        MemoryDB::new()
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

pub fn init_logger() -> Result<(), SetLoggerError> {
    let format = |record: &LogRecord| {
        let ts = time::now_utc().to_timespec();
        let now = (ts.sec * 1000 + ts.nsec as i64 / 1000000).to_string();

        if has_colors() {
            let level = match record.level() {
                LogLevel::Error => "ERROR".red(),
                LogLevel::Warn => "WARN".yellow(),
                LogLevel::Info => "INFO".green(),
                LogLevel::Debug => "DEBUG".cyan(),
                LogLevel::Trace => "TRACE".white(),
            };
            format!("{} - [ {} ] - {}", now.bold(), level, record.args())
        } else {
            let level = match record.level() {
                LogLevel::Error => "ERROR",
                LogLevel::Warn => "WARN",
                LogLevel::Info => "INFO",
                LogLevel::Debug => "DEBUG",
                LogLevel::Trace => "TRACE",
            };
            format!("{} - [ {} ] - {}", now, level, record.args())
        }
    };

    let mut builder = LogBuilder::new();
    builder.format(format);

    if env::var("RUST_LOG").is_ok() {
        builder.parse(&env::var("RUST_LOG").unwrap());
    }

    builder.init()
}