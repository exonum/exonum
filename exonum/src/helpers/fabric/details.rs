#![allow(missing_debug_implementations)]

//! This module implement all core commands.
use toml::Value;

use std::fs;
use std::path::Path;
use std::net::SocketAddr;
use std::collections::BTreeMap;

use blockchain::GenesisConfig;
use helpers::generate_testnet_config;
use helpers::config::ConfigFile;
use node::NodeConfig;
use storage::Database;
use crypto;

use blockchain::config::ValidatorKeys;

use super::internal::{Command, Feedback};
use super::{Argument, Context, CommandName};

use super::shared::{AbstractConfig, NodePublicConfig, SharedConfig,
                     NodePrivateConfig, CommonConfigTemplate};
use super::DEFAULT_EXONUM_LISTEN_PORT;

pub struct Run;

impl Run {

    pub fn name() -> CommandName {
        "run"
    }

    #[cfg(not(feature="memorydb"))]
    pub fn db_helper(ctx: &Context) -> Box<Database> {
        use storage::{LevelDB, LevelDBOptions};

        let path = ctx.arg::<String>("LEVELDB_PATH")
                      .expect("LEVELDB_PATH not found.");
        let mut options = LevelDBOptions::new();
        options.create_if_missing = true;
        Box::new(LevelDB::open(Path::new(&path), options).unwrap())
    }

    #[cfg(feature="memorydb")]
    pub fn db_helper(_: &Context) -> Box<Database> {
        use storage::MemoryDB;
        Box::new(MemoryDB::new())
    }

    fn node_config(ctx: &Context) -> NodeConfig {
        let path = ctx.arg::<String>("NODE_CONFIG_PATH")
                      .expect("NODE_CONFIG_PATH not found.");
        ConfigFile::load(path).unwrap()
    }

    fn public_api_address(ctx: &Context) -> Option<SocketAddr> {
        ctx.arg("PUBLIC_API_ADDRESS").ok()
    }

    fn private_api_address(ctx: &Context) -> Option<SocketAddr> {
        ctx.arg("PRIVATE_API_ADDRESS").ok()
    }
}

impl Command for Run {
    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::new_named("NODE_CONFIG_PATH", true,
            "Path to node configuration file.", "c", "node-config", false),
            Argument::new_named("LEVELDB_PATH", true,
            "Use leveldb database with the given path.", "d", "leveldb", false),
            Argument::new_named("PUBLIC_API_ADDRESS", false,
            "Listen address for public api.", None, "public-api-address", false),
            Argument::new_named("PRIVATE_API_ADDRESS", false,
            "Listen address for private api.", None, "private-api-address", false),
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


/// implements command for template generating
pub struct GenerateCommonConfig;
impl GenerateCommonConfig {
    pub fn name() -> CommandName {
        "generate-template"
    }
}

impl Command for GenerateCommonConfig {
    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::new_positional("COMMON_CONFIG", true,
            "Path to common config."),
        ]
    }

    fn name(&self) -> CommandName {
        Self::name()
    }

    fn about(&self) -> &str {
        "Generate basic config template."
    }

    fn execute(&self,
               mut context: Context,
               exts: &Fn(Context) -> Context) -> Feedback {
        let template_path = context.arg::<String>("COMMON_CONFIG")
                                   .expect("COMMON_CONFIG not found");
                                   
        context.set("services_config", AbstractConfig::default());
        let new_context = exts(context);
        let services_config = new_context.get("services_config").unwrap_or_default();

        let template = CommonConfigTemplate {
            services_config,
            ..CommonConfigTemplate::default()
        };

        ConfigFile::save(&template, template_path)
                        .expect("Could not write template file.");
        Feedback::None
    }
}

pub struct GenerateNodeConfig;

impl GenerateNodeConfig {
    pub fn name() -> CommandName {
        "generate-config"
    }

    fn addr(context: &Context) -> (SocketAddr, SocketAddr) {
        let addr = context.arg::<String>("PEER_ADDR").unwrap_or_default();

        let mut addr_parts = addr.split(':');
        let ip = addr_parts.next().expect("Expected ip address");
        if ip.len() < 8 {
            panic!("Expected ip address in PEER_ADDR.")
        }
        let port = addr_parts.next().map_or(DEFAULT_EXONUM_LISTEN_PORT,
                                                |s| s.parse()
                                                     .expect("could not parse port"));
        let external_addr = format!("{}:{}", ip, port);
        let listen_addr = format!("0.0.0.0:{}", port);
        (external_addr.parse().unwrap(),
            listen_addr.parse().unwrap())
    }
}

impl Command for GenerateNodeConfig {
    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::new_positional("COMMON_CONFIG", true,
            "Path to common config."),
            Argument::new_positional("PUB_CONFIG", true,
            "Path where save public config."),
            Argument::new_positional("SEC_CONFIG", true,
            "Path where save private config."),
            Argument::new_named("PEER_ADDR", true,
            "Remote peer address", "a", "peer-addr", false),
        ]
    }

    fn name(&self) -> CommandName {
        Self::name()
    }

    fn about(&self) -> &str {
        "Generate node secret and public configs."
    }

    fn execute(&self,
               mut context: Context,
               exts: &Fn(Context) -> Context) -> Feedback {
        let common_config_path = context.arg::<String>("COMMON_CONFIG")
                              .expect("expected common config path");
        let pub_config_path = context.arg::<String>("PUB_CONFIG")
                              .expect("expected public config path");
        let priv_config_path = context.arg::<String>("SEC_CONFIG")
                              .expect("expected secret config path");

        
        let addr = Self::addr(&context);
        let common: CommonConfigTemplate = ConfigFile::load(&common_config_path)
                                .expect("Could not load common config");
        context.set("common_config", common.clone());
        context.set("services_public_configs", BTreeMap::<String, Value>::default());
        context.set("services_secret_configs", BTreeMap::<String, Value>::default());
        let new_context = exts(context);
        let services_public_configs = new_context.get("services_public_configs")
                                                 .unwrap();
        let services_secret_configs = new_context.get("services_secret_configs");

        let (consensus_public_key,
                consensus_secret_key) = crypto::gen_keypair();
        let (service_public_key,
                service_secret_key) = crypto::gen_keypair();

        let validator_keys = ValidatorKeys {
            consensus_key: consensus_public_key,
            service_key: service_public_key,
        };
        let node_pub_config =  NodePublicConfig {
            addr: addr.0,
            validator_keys,
            services_public_configs,
        };
        let shared_config = SharedConfig {
            node: node_pub_config,
            common: common
        };
        // save public config seperately
        ConfigFile::save(&shared_config, &pub_config_path)
                    .expect("Could not write public config file.");

        let priv_config = NodePrivateConfig {
            listen_addr: addr.1, 
            consensus_public_key,
            consensus_secret_key,
            service_public_key,
            service_secret_key,
            services_secret_configs: services_secret_configs
                                .expect("services_secret_configs not found after exts call"),
        };

        ConfigFile::save(&priv_config, priv_config_path)
                    .expect("Could not write secret config file.");
        Feedback::None
    }
}

pub struct Finalize;
impl Finalize {
    pub fn name() -> CommandName {
        "finalize"
    }

    pub fn genesis_from_template(template: CommonConfigTemplate,
                            configs: Vec<NodePublicConfig>) -> GenesisConfig {
        GenesisConfig::new_with_consensus(template.consensus_config,
                                        configs
                                            .iter()
                                            .map(|c| c.validator_keys))
    }

    pub fn reduce_configs(public_configs: Vec<SharedConfig>,
                            our_config: &NodePrivateConfig) 
    -> (CommonConfigTemplate, Vec<NodePublicConfig>, NodePublicConfig)
    {
        let mut map =  BTreeMap::new();
        let mut config_iter = public_configs.into_iter();
        let first = config_iter
                        .next()
                        .expect("Expected at least one config in PUBLIC_CONFIGS");
        let common = first.common;
        map.insert(first.node
                        .validator_keys
                        .consensus_key, first.node);

        for config in config_iter {
            if common != config.common {
                panic!("Found config with different common part.");
            };
            if map.insert(config.node
                        .validator_keys
                        .consensus_key, config.node).is_some() {
                panic!("Found duplicate consenus keys in PUBLIC_CONFIGS");
            }
            
        }
        (common, 
            map.iter().map(|(_, &ref c)|c.clone()).collect(),
            map.get(&our_config.consensus_public_key)
               .expect("our key not found in config").clone())
    }
}

impl Command for Finalize {
    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::new_named("PUBLIC_CONFIGS", true,
            "Path to validators public configs",
            "p", "public-configs", true),
            Argument::new_positional("SECRET_CONFIG", true,
            "Path to our secret config."),
            Argument::new_positional("OUTPUT_CONFIG_PATH", true,
            "Path to output node config."),
        ]
    }

    fn name(&self) -> CommandName {
        Self::name()
    }

    fn about(&self) -> &str {
        "Collect public and secret configs into node config."
    }

    fn execute(&self,
               mut context: Context,
               exts: &Fn(Context) -> Context) -> Feedback {
        let public_configs_path = context.arg_multiple::<String>("PUBLIC_CONFIGS")
                                   .expect("keychain path not found");
        let secret_config_path = context.arg::<String>("SECRET_CONFIG")
                                   .expect("config path not found");
        let output_config_path = context.arg::<String>("OUTPUT_CONFIG_PATH")
                                   .expect("config path not found");

        let secret_config: NodePrivateConfig = ConfigFile::load(secret_config_path)
                                                .expect("Failed to load key config.");
        let public_configs: Vec<SharedConfig> = 
                public_configs_path.into_iter()
                                .map(|path| 
                                    ConfigFile::load(path)
                                    .expect("Failed to load validator public config.")
                                ).collect();
        let (common, list, our) = Self::reduce_configs(public_configs, &secret_config);
        let peers = list
            .iter()
            .map(|c| c.addr)
            .collect();

        let genesis = Self::genesis_from_template(common.clone(), list.clone());

        let config = {
            NodeConfig {
                listen_address: secret_config.listen_addr,
                external_address: Some(our.addr),
                network: Default::default(),
                whitelist: Default::default(),
                peers: peers,
                consensus_public_key: secret_config.consensus_public_key,
                consensus_secret_key: secret_config.consensus_secret_key,
                service_public_key: secret_config.service_public_key,
                service_secret_key: secret_config.service_secret_key,
                genesis: genesis,
                api: Default::default(),
                mempool: Default::default(),
                services_configs: Default::default(),
            }
        };

        context.set("public_config_list", list);
        context.set("node_config", config);
        context.set("common_config", common);
        context.set("services_secret_configs", secret_config.services_secret_configs);

        let new_context = exts(context);

        let config: NodeConfig = new_context.get("node_config")
            .expect("Could not create config from template, services return error");
        ConfigFile::save(&config, output_config_path)
                .expect("Could not write config file.");

        Feedback::None
    }
}

pub struct GenerateTestnet;
impl GenerateTestnet {
    pub fn name() -> CommandName {
        "generate-testnet"
    }
}

impl Command for GenerateTestnet {
    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::new_named("OUTPUT_DIR", true,
                "Path to directory where save configs.",
                "o", "output_dir", false),
            Argument::new_named("START_PORT", false,
                "Port number started from which should validators listen.",
                "p", "start", false),
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
               mut context: Context,
               exts: &Fn(Context) -> Context) -> Feedback {

        let dir = context.arg::<String>("OUTPUT_DIR").expect("output dir");
        let count: u8 = context.arg("COUNT")
                                .expect("count as int");
        let start_port = context.arg::<u16>("START_PORT")
                                .unwrap_or(DEFAULT_EXONUM_LISTEN_PORT);

        let dir = Path::new(&dir);
        let dir = dir.join("validators");
        if !dir.exists() {
            fs::create_dir_all(&dir).unwrap();
        }

        let configs = generate_testnet_config(count, start_port);
        context.set("configs", configs);
        let new_context = exts(context);
        let configs: Vec<NodeConfig> = new_context.get("configs")
                      .expect("Couldn't read testnet configs after exts call.");

        for (idx, cfg) in configs.into_iter().enumerate() {
            let file_name = format!("{}.toml", idx);
            ConfigFile::save(&cfg, &dir.join(file_name)).unwrap();
        }

        Feedback::None
    }
}
