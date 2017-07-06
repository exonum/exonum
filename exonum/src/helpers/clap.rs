use clap::{SubCommand, App, Arg, ArgMatches};

use std::path::Path;
use std::marker::PhantomData;
use std::fs;
use std::net::SocketAddr;

use config::ConfigFile;
use node::NodeConfig;
use storage::Database;
use helpers::generate_testnet_config;

#[derive(Debug)]
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

        let configs = generate_testnet_config(count, start_port);
        for (idx, cfg) in configs.into_iter().enumerate() {
            let file_name = format!("{}.toml", idx);
            ConfigFile::save(&cfg, &dir.join(file_name)).unwrap();
        }
    }
}

#[derive(Debug)]
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
            .about("Run node with given configuration")
            .arg(Arg::with_name("NODE_CONFIG_PATH")
                     .short("c")
                     .long("node-config")
                     .value_name("NODE_CONFIG_PATH")
                     .help("Path to node configuration file")
                     .required(true)
                     .takes_value(true))
            .arg(Arg::with_name("DB_PATH")
                     .short("d")
                     .long("db-path")
                     .help("Use database with the given path")
                     .required(false)
                     .takes_value(true))
            .arg(Arg::with_name("PUBLIC_API_ADDRESS")
                     .long("public-api-address")
                     .help("Listen address for public api")
                     .required(false)
                     .takes_value(true))
            .arg(Arg::with_name("PRIVATE_API_ADDRESS")
                     .long("private-api-address")
                     .help("Listen address for private api")
                     .required(false)
                     .takes_value(true))
    }

    pub fn node_config_path(matches: &'a ArgMatches<'a>) -> &'a Path {
        matches
            .value_of("NODE_CONFIG_PATH")
            .map(Path::new)
            .expect("Path to node configuration is no setted")
    }

    pub fn node_config(matches: &'a ArgMatches<'a>) -> NodeConfig {
        let path = Self::node_config_path(matches);
        let mut cfg: NodeConfig = ConfigFile::load(path).unwrap();
        // Override api options
        if let Some(addr) = Self::public_api_address(matches) {
            cfg.api.public_api_address = Some(addr);
        }
        if let Some(addr) = Self::private_api_address(matches) {
            cfg.api.private_api_address = Some(addr);
        }
        cfg
    }

    pub fn public_api_address(matches: &'a ArgMatches<'a>) -> Option<SocketAddr> {
        matches
            .value_of("PUBLIC_API_ADDRESS")
            .map(|s| s.parse().expect("Public api address has incorrect format"))
    }

    pub fn private_api_address(matches: &'a ArgMatches<'a>) -> Option<SocketAddr> {
        matches
            .value_of("PRIVATE_API_ADDRESS")
            .map(|s| s.parse().expect("Private api address has incorrect format"))
    }

    pub fn db_path(matches: &'a ArgMatches<'a>) -> Option<&'a Path> {
        matches.value_of("DB_PATH").map(Path::new)
    }

    #[cfg(not(feature="memorydb"))]
    pub fn db(matches: &'a ArgMatches<'a>) -> Box<Database> {
        use storage::{RocksDB, RocksDBOptions};
        let path = Self::db_path(matches).unwrap();
        let mut options = RocksDBOptions::default();
        options.create_if_missing(true);
        Box::new(RocksDB::open(path, options).unwrap())
    }

    #[cfg(feature="memorydb")]
    pub fn db(_: &'a ArgMatches<'a>) -> Box<Database> {
        use storage::MemoryDB;
        Box::new(MemoryDB::new())
    }
}
