//! This module implement all core commands.

use std::fs;
use std::path::Path;

use helpers::generate_testnet_config;
use config::ConfigFile;

use super::internal::{Command, Feedback};
use super::{Argument, Context, ArgumentType, CommandName, NamedArgument};
const DEFAULT_EXONUM_LISTEN_PORT: u16 = 6333;
/*
pub struct RunCommand;
impl RunCommand;
{
    pub fn args() -> Vec<Argument> {
        SubCommand::with_name("run")
            .about("Run node with given configuration")
            .arg(Arg::with_name("NODE_CONFIG_PATH")
                     .short("c")
                     .long("node-config")
                     .help("Path to node configuration file")
                     .required(true)
                     .takes_value(true))
            .arg(Arg::with_name("LEVELDB_PATH")
                     .short("d")
                     .long("leveldb-path")
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
    pub variables: BTreeMap<String, Value>,
    keys: BTreeMap<String, Value>,
    addr: SocketAddr,
}

impl ValidatorIdent {

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn keys(&self) -> &BTreeMap<String, Value> {
        &self.keys
    }
}

#[derive(Serialize, Deserialize, Default)]
pub struct ConfigTemplate {
    validators: BTreeMap<PublicKey, ValidatorIdent>,
    consensus_cfg: ConsensusConfig,
    count: usize,
    pub services: BTreeMap<String, Value>,
}

impl ConfigTemplate {
    pub fn count(&self) -> usize {
        self.count
    }

    pub fn validators(&self) -> &BTreeMap<PublicKey, ValidatorIdent> {
        &self.validators
    }

    pub fn consensus_cfg(&self) -> &ConsensusConfig {
        &self.consensus_cfg
    }
}

// toml file could not save array without "field name"
#[derive(Serialize, Deserialize)]
struct PubKeyConfig {
    public_key: PublicKey,
    services_pub_keys: BTreeMap<String, Value>
}

#[derive(Serialize, Deserialize)]
pub struct KeyConfig {
    public_key: PublicKey,
    secret_key: SecretKey,
    services_sec_keys: BTreeMap<String, Value>
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
    pub fn keychain_filee<'a>(matches: &'a ArgMatches<'a>) -> &'a Path {
        Path::new(matches.value_of("KEYCHAIN").unwrap())
    }

    /// Generates and writes key config to `keychain()` path.
    pub fn execute_default(matches: &ArgMatches) {
        Self::execute(matches, None, None)
    }

    /// Generates and writes key config to `keychain()` path.
    /// Append `services_sec_keys` to keychain.
    /// Append `services_pub_keys` to public key config. 
    /// `add-validator` command autmaticaly share public key config.
    pub fn execute<X, Y>(matches: &ArgMatches,
                    services_sec_keys: X,
                    services_pub_keys: Y)
    where X: Into<Option<BTreeMap<String, Value>>>,
          Y: Into<Option<BTreeMap<String, Value>>>
    {
        let (pub_key, sec_key) = crypto::gen_keypair();
        let keyconfig = Self::keychain_filee(matches);
        let pub_key_path = keyconfig.with_extension("pub");
        let pub_key_config: PubKeyConfig = PubKeyConfig {
            public_key: pub_key,
            services_pub_keys: services_pub_keys.into().unwrap_or_default(),
        };
        // save pub_key seperately
        ConfigFile::save(&pub_key_config, &pub_key_path)
                    .expect("Could not write public key file.");

        let config = KeyConfig {
            public_key: pub_key,
            secret_key: sec_key,
            services_sec_keys: services_sec_keys.into().unwrap_or_default(),
        };

        ConfigFile::save(&config, Self::keychain_filee(matches))
                    .expect("Could not write keychain file.");
    }
}

/// implements command for template generating
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
    pub fn template_file_path<'a>(matches: &'a ArgMatches<'a>) -> &'a Path {
        Path::new(matches.value_of("TEMPLATE").unwrap())
    }
    /// Validator total count
    pub fn validator_count(matches: &ArgMatches) -> usize {
        matches.value_of("COUNT").unwrap().parse().unwrap()
    }

    /// Write default template config into `template()` path.
    pub fn execute_default(matches: &ArgMatches) {
        Self::execute(matches, None)
    }
    /// Write default template config into `template()` path.
    /// You can append some values to template as second argument.
    pub fn execute<T>(matches: &ArgMatches, values: T)
        where T: Into<Option<BTreeMap<String, Value>>>
    {
        let values = values.into().unwrap_or_default();
        let template = ConfigTemplate {
            count: Self::validator_count(matches),
            services: values,
            ..ConfigTemplate::default()
        };

        ConfigFile::save(&template, Self::template_file_path(matches))
                        .expect("Could not write template file.");
    }
}

/// `add-validator` - append validator to template.
/// Automaticaly share keys from public key config.
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
                     .required(true)
                     .takes_value(true))

    }

    /// path to public_key file
    pub fn public_key_file_path<'a>(matches: &'a ArgMatches<'a>) -> &'a Path {
        Path::new(matches.value_of("PUBLIC_KEY").unwrap())
    }

    /// path to template config
    pub fn template_file_path<'a>(matches: &'a ArgMatches<'a>) -> &'a Path {
        Path::new(matches.value_of("TEMPLATE").unwrap())
    }

    // exonum listen addr
    pub fn listen_addr(matches: &ArgMatches) -> String {
        matches.value_of("LISTEN_ADDR").unwrap().to_string()
    }

    /// Add validator to template config.
    pub fn execute_default(matches: &ArgMatches) {
        Self::execute(matches, |_, _| Ok(()))
    }

    #[cfg_attr(feature="cargo-clippy", allow(map_entry))]
    pub fn execute<F>(matches: &ArgMatches, on_add: F)
        where F: FnOnce(&mut ValidatorIdent, &mut ConfigTemplate)
                        -> Result<(), Box<Error>>,
    {
        let template_path = Self::template_file_path(matches);
        let public_key_path = Self::public_key_file_path(matches);
        let addr = Self::listen_addr(matches);
        let mut addr_parts = addr.split(':');

        let mut template: ConfigTemplate = ConfigFile::load(template_path).unwrap();
        let public_key_config: PubKeyConfig = ConfigFile::load(public_key_path).unwrap();
        let addr = format!("{}:{}",
                           addr_parts.next().expect("expected ip addr"),
                           addr_parts.next().map_or(DEFAULT_EXONUM_LISTEN_PORT, 
                                                    |s| s.parse().expect("could not parse port")))
                .parse()
                .unwrap();
        if !template.validators.contains_key(&public_key_config.public_key) {
            if template.validators.len() >= template.count {
                panic!("This template already full.");
            }

            let mut ident = ValidatorIdent {
                addr: addr,
                variables: BTreeMap::default(),
                keys: public_key_config.services_pub_keys,
            };

            on_add(&mut ident, &mut template)
                  .expect("could not add validator, service return");

            template.validators.insert(public_key_config.public_key, ident);
        } else {
            panic!("This node already in template");
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

    /// Add validator to template config.
    pub fn execute_default(matches: &ArgMatches) {
        Self::execute(matches, |config, _, _| Ok(Value::try_from(config)? ))
    }

    pub fn execute<F>(matches: &ArgMatches, on_init: F)
        where F: FnOnce(NodeConfig, &ConfigTemplate, &BTreeMap<String, Value>)
                        -> Result<Value, Box<Error>>
    {
        let config_path = Self::config(matches);
        let template_path = Self::template(matches);
        let keychain_path = Self::keychain(matches);

        let template: ConfigTemplate = ConfigFile::load(template_path)
                                                .expect("Failed to load config template.");
        let keychain: KeyConfig = ConfigFile::load(keychain_path)
                                                .expect("Failed to load key config.");

        if template.validators.len() != template.count {
            panic!("Template should be full.");
        }

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
        let value = on_init(config, &template, &keychain.services_sec_keys)
            .expect("Could not create config from template, services return error");
        ConfigFile::save(&value, config_path)
                .expect("Could not write config file.");
        
    }
}

*/


pub struct GenerateTestnetCommand;

impl Command for GenerateTestnetCommand {
    fn args(&self) -> Vec<Argument> {
        vec![
            Argument {
                name: "OUTPUT_DIR",
                argument: ArgumentType::Named (
                    NamedArgument { 
                        short_name: "o",
                        long_name: "output_dir",
                    }
                ),
                required: true,
                help: "Path to directory where save configs.",
            },
            Argument {
                name: "START_PORT",
                argument: ArgumentType::Named (
                    NamedArgument { 
                        short_name: "p",
                        long_name: "start-port",
                    }
                ),
                required: false,
                help: "Port number started from which should validators listen.",
            },
            Argument {
                name: "COUNT",
                argument: ArgumentType::Positional,
                required: true,
                help: "Count of validators in testnet.",
            }
        ]
    }

    fn name(&self) -> CommandName {
        "generate-testnet"
    }

    fn about(&self) -> &str {
        "Generates genesis configuration for testnet"
    }

    fn execute(&self, 
               context: Context,
               _: &Fn(Context) -> Context) -> Feedback {

        let dir = context.get::<String>("OUTPUT_DIR").unwrap();
        let count = context.get::<u8>("COUNT").unwrap();
        let start_port = context.get::<u16>("START_PORT")
                                .unwrap_or_else(|| DEFAULT_EXONUM_LISTEN_PORT);
        
        let dir = Path::new(&dir);
        let dir = dir.join("validators");
        if !dir.exists() {
            fs::create_dir_all(&dir).unwrap();
        }

        let configs = generate_testnet_config(count, start_port);
        for (idx, cfg) in configs.into_iter().enumerate() {
            let file_name = format!("{}.toml", idx);
            ConfigFile::save(&cfg, &dir.join(file_name)).unwrap();
        }

        Feedback::None
    }
}