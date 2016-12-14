use std::path::Path;
use std::marker::PhantomData;
use std::fs;

use clap::{SubCommand, App, Arg, ArgMatches};

use exonum::config::ConfigFile;
use exonum::blockchain::GenesisConfig;
use exonum::node::NodeConfig;
use exonum::crypto::gen_keypair;
use exonum::storage::{LevelDB, LevelDBOptions, MemoryDB};

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

pub enum DatabaseType {
    LevelDB(LevelDB),
    MemoryDB(MemoryDB),
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

    pub fn db(matches: &'a ArgMatches<'a>) -> DatabaseType {
        if let Some(path) = Self::leveldb_path(matches) {
            let mut options = LevelDBOptions::new();
            options.create_if_missing = true;
            let db = LevelDB::new(path, options).unwrap();

            DatabaseType::LevelDB(db)
        } else {
            DatabaseType::MemoryDB(MemoryDB::new())
        }
    }
}
