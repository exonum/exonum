
use clap::{SubCommand, App, Arg, ArgMatches};

use std::path::Path;
use std::fs;
use std::marker::PhantomData;
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::error::Error;

use config::ConfigFile;
use blockchain::{ConsensusConfig, GenesisConfig};
use node::NodeConfig;
use storage::Storage;
use crypto::{self, PublicKey, SecretKey};
use super::generate_testnet_config;

const DEFAULT_EXONUM_LISTEN_PORT: u16 = 6333;

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
            .map(|s| {
                     s.parse()
                         .expect("Public api address has incorrect format")
                 })
    }

    pub fn private_api_address(matches: &'a ArgMatches<'a>) -> Option<SocketAddr> {
        matches
            .value_of("PRIVATE_API_ADDRESS")
            .map(|s| {
                     s.parse()
                         .expect("Private api address has incorrect format")
                 })
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

#[derive(Serialize, Deserialize)]
pub struct ValidatorIdent {
    pub values: BTreeMap<String, String>,
    addr: SocketAddr,
}

#[derive(Serialize, Deserialize, Default)]
pub struct ConfigTemplate {
    validators: BTreeMap<PublicKey, ValidatorIdent>,
    consensus_cfg: ConsensusConfig,
    count: usize,
    services: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize)]
pub struct KeyConfig {
    public_key: PublicKey,
    secret_key: SecretKey,
}

pub struct KeyGeneratorCommand;

impl KeyGeneratorCommand {
    /// Creates basic keygen subcommand.
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

    /// Generates and writes key config to `keychain()` path.
    pub fn execute_default(matches: &ArgMatches) {
        let (pub_key, sec_key) = crypto::gen_keypair();
        let keyconfig = Self::keychain(matches);
        let pub_key_path = keyconfig.with_extension("pub");

        // save pub_key seperately
        ConfigFile::save(&pub_key, &pub_key_path).unwrap();

        let config = KeyConfig {
            public_key: pub_key,
            secret_key: sec_key,
        };

        ConfigFile::save(&config, Self::keychain(matches)).unwrap();
    }
}

/// implement command for template generating
pub struct GenerateTemplateCommand;
impl GenerateTemplateCommand {
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
    pub fn execute<T>(matches: &ArgMatches, values: T)
        where T: Into<Option<BTreeMap<String, String>>>
    {
        let values = values.into().unwrap_or_default();
        let template = ConfigTemplate {
            count: Self::count(matches),
            services: values,
            ..ConfigTemplate::default()
        };

        ConfigFile::save(&template, Self::template(matches)).unwrap();
    }
}

pub struct AddValidatorCommand;
impl AddValidatorCommand {
    pub fn new<'a>() -> App<'a, 'a> {
        SubCommand::with_name("add-validator")
            .about("Preinit configuration, add validator to config template.")
            .arg(Arg::with_name("TEMPLATE")
                     .help("Path to template")
                     .required(true)
                     .index(1))
            .arg(Arg::with_name("PUBLIC_KEY")
                     .help("Path to public key file.")
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

    /// path to public_key file
    pub fn public_key<'a>(matches: &'a ArgMatches<'a>) -> &'a Path {
        Path::new(matches.value_of("PUBLIC_KEY").unwrap())
    }

    /// path to template config
    pub fn template<'a>(matches: &'a ArgMatches<'a>) -> &'a Path {
        Path::new(matches.value_of("TEMPLATE").unwrap())
    }

    // exonum listen port
    pub fn port(matches: &ArgMatches) -> Option<u16> {
        matches
            .value_of("PORT")
            .and_then(|port| port.parse().ok())
    }

    // exonum listen addr
    pub fn addr(matches: &ArgMatches) -> String {
        matches.value_of("LISTEN_ADDR").unwrap().to_string()
    }


    #[cfg_attr(feature="cargo-clippy", allow(map_entry))]
    pub fn execute<F, X>(matches: &ArgMatches, on_add: F)
        where F: Into<Option<X>>,
              X: FnOnce(usize, &mut BTreeMap<String, String>)
                        -> Result<BTreeMap<String, String>, Box<Error>>
    {
        let template_path = Self::template(matches);
        let public_key_path = Self::public_key(matches);

        let mut template: ConfigTemplate = ConfigFile::load(template_path).unwrap();
        let public_key: PublicKey = ConfigFile::load(public_key_path).unwrap();
        let addr = format!("{}:{}",
                           Self::addr(matches),
                           Self::port(matches).unwrap_or(DEFAULT_EXONUM_LISTEN_PORT))
                .parse()
                .unwrap();
        if !template.validators.contains_key(&public_key) {
            if template.validators.len() >= template.count {
                panic!("This template alredy full.");
            }
            let func = on_add
                .into()
                .map(|f| f(template.count, &mut template.services));
            let map = func.unwrap_or_else(|| Ok(BTreeMap::new()));

            let ident = ValidatorIdent {
                addr: addr,
                values: map.unwrap(),
            };
            template.validators.insert(public_key, ident);
        } else {
            panic!("This node alredy in template");
        }

        ConfigFile::save(&template, template_path).unwrap();
    }
}

pub struct InitCommand;

impl InitCommand {
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

    /// path to full template config
    pub fn template<'a>(matches: &'a ArgMatches<'a>) -> &'a Path {
        Path::new(matches.value_of("FULL_TEMPLATE").unwrap())
    }

    /// path to output config
    pub fn config<'a>(matches: &'a ArgMatches<'a>) -> &'a Path {
        Path::new(matches.value_of("CONFIG_PATH").unwrap())
    }

    /// path to keychain (public and secret keys)
    pub fn keychain<'a>(matches: &'a ArgMatches<'a>) -> &'a Path {
        Path::new(matches.value_of("KEYCHAIN").unwrap())
    }

    pub fn execute<F, X>(matches: &ArgMatches, on_init: F)
        where F: Into<Option<X>>,
              X: FnOnce(&BTreeMap<String, String>,
                        &BTreeMap<PublicKey, ValidatorIdent>)
                        -> Result<(), Box<Error>>
    {
        let config_path = Self::config(matches);
        let template_path = Self::template(matches);
        let keychain_path = Self::keychain(matches);

        let template: ConfigTemplate = ConfigFile::load(template_path).unwrap();
        let keychain: KeyConfig = ConfigFile::load(keychain_path).unwrap();

        if template.validators.len() != template.count {
            panic!("Template should be full.");
        }

        let func = on_init
            .into()
            .map(|f| f(&template.services, &template.validators));
        let func = func.unwrap_or(Ok(()));
        // propogate error
        func.unwrap();

        let genesis = GenesisConfig::new(template.validators.iter().map(|(k, _)| *k));
        let peers = template
            .validators
            .iter()
            .map(|(_, ident)| ident.addr)
            .collect();
        let validator_ident = &template.validators[&keychain.public_key];



        let config = NodeConfig {
            listen_address: validator_ident.addr,
            network: Default::default(),
            peers: peers,
            public_key: keychain.public_key,
            secret_key: keychain.secret_key,
            genesis: genesis,
            api: Default::default(),
        };

        ConfigFile::save(&config, config_path).unwrap();
    }
}

pub struct GenerateTestnetCommand;

impl GenerateTestnetCommand {
    pub fn new<'a>() -> App<'a, 'a> {
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

    pub fn output_dir<'a>(matches: &'a ArgMatches<'a>) -> &'a Path {
        Path::new(matches.value_of("OUTPUT_DIR").unwrap())
    }

    pub fn validators_count(matches: &ArgMatches) -> u8 {
        matches.value_of("COUNT").unwrap().parse().unwrap()
    }

    pub fn start_port(matches: &ArgMatches) -> Option<u16> {
        matches
            .value_of("START_PORT")
            .map(|p| p.parse().unwrap())
    }

    pub fn execute<'a>(matches: &'a ArgMatches<'a>) {
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
