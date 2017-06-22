//! This module implement all core commands.

use std::fs;
use std::path::Path;

use helpers::generate_testnet_config;
use config::ConfigFile;

use crypto;

use super::internal::{Command, Feedback};
use super::{Argument, Context, CommandName};

const DEFAULT_EXONUM_LISTEN_PORT: u16 = 6333;
use helpers::clap::{ValidatorIdent, ConfigTemplate, KeyConfig, PubKeyConfig};

pub struct RunCommand;

impl Command for RunCommand {
    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::new_named("NODE_CONFIG_PATH", true, 
            "Path to node configuration file.", "c", "node-config"),
            Argument::new_named("LEVELDB_PATH", true, 
            "Use leveldb database with the given path.", "d", "leveldb"),
            Argument::new_named("PUBLIC_API_ADDRESS", false, 
            "Listen address for public api.", None, "public-api-address"),
            Argument::new_named("PRIVATE_API_ADDRESS", false, 
            "Listen address for private api.", None, "private-api-address"),
        ]
    }

    fn name(&self) -> CommandName {
        "run"
    }

    fn about(&self) -> &str {
        "Run application"
    }

    fn execute(&self, 
               context: Context,
               _: &Fn(Context) -> Context) -> Feedback {
        Feedback::RunNode(context)
    }
}


pub struct KeyGeneratorCommand;

impl Command for KeyGeneratorCommand {
    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::new_positional("KEYCHAIN", true, 
            "Path to key config."),
        ]
    }

    fn name(&self) -> CommandName {
        "keygen"
    }

    fn about(&self) -> &str {
        "Generate node secret and public keys."
    }

    fn execute(&self, 
               context: Context,
               exts: &Fn(Context) -> Context) -> Feedback {
        let (pub_key, sec_key) = crypto::gen_keypair();
        let keyconfig = context.get::<String>("KEYCHAIN")
                              .expect("expected keychain path");
        let keyconfig = Path::new(&keyconfig);

        let pub_key_path = keyconfig.with_extension("pub");

        let new_context = exts(context);
        let services_pub_keys = new_context.get("services_pub_keys");
        let services_sec_keys = new_context.get("services_sec_keys");

        let pub_key_config: PubKeyConfig = PubKeyConfig {
            public_key: pub_key,
            services_pub_keys: services_pub_keys.ok().unwrap_or_default(),
        };
        // save pub_key seperately
        ConfigFile::save(&pub_key_config, &pub_key_path)
                    .expect("Could not write public key file.");

        let config = KeyConfig {
            public_key: pub_key,
            secret_key: sec_key,
            services_sec_keys: services_sec_keys.ok().unwrap_or_default(),
        };

        ConfigFile::save(&config, keyconfig)
                    .expect("Could not write keychain file.");
        Feedback::None
    }
}

/// implements command for template generating
pub struct GenerateTemplateCommand;

impl Command for GenerateTemplateCommand {
    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::new_positional("COUNT", true, 
            "Validator total count."),
            Argument::new_positional("TEMPLATE", true, 
            "Path to template config."),
        ]
    }

    fn name(&self) -> CommandName {
        "generate-template"
    }

    fn about(&self) -> &str {
        "Generate basic config template."
    }

    fn execute(&self, 
               context: Context,
               exts: &Fn(Context) -> Context) -> Feedback {
        let template_path = context.get::<String>("TEMPLATE")
                                   .expect("template not found");
        let template_path = Path::new(&template_path);
        let count = context.get::<String>("COUNT")
                                   .expect("template not found")
                                   .parse()
                                   .expect("expected count to be int");

        let new_context = exts(context);
        let values = new_context.get("VALUES").unwrap_or_default();

        let template = ConfigTemplate {
            count: count,
            services: values,
            ..ConfigTemplate::default()
        };

        ConfigFile::save(&template, template_path)
                        .expect("Could not write template file.");
        Feedback::None
    }
}

/*
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
            Argument::new_named("OUTPUT_DIR", true,
                "Path to directory where save configs.",
                "o", "output_dir"),
            Argument::new_named("START_PORT", false,
                "Port number started from which should validators listen.",
                "p", "start"),
            Argument::new_positional("COUNT", true,
                "Count of validators in testnet."),
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

        let dir = context.get::<String>("OUTPUT_DIR").expect("output dir");
        let count: u8 = context.get::<String>("COUNT")
                                .expect("COUNT")
                                .parse()
                                .expect("count as int");
        let start_port = context.get::<String>("START_PORT")
                                .ok()
                                .map_or(DEFAULT_EXONUM_LISTEN_PORT, 
                                        |v| v.parse::<u16>()
                                             .expect("COUNT as int"));
        
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