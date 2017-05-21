use clap::{SubCommand, App, Arg, ArgMatches};

use std::path::Path;
use std::marker::PhantomData;
use std::fs;

use config::ConfigFile;
use node::NodeConfig;
use storage::Storage;
use helpers::generate_testnet_config;

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

    pub fn node_config_path(matches: &'a ArgMatches<'a>) -> &'a Path {
        Path::new(matches.value_of("NODE_CONFIG_PATH").unwrap())
    }

    pub fn node_config(matches: &'a ArgMatches<'a>) -> NodeConfig {
        let path = Self::node_config_path(matches);
        ConfigFile::load(path).unwrap()
    }

    pub fn leveldb_path(matches: &'a ArgMatches<'a>) -> Option<&'a Path> {
        matches.value_of("LEVELDB_PATH").map(Path::new)
    }

    #[cfg(not(feature="memorydb"))]
    pub fn db(matches: &'a ArgMatches<'a>) -> Storage {
        use storage::{LevelDB, LevelDBOptions};

        let path = Self::leveldb_path(matches).unwrap();
        let mut options = LevelDBOptions::new();
        options.create_if_missing = true;
        LevelDB::new(path, options).unwrap()
    }

    #[cfg(feature="memorydb")]
    pub fn db(_: &'a ArgMatches<'a>) -> Storage {
        use storage::MemoryDB;
        MemoryDB::new()
    }
}
