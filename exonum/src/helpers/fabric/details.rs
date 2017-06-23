#![allow(missing_debug_implementations)]

//! This module implement all core commands.
use toml::Value;

use std::fs;
use std::path::Path;
use std::net::SocketAddr;
use std::collections::BTreeMap;

use blockchain::GenesisConfig;
use helpers::generate_testnet_config;
use config::ConfigFile;
use node::NodeConfig;
use storage::Storage;
use crypto;

use super::internal::{Command, Feedback};
use super::{Argument, Context, CommandName};

const DEFAULT_EXONUM_LISTEN_PORT: u16 = 6333;
use helpers::clap::{ValidatorIdent, ConfigTemplate, KeyConfig, PubKeyConfig};
// TODO:How to split `NodeConfig`, from services configs?
// We should extend `NodeConfig` type to take services configs aswell.

pub struct RunCommand;

impl RunCommand {

    pub fn name() -> CommandName {
        "run"
    }

    #[cfg(not(feature="memorydb"))]
    pub fn db_helper(ctx: &Context) -> Storage {
        use storage::{LevelDB, LevelDBOptions};

        let path = ctx.get::<String>("LEVELDB_PATH")
                      .expect("LEVELDB_PATH not found.");
        let mut options = LevelDBOptions::new();
        options.create_if_missing = true;
        LevelDB::new(Path::new(&path), options).unwrap()
    }

    #[cfg(feature="memorydb")]
    pub fn db_helper(_: &Context) -> Storage {
        use storage::MemoryDB;
        MemoryDB::new()
    }

    fn node_config(ctx: &Context) -> Value {
        let path = ctx.get::<String>("NODE_CONFIG_PATH")
                      .expect("NODE_CONFIG_PATH not found.");
        ConfigFile::load(Path::new(&path)).unwrap()
    }

    fn public_api_address(ctx: &Context) -> Option<SocketAddr> {
        ctx.get::<String>("PUBLIC_API_ADDRESS").ok()
            .map(|s|
                s.parse()
                 .expect("Public api address has incorrect format"))
    }

    fn private_api_address(ctx: &Context) -> Option<SocketAddr> {
        ctx.get::<String>("PRIVATE_API_ADDRESS").ok()
            .map(|s|
                s.parse()
                 .expect("Public api address has incorrect format"))
    }
}

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
        Self::name()
    }

    fn about(&self) -> &str {
        "Run application"
    }

    fn execute(&self, 
               mut context: Context,
               exts: &Fn(Context) -> Context) -> Feedback {
        let config = Self::node_config(&context);
        let public_addr = Self::public_api_address(&context);
        let private_addr = Self::private_api_address(&context);
        context.set("node_config", config);
        let mut new_context = exts(context);
        let mut config: NodeConfig = new_context
                                    .get("node_config")
                                    .expect("cant load node_config");
        // Override api options
        if let Some(addr) = public_addr {
            config.api.public_api_address = Some(addr);
        }
        if let Some(addr) = private_addr {
            config.api.private_api_address = Some(addr);
        }
        new_context.set("node_config", config);

        Feedback::RunNode(new_context)
    }
}


pub struct KeyGeneratorCommand;

impl KeyGeneratorCommand {
    pub fn name() -> CommandName {
        "keygen"
    }
}

