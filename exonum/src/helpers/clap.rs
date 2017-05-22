
use clap::{SubCommand, App, Arg, ArgMatches};

use std::path::Path;
use std::marker::PhantomData;
use std::collections::BTreeMap;
use std::net::SocketAddr;

use config::ConfigFile;
use blockchain::{ConsensusConfig, GenesisConfig};
use node::NodeConfig;
use storage::Storage;
use crypto::{self, PublicKey, SecretKey};

const DEFAULT_EXONUM_LISTEN_PORT: u16 = 6333;

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
            .arg(Arg::with_name("LEVELDB_PATH")
                     .short("d")
                     .long("leveldb-path")
                     .value_name("LEVELDB_PATH")
                     .help("Use leveldb database with the given path")
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

    pub fn leveldb_path(matches: &'a ArgMatches<'a>) -> Option<&'a Path> {
        matches.value_of("LEVELDB_PATH").map(Path::new)
    }

    #[cfg(not(feature="memorydb"))]
    pub fn db(matches: &'a ArgMatches<'a>) -> Box<Database> {
        use storage::{LevelDB, LevelDBOptions};

        let path = Self::leveldb_path(matches).unwrap();
        let mut options = LevelDBOptions::new();
        options.create_if_missing = true;
        Box::new(LevelDB::open(path, options).unwrap())
    }

    #[cfg(feature="memorydb")]
    pub fn db(_: &'a ArgMatches<'a>) -> Box<Database> {
        use storage::MemoryDB;
        Box::new(MemoryDB::new())
    }
}

#[derive(Serialize, Deserialize)]
pub struct ValidatorIdent {
    addr: SocketAddr,
}

#[derive(Serialize, Deserialize, Default)]
pub struct TemplateConfig {
    validators: BTreeMap<PublicKey, ValidatorIdent>,
    consensus_cfg: ConsensusConfig,
    count: usize
}

#[derive(Serialize, Deserialize)]
pub struct KeyConfig {
    public_key: PublicKey,
    secret_key: SecretKey,
}


pub struct KeyGeneratorCommand;
impl KeyGeneratorCommand
{
    /// Create basic keygen subcommand.
    pub fn new<'a>() -> App<'a, 'a> {
        SubCommand::with_name("keygen")
            .about("Generate basic node secret key.")
            .arg(Arg::with_name("KEYCHAIN")
                     .help("Path to key config.")
                     .required(true)
                     .index(1))
    }

    /// Path where keychain config should be saved
    pub fn keychain<'a>(matches: &'a ArgMatches<'a>) -> &'a Path {
        Path::new(matches.value_of("KEYCHAIN").unwrap())
    }

    /// Generate and write key config to `keychain()` path.
    pub fn execute_default(matches: &ArgMatches) {
        let (pub_key, sec_key) = crypto::gen_keypair();
        let config = KeyConfig {
            public_key: pub_key,
            secret_key: sec_key,
        };

        ConfigFile::save(&config, Self::keychain(matches)).unwrap();
    }
}

/// implement command for template generating
pub struct GenerateTemplateCommand;
impl GenerateTemplateCommand
{
    pub fn new<'a>() -> App<'a, 'a> {
        SubCommand::with_name("generate-template")
            .about("Generate basic template.")
            .arg(Arg::with_name("COUNT")
                     .help("Validator total count.")
                     .required(true)
                     .index(1))
            .arg(Arg::with_name("TEMPLATE")
                     .help("Path to template config.")
                     .required(true)
                     .index(2))
    }

    /// Path where template config should be saved
    pub fn template<'a>(matches: &'a ArgMatches<'a>) -> &'a Path {
        Path::new(matches.value_of("TEMPLATE").unwrap())
    }
    /// Validator total count
    pub fn count(matches: &ArgMatches) -> usize {
        matches.value_of("COUNT").unwrap().parse().unwrap()
    }

    /// Write default template config into `template()` path.
    pub fn execute_default(matches: &ArgMatches) {
        let mut template = TemplateConfig::default();
        template.count = Self::count(matches);
        ConfigFile::save(&template, Self::template(matches)).unwrap();
    }
}

pub struct PreInitCommand;
impl PreInitCommand
{
    pub fn new<'a>() -> App<'a, 'a> {
        SubCommand::with_name("preinit")
            .about("Preinit configuration, optionaly generate public and secret key.")
            .arg(Arg::with_name("TEMPLATE")
                     .help("Path to template")
                     .required(true)
                     .index(1))
            .arg(Arg::with_name("KEYCHAIN")
                     .help("Path to secret and public key file.")
                     .required(true)
                     .index(2))
             .arg(Arg::with_name("LISTEN_ADDR")
                     .short("a")
                     .long("listen-addr")
                     .value_name("LISTEN_ADDR")
                     .required(true)
                     .takes_value(true))                    
            .arg(Arg::with_name("PORT")
                     .short("p")
                     .long("port")
                     .value_name("PORT")
                     .required(false)
                     .takes_value(true))
            
    }

    pub fn keychain<'a>(matches: &'a ArgMatches<'a>) -> &'a Path {
        Path::new(matches.value_of("KEYCHAIN").unwrap())
    }

    pub fn template<'a>(matches: &'a ArgMatches<'a>) -> &'a Path {
        Path::new(matches.value_of("TEMPLATE").unwrap())
    }

    pub fn port(matches: &ArgMatches) -> Option<u16> {
        matches.value_of("PORT").and_then(|port| port.parse().ok())
    }

    pub fn addr(matches: &ArgMatches) -> String {
        matches.value_of("LISTEN_ADDR").unwrap().to_string()
    }

    #[cfg_attr(feature="cargo-clippy", allow(map_entry))]
    pub fn execute_default(matches: &ArgMatches) {
        let template_path = Self::template(matches);
        let keychain_path = Self::keychain(matches);

        let mut template: TemplateConfig = ConfigFile::load(template_path).unwrap();
        let keychain: KeyConfig = ConfigFile::load(keychain_path).unwrap();
        let addr = format!("{}:{}", Self::addr(matches),
                     Self::port(matches).unwrap_or(DEFAULT_EXONUM_LISTEN_PORT))
                     .parse()
                     .unwrap();
        if !template.validators.contains_key(&keychain.public_key) {
            if template.validators.len() >= template.count {
                panic!("This template alredy full.");
            }
            let ident = ValidatorIdent { 
                addr: addr
            };
            template.validators.insert(keychain.public_key, ident);
        }
        else {
            panic!("This node alredy in template");
        }

        ConfigFile::save(&template, template_path).unwrap();
    }
}

pub struct InitCommand;

impl InitCommand
{
    pub fn new<'a>() -> App<'a, 'a> {
        SubCommand::with_name("init")
            .about("Toolchain to generate configuration")
            .arg(Arg::with_name("FULL_TEMPLATE")
                     .help("Path to full template")
                     .required(true)
                     .index(1))
            .arg(Arg::with_name("KEYCHAIN")
                     .help("Path to keychain config.")
                     .required(true)
                     .index(2))
            .arg(Arg::with_name("CONFIG_PATH")
                     .help("Path to node config.")
                     .required(true)
                     .index(3))

    }

    pub fn template<'a>(matches: &'a ArgMatches<'a>) -> &'a Path {
        Path::new(matches.value_of("FULL_TEMPLATE").unwrap())
    }

    pub fn config<'a>(matches: &'a ArgMatches<'a>) -> &'a Path {
        Path::new(matches.value_of("CONFIG_PATH").unwrap())
    }

    pub fn keychain<'a>(matches: &'a ArgMatches<'a>) -> &'a Path {
        Path::new(matches.value_of("KEYCHAIN").unwrap())
    }

    pub fn execute_default(matches: &ArgMatches) {
        let config_path = Self::config(matches);
        let template_path = Self::template(matches);
        let keychain_path = Self::keychain(matches);

        let template: TemplateConfig = ConfigFile::load(template_path).unwrap();
        let keychain: KeyConfig = ConfigFile::load(keychain_path).unwrap();

        if template.validators.len() != template.count {
            panic!("Template should be full.");
        }

        let genesis = GenesisConfig::new(template.validators.iter().map(|(k,_)| *k));
        let peers = template.validators.iter().map(|(_,ident)| ident.addr).collect();
        let validator_ident = &template.validators[&keychain.public_key];

        let config =  NodeConfig {
            listen_address: validator_ident.addr,
            network: Default::default(),
            peers: peers,
            public_key: keychain.public_key,
            secret_key: keychain.secret_key,
            genesis: genesis,
        };

        ConfigFile::save(&config, config_path).unwrap();
    }

}

pub struct GenerateCommand<'a, 'b>
    where 'a: 'b
{
    _p: PhantomData<App<'a, 'b>>,
}