impl Command for KeyGeneratorCommand {
    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::new_positional("KEYCHAIN", true,
            "Path to key config."),
        ]
    }

    fn name(&self) -> CommandName {
        Self::name()
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
impl GenerateTemplateCommand {
    pub fn name() -> CommandName {
        "generate-template"
    }
}

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
        Self::name()
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



/// `add-validator` - append validator to template.
/// Automaticaly share keys from public key config.
pub struct AddValidatorCommand;
impl AddValidatorCommand {
    pub fn name() -> CommandName {
        "add-validator"
    }
}

impl Command for AddValidatorCommand {
    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::new_positional("TEMPLATE", true,
            "Path to template config."),
            Argument::new_positional("PUBLIC_KEY", true,
            "Path to public key file."),
            Argument::new_named("LISTEN_ADDR", true,
            "Path to public key file.", "a", "listen-addr"),
        ]
    }

    fn name(&self) -> CommandName {
        Self::name()
    }

    fn about(&self) -> &str {
        "Preinit configuration, add validator to config template."
    }

    fn execute(&self, 
               mut context: Context,
               exts: &Fn(Context) -> Context) -> Feedback {
        let template_path = context.get::<String>("TEMPLATE")
                                   .expect("template not found");
        let template_path = Path::new(&template_path);
        let public_key_path = context.get::<String>("PUBLIC_KEY")
                                   .expect("public_key path not found");
        let public_key_path = Path::new(&public_key_path);

        let addr = context.get::<String>("LISTEN_ADDR")
                                   .unwrap_or_default();
        let mut addr_parts = addr.split(':');

        let template: ConfigTemplate = ConfigFile::load(template_path).unwrap();
        let public_key_config: PubKeyConfig = ConfigFile::load(public_key_path).unwrap();
        let addr = format!("{}:{}",
                           addr_parts.next().unwrap_or("0.0.0.0"),
                           addr_parts.next().map_or(DEFAULT_EXONUM_LISTEN_PORT, 
                                                    |s| s.parse().expect("could not parse port")))
                .parse()
                .unwrap();
        let template = if !template.validators.contains_key(&public_key_config.public_key) {
            if template.validators.len() >= template.count {
                panic!("This template already full.");
            }

            let ident = ValidatorIdent {
                addr: addr,
                variables: BTreeMap::default(),
                keys: public_key_config.services_pub_keys,
            };
            context.set("validator_ident", ident);
            context.set("template", template);

            let new_context = exts(context);
            let ident = new_context.get("validator_ident").expect("validator_ident not found after call exts");
            let mut template:ConfigTemplate = new_context.get("template").expect("template not found after call exts");
            
            template.validators.insert(public_key_config.public_key, ident);
            template
        } else {
            panic!("This node already in template")
        };

        ConfigFile::save(&template, template_path).unwrap();
        Feedback::None
    }
}

pub struct InitCommand;
impl InitCommand {
    pub fn name() -> CommandName {
        "init"
    }
}

impl Command for InitCommand {
    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::new_positional("FULL_TEMPLATE", true,
            "Path to full template."),
            Argument::new_positional("KEYCHAIN", true,
            "Path to keychain config."),
            Argument::new_positional("CONFIG_PATH", true,
            "Path to output node config."),
        ]
    }

    fn name(&self) -> CommandName {
        Self::name()
    }

    fn about(&self) -> &str {
        "Toolchain to generate configuration."
    }

    fn execute(&self, 
               mut context: Context,
               exts: &Fn(Context) -> Context) -> Feedback {
        let template_path = context.get::<String>("FULL_TEMPLATE")
                                   .expect("template not found");
        let template_path = Path::new(&template_path);
        let keychain_path = context.get::<String>("KEYCHAIN")
                                   .expect("keychain path not found");
        let keychain_path = Path::new(&keychain_path);

        let config_path = context.get::<String>("CONFIG_PATH")
                                   .expect("config path not found");
        let config_path = Path::new(&config_path);

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
        let config = {
            let validator_ident = &template.validators
                                            .get(&keychain.public_key)
                                            .expect("validator not found in template");

            NodeConfig {
                listen_address: validator_ident.addr,
                network: Default::default(),
                whitelist: Default::default(),
                peers: peers,
                public_key: keychain.public_key,
                secret_key: keychain.secret_key,
                genesis: genesis,
                api: Default::default(),
                services_configs: Default::default(),
            }
        };

        context.set("node_config", config);
        context.set("template", template);
        context.set("services_sec_keys", keychain.services_sec_keys);

        let new_context = exts(context);

        let value: Value = new_context.get("node_config")
            .expect("Could not create config from template, services return error");
        ConfigFile::save(&value, config_path)
                .expect("Could not write config file.");
        
        Feedback::None
    }
}

pub struct GenerateTestnetCommand;
impl GenerateTestnetCommand {
    pub fn name() -> CommandName {
        "generate-testnet"
    }
}

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
        Self::name()
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