// Copyright 2019 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![allow(missing_debug_implementations)]

//! This module implement all core commands.
// spell-checker:ignore exts, rsplitn

use std::{
    collections::{BTreeMap, HashMap},
    fs,
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
};

use super::{
    super::path_relative_from,
    internal::{CollectedCommand, Command, Feedback},
    keys,
    password::{PassInputMethod, SecretKeyType},
    shared::{
        AbstractConfig, CommonConfigTemplate, NodePrivateConfig, NodePublicConfig, NodeRunConfig,
        SharedConfig,
    },
    Argument, CommandName, Context, DEFAULT_EXONUM_LISTEN_PORT,
};
use crate::api::backends::actix::AllowOrigin;
use crate::blockchain::{config::ValidatorKeys, GenesisConfig};
use crate::crypto::{generate_keys_file, PublicKey};
use crate::helpers::{config::ConfigFile, generate_testnet_config, ZeroizeOnDrop};
use crate::node::{ConnectListConfig, NodeApiConfig, NodeConfig};
use crate::storage::{Database, DbOptions, RocksDB};

const DATABASE_PATH: &str = "DATABASE_PATH";
const OUTPUT_DIR: &str = "OUTPUT_DIR";
const PEER_ADDRESS: &str = "PEER_ADDRESS";
const LISTEN_ADDRESS: &str = "LISTEN_ADDRESS";
const NODE_CONFIG_PATH: &str = "NODE_CONFIG_PATH";
const PUBLIC_API_ADDRESS: &str = "PUBLIC_API_ADDRESS";
const PRIVATE_API_ADDRESS: &str = "PRIVATE_API_ADDRESS";
const PUBLIC_ALLOW_ORIGIN: &str = "PUBLIC_ALLOW_ORIGIN";
const PRIVATE_ALLOW_ORIGIN: &str = "PRIVATE_ALLOW_ORIGIN";
const CONSENSUS_KEY_PATH: &str = "CONSENSUS_KEY_PATH";
const SERVICE_KEY_PATH: &str = "SERVICE_KEY_PATH";
const NO_PASSWORD: &str = "NO_PASSWORD";
const CONSENSUS_KEY_PASS_METHOD: &str = "CONSENSUS_KEY_PASS_METHOD";
const SERVICE_KEY_PASS_METHOD: &str = "SERVICE_KEY_PASS_METHOD";

/// Run command.
pub struct Run;

impl Run {
    /// Returns created database instance.
    pub fn db_helper(ctx: &Context, options: &DbOptions) -> Box<dyn Database> {
        let path = ctx
            .arg::<String>(DATABASE_PATH)
            .unwrap_or_else(|_| panic!("{} not found.", DATABASE_PATH));
        Box::new(RocksDB::open(Path::new(&path), options).expect("Can't load database file"))
    }

    fn node_config_path(ctx: &Context) -> String {
        ctx.arg::<String>(NODE_CONFIG_PATH)
            .unwrap_or_else(|_| panic!("{} not found.", NODE_CONFIG_PATH))
    }

    fn node_config(path: String) -> NodeConfig<PathBuf> {
        ConfigFile::load(path).expect("Can't load node config file")
    }

    fn public_api_address(ctx: &Context) -> Option<SocketAddr> {
        ctx.arg(PUBLIC_API_ADDRESS).ok()
    }

    fn private_api_address(ctx: &Context) -> Option<SocketAddr> {
        ctx.arg(PRIVATE_API_ADDRESS).ok()
    }

    fn pass_input_method(ctx: &Context, key_type: SecretKeyType) -> String {
        let arg_key = match key_type {
            SecretKeyType::Consensus => CONSENSUS_KEY_PASS_METHOD,
            SecretKeyType::Service => SERVICE_KEY_PASS_METHOD,
        };
        ctx.arg(arg_key).unwrap_or_default()
    }
}

impl Command for Run {
    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::new_named(
                NODE_CONFIG_PATH,
                true,
                "Path to node configuration file.",
                "c",
                "node-config",
                false,
            ),
            Argument::new_named(
                DATABASE_PATH,
                true,
                "Use database with the given path.",
                "d",
                "db-path",
                false,
            ),
            Argument::new_named(
                PUBLIC_API_ADDRESS,
                false,
                "Listen address for public api.",
                None,
                "public-api-address",
                false,
            ),
            Argument::new_named(
                PRIVATE_API_ADDRESS,
                false,
                "Listen address for private api.",
                None,
                "private-api-address",
                false,
            ),
            Argument::new_named(
                CONSENSUS_KEY_PASS_METHOD,
                false,
                "Passphrase entry method for consensus key.\n\
                 Possible values are: stdin, env{:ENV_VAR_NAME}, pass:PASSWORD (default: stdin)\n\
                 If ENV_VAR_NAME is not specified $EXONUM_CONSENSUS_PASS is used",
                None,
                "consensus-key-pass",
                false,
            ),
            Argument::new_named(
                SERVICE_KEY_PASS_METHOD,
                false,
                "Passphrase entry method for service key.\n\
                 Possible values are: stdin, env{:ENV_VAR_NAME}, pass:PASSWORD (default: stdin)\n\
                 If ENV_VAR_NAME is not specified $EXONUM_SERVICE_PASS is used",
                None,
                "service-key-pass",
                false,
            ),
        ]
    }

    fn name(&self) -> CommandName {
        "run"
    }

    fn about(&self) -> &str {
        "Run application"
    }

    fn execute(
        &self,
        _commands: &HashMap<CommandName, CollectedCommand>,
        mut context: Context,
        exts: &dyn Fn(Context) -> Context,
    ) -> Feedback {
        let config_path = Self::node_config_path(&context);

        let config = Self::node_config(config_path.clone());
        let public_addr = Self::public_api_address(&context);
        let private_addr = Self::private_api_address(&context);

        context.set(keys::NODE_CONFIG, config);
        context.set(keys::NODE_CONFIG_PATH, config_path);
        let mut new_context = exts(context);
        let mut config = new_context
            .get(keys::NODE_CONFIG)
            .expect("cant load node_config");
        // Override api options
        if let Some(public_addr) = public_addr {
            config.api.public_api_address = Some(public_addr);
        }

        if let Some(private_api_address) = private_addr {
            config.api.private_api_address = Some(private_api_address);
        }

        new_context.set(keys::NODE_CONFIG, config);

        let run_config = {
            let consensus_pass_method =
                Run::pass_input_method(&new_context, SecretKeyType::Consensus);
            let service_pass_method = Run::pass_input_method(&new_context, SecretKeyType::Service);
            NodeRunConfig {
                consensus_pass_method,
                service_pass_method,
            }
        };
        new_context.set(keys::RUN_CONFIG, run_config);

        Feedback::RunNode(new_context)
    }
}

/// Command for running service in dev mode.
pub struct RunDev;

impl RunDev {
    fn artifacts_directory(ctx: &Context) -> PathBuf {
        let directory = ctx
            .arg::<String>("ARTIFACTS_DIR")
            .unwrap_or_else(|_| ".exonum".into());
        PathBuf::from(&directory)
    }

    fn artifacts_path(inner_path: &str, ctx: &Context) -> String {
        let mut path = Self::artifacts_directory(ctx);
        path.push(inner_path);
        path.to_str().expect("Expected correct path").into()
    }

    fn set_config_command_arguments(ctx: &mut Context) {
        let common_config_path = Self::artifacts_path("common.toml", &ctx);
        let validators_count = "1";
        let peer_addr = "127.0.0.1";
        let pub_config_path = Self::artifacts_path("public.toml", &ctx);
        let sec_config_path = Self::artifacts_path("secret.toml", &ctx);
        let output_config_path = Self::artifacts_path("output.toml", &ctx);
        let consensus_key_path = Self::artifacts_path("consensus.toml", &ctx);
        let service_key_path = Self::artifacts_path("service.toml", &ctx);

        // Arguments for common config command.
        ctx.set_arg("COMMON_CONFIG", common_config_path.clone());
        ctx.set_arg("VALIDATORS_COUNT", validators_count.into());

        // Arguments for node config command.
        ctx.set_arg("COMMON_CONFIG", common_config_path.clone());
        ctx.set_arg("PUB_CONFIG", pub_config_path.clone());
        ctx.set_arg("SEC_CONFIG", sec_config_path.clone());
        ctx.set_arg(PEER_ADDRESS, peer_addr.into());
        ctx.set_arg(CONSENSUS_KEY_PATH, consensus_key_path);
        ctx.set_arg(SERVICE_KEY_PATH, service_key_path);
        ctx.set_flag_occurrences(NO_PASSWORD, 1);

        // Arguments for finalize config command.
        ctx.set_arg_multiple("PUBLIC_CONFIGS", vec![pub_config_path.clone()]);
        ctx.set_arg(PUBLIC_API_ADDRESS, "127.0.0.1:8080".to_string());
        ctx.set_arg(PRIVATE_API_ADDRESS, "127.0.0.1:8081".to_string());
        ctx.set_arg(
            PUBLIC_ALLOW_ORIGIN,
            "http://127.0.0.1, http://localhost".to_string(),
        );
        ctx.set_arg(
            PRIVATE_ALLOW_ORIGIN,
            "http://127.0.0.1, http://localhost".to_string(),
        );
        ctx.set_arg("SECRET_CONFIG", sec_config_path.clone());
        ctx.set_arg("OUTPUT_CONFIG_PATH", output_config_path.clone());

        // Arguments for run command.
        ctx.set_arg(NODE_CONFIG_PATH, output_config_path.clone());
        ctx.set_arg(CONSENSUS_KEY_PASS_METHOD, "pass:".to_owned());
        ctx.set_arg(SERVICE_KEY_PASS_METHOD, "pass:".to_owned());
    }

    fn generate_config(commands: &HashMap<CommandName, CollectedCommand>, ctx: Context) -> Context {
        let common_config_command = commands
            .get(GenerateCommonConfig.name())
            .expect("Expected GenerateCommonConfig in the commands list.");
        common_config_command.execute(commands, ctx.clone());

        let node_config_command = commands
            .get(GenerateNodeConfig.name())
            .expect("Expected GenerateNodeConfig in the commands list.");
        node_config_command.execute(commands, ctx.clone());

        let finalize_command = commands
            .get(Finalize.name())
            .expect("Expected Finalize in the commands list.");
        finalize_command.execute(commands, ctx.clone());

        ctx
    }

    fn cleanup(ctx: &Context) {
        let database_dir_path = ctx
            .arg::<String>(DATABASE_PATH)
            .expect("Expected DATABASE_PATH being set.");
        let database_dir = Path::new(&database_dir_path);
        if database_dir.exists() {
            fs::remove_dir_all(Self::artifacts_directory(ctx))
                .expect("Expected DATABASE_PATH folder being removable.");
        }
    }
}

impl Command for RunDev {
    fn args(&self) -> Vec<Argument> {
        vec![Argument::new_named(
            "ARTIFACTS_DIR",
            false,
            "The path where configuration and db files will be generated.",
            "a",
            "artifacts-dir",
            false,
        )]
    }

    fn name(&self) -> CommandName {
        "run-dev"
    }

    fn about(&self) -> &str {
        "Run application in development mode (generate configuration and db files automatically)"
    }

    fn execute(
        &self,
        commands: &HashMap<CommandName, CollectedCommand>,
        mut context: Context,
        exts: &dyn Fn(Context) -> Context,
    ) -> Feedback {
        let db_path = Self::artifacts_path("db", &context);
        context.set_arg(DATABASE_PATH, db_path);
        Self::cleanup(&context);

        Self::set_config_command_arguments(&mut context);
        let context = exts(context);
        let context = Self::generate_config(commands, context);

        commands
            .get(Run.name())
            .expect("Expected Run in the commands list.")
            .execute(commands, context)
    }
}

/// Command for the template generation.
pub struct GenerateCommonConfig;

impl Command for GenerateCommonConfig {
    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::new_positional("COMMON_CONFIG", true, "Path to common config."),
            Argument::new_named(
                "VALIDATORS_COUNT",
                true,
                "Number of validators",
                None,
                "validators-count",
                false,
            ),
        ]
    }

    fn name(&self) -> CommandName {
        "generate-template"
    }

    fn about(&self) -> &str {
        "Generate basic config template."
    }

    fn execute(
        &self,
        _commands: &HashMap<CommandName, CollectedCommand>,
        mut context: Context,
        exts: &dyn Fn(Context) -> Context,
    ) -> Feedback {
        let template_path = context
            .arg::<String>("COMMON_CONFIG")
            .expect("COMMON_CONFIG not found");

        let validators_count = context
            .arg::<u16>("VALIDATORS_COUNT")
            .expect("VALIDATORS_COUNT not found");

        context.set(keys::SERVICES_CONFIG, AbstractConfig::default());
        let new_context = exts(context);
        let services_config = new_context.get(keys::SERVICES_CONFIG).unwrap_or_default();

        let mut general_config = AbstractConfig::default();
        general_config.insert(
            String::from("validators_count"),
            u32::from(validators_count).into(),
        );

        let template = CommonConfigTemplate {
            services_config,
            general_config,
            ..CommonConfigTemplate::default()
        };

        ConfigFile::save(&template, template_path).expect("Could not write template file.");
        Feedback::None
    }
}

/// Command for the node configuration generation.
pub struct GenerateNodeConfig;

impl GenerateNodeConfig {
    fn addresses(context: &Context) -> (String, SocketAddr) {
        let external_address_str = &context.arg::<String>(PEER_ADDRESS).unwrap_or_default();
        let listen_address_str = &context.arg::<String>(LISTEN_ADDRESS).ok();

        // Try case where peer external address is socket address or ip address.
        let external_address_socket = external_address_str.parse().or_else(|_| {
            external_address_str
                .parse()
                .map(|ip| SocketAddr::new(ip, DEFAULT_EXONUM_LISTEN_PORT))
        });

        let (external_address, external_port) = if let Ok(addr) = external_address_socket {
            (addr.to_string(), addr.port())
        } else {
            let port = external_address_str
                .rsplitn(2, ':')
                .next()
                .and_then(|p| p.parse::<u16>().ok());
            if let Some(port) = port {
                (external_address_str.clone(), port)
            } else {
                let port = DEFAULT_EXONUM_LISTEN_PORT;
                (format!("{}:{}", external_address_str, port), port)
            }
        };

        let listen_address: SocketAddr = listen_address_str.as_ref().map_or_else(
            || {
                let listen_ip = match external_address_socket {
                    Ok(addr) => match addr.ip() {
                        IpAddr::V4(_) => "0.0.0.0".parse().unwrap(),
                        IpAddr::V6(_) => "::".parse().unwrap(),
                    },
                    Err(_) => "0.0.0.0".parse().unwrap(),
                };
                SocketAddr::new(listen_ip, external_port)
            },
            |a| {
                a.parse().unwrap_or_else(|_| {
                    panic!(
                        "Correct socket address is expected for {}: {:?}",
                        LISTEN_ADDRESS, a
                    )
                })
            },
        );

        (external_address, listen_address)
    }

    fn get_passphrase(
        context: &Context,
        method: PassInputMethod,
        secret_key_type: SecretKeyType,
    ) -> ZeroizeOnDrop<String> {
        if context.get_flag_occurrences(NO_PASSWORD).is_some() {
            ZeroizeOnDrop::default()
        } else {
            method.get_passphrase(secret_key_type, false)
        }
    }
}

impl Command for GenerateNodeConfig {
    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::new_positional("COMMON_CONFIG", true, "Path to common config."),
            Argument::new_positional("PUB_CONFIG", true, "Path where save public config."),
            Argument::new_positional("SEC_CONFIG", true, "Path where save private config."),
            Argument::new_named(
                PEER_ADDRESS,
                true,
                "Remote peer address",
                "a",
                "peer-address",
                false,
            ),
            Argument::new_named(
                LISTEN_ADDRESS,
                false,
                "Listen address",
                "l",
                "listen-address",
                false,
            ),
            Argument::new_named(
                CONSENSUS_KEY_PATH,
                false,
                "Path to the file storing consensus private key (default: ./consensus.toml)",
                "c",
                "consensus-path",
                false,
            ),
            Argument::new_named(
                SERVICE_KEY_PATH,
                false,
                "Path to the file storing service private key (default: ./service.toml)",
                "s",
                "service-path",
                false,
            ),
            Argument::new_flag(
                NO_PASSWORD,
                "Don't prompt for passwords when generating private keys (leave empty)",
                "n",
                "no-password",
                false,
            ),
            Argument::new_named(
                CONSENSUS_KEY_PASS_METHOD,
                false,
                "Passphrase entry method for consensus key.\n\
                 Possible values are: stdin, env{:ENV_VAR_NAME}, pass:PASSWORD (default: stdin)\n\
                 If ENV_VAR_NAME is not specified $EXONUM_CONSENSUS_PASS is used",
                None,
                "consensus-key-pass",
                false,
            ),
            Argument::new_named(
                SERVICE_KEY_PASS_METHOD,
                false,
                "Passphrase entry method for service key.\n\
                 Possible values are: stdin, env{:ENV_VAR_NAME}, pass:PASSWORD (default: stdin)\n\
                 If ENV_VAR_NAME is not specified $EXONUM_SERVICE_PASS is used",
                None,
                "service-key-pass",
                false,
            ),
        ]
    }

    fn name(&self) -> CommandName {
        "generate-config"
    }

    fn about(&self) -> &str {
        "Generate node secret and public configs."
    }

    fn execute(
        &self,
        _commands: &HashMap<CommandName, CollectedCommand>,
        mut context: Context,
        exts: &dyn Fn(Context) -> Context,
    ) -> Feedback {
        let common_config_path = context
            .arg::<String>("COMMON_CONFIG")
            .expect("expected common config path");
        let pub_config_path = context
            .arg::<String>("PUB_CONFIG")
            .expect("expected public config path");
        let private_config_path = context
            .arg::<String>("SEC_CONFIG")
            .expect("expected secret config path");
        let consensus_secret_key_path: PathBuf = context
            .arg::<String>(CONSENSUS_KEY_PATH)
            .unwrap_or_else(|_| "consensus.toml".into())
            .into();
        let service_secret_key_path: PathBuf = context
            .arg::<String>(SERVICE_KEY_PATH)
            .unwrap_or_else(|_| "service.toml".into())
            .into();
        let consensus_key_pass_method: PassInputMethod = context
            .arg::<String>(CONSENSUS_KEY_PASS_METHOD)
            .unwrap_or_default()
            .parse()
            .expect("expected correct passphrase input method for consensus key");
        let service_key_pass_method: PassInputMethod = context
            .arg::<String>(SERVICE_KEY_PASS_METHOD)
            .unwrap_or_default()
            .parse()
            .expect("expected correct passphrase input method for service key");

        let addresses = Self::addresses(&context);
        let common: CommonConfigTemplate =
            ConfigFile::load(&common_config_path).expect("Could not load common config");
        context.set(keys::COMMON_CONFIG, common.clone());
        context.set(
            keys::SERVICES_PUBLIC_CONFIGS,
            BTreeMap::<String, toml::Value>::default(),
        );
        context.set(
            keys::SERVICES_SECRET_CONFIGS,
            BTreeMap::<String, toml::Value>::default(),
        );
        let new_context = exts(context);
        let services_public_configs = new_context.get(keys::SERVICES_PUBLIC_CONFIGS).unwrap();
        let services_secret_configs = new_context.get(keys::SERVICES_SECRET_CONFIGS);

        let consensus_public_key = {
            let passphrase = Self::get_passphrase(
                &new_context,
                consensus_key_pass_method,
                SecretKeyType::Consensus,
            );
            create_secret_key_file(&consensus_secret_key_path, passphrase.as_bytes())
        };
        let service_public_key = {
            let passphrase = Self::get_passphrase(
                &new_context,
                service_key_pass_method,
                SecretKeyType::Service,
            );
            create_secret_key_file(&service_secret_key_path, passphrase.as_bytes())
        };

        let pub_config_dir = Path::new(&pub_config_path)
            .parent()
            .expect("Cannot get directory with configuration file");
        let consensus_secret_key = if consensus_secret_key_path.is_absolute() {
            consensus_secret_key_path
        } else {
            path_relative_from(&consensus_secret_key_path, &pub_config_dir).unwrap()
        };
        let service_secret_key = if service_secret_key_path.is_absolute() {
            service_secret_key_path
        } else {
            path_relative_from(&service_secret_key_path, &pub_config_dir).unwrap()
        };

        let validator_keys = ValidatorKeys {
            consensus_key: consensus_public_key,
            service_key: service_public_key,
        };
        let node_pub_config = NodePublicConfig {
            address: addresses.0.clone(),
            validator_keys,
            services_public_configs,
        };
        let shared_config = SharedConfig {
            node: node_pub_config,
            common,
        };
        // Save public config separately.
        ConfigFile::save(&shared_config, &pub_config_path)
            .expect("Could not write public config file.");

        let private_config = NodePrivateConfig {
            listen_address: addresses.1,
            external_address: addresses.0.clone(),
            consensus_public_key,
            consensus_secret_key,
            service_public_key,
            service_secret_key,
            services_secret_configs: services_secret_configs
                .expect("services_secret_configs not found after exts call"),
        };

        ConfigFile::save(&private_config, private_config_path)
            .expect("Could not write secret config file.");
        Feedback::None
    }
}

/// Finalize command.
pub struct Finalize;

impl Finalize {
    /// Returns `GenesisConfig` from the template.
    fn genesis_from_template(
        template: CommonConfigTemplate,
        configs: &[NodePublicConfig],
    ) -> GenesisConfig {
        GenesisConfig::new_with_consensus(
            template.consensus_config,
            configs.iter().map(|c| c.validator_keys),
        )
    }

    fn reduce_configs(
        public_configs: Vec<SharedConfig>,
        our_config: &NodePrivateConfig,
    ) -> (
        CommonConfigTemplate,
        Vec<NodePublicConfig>,
        Option<NodePublicConfig>,
    ) {
        let mut map = BTreeMap::new();
        let mut config_iter = public_configs.into_iter();
        let first = config_iter
            .next()
            .expect("Expected at least one config in PUBLIC_CONFIGS");
        let common = first.common;
        map.insert(first.node.validator_keys.consensus_key, first.node);

        for config in config_iter {
            if common != config.common {
                panic!("Found config with different common part.");
            };
            if map
                .insert(config.node.validator_keys.consensus_key, config.node)
                .is_some()
            {
                panic!("Found duplicate consensus keys in PUBLIC_CONFIGS");
            }
        }
        (
            common,
            map.iter().map(|(_, c)| c.clone()).collect(),
            map.get(&our_config.consensus_public_key).cloned(),
        )
    }

    fn public_allow_origin(context: &Context) -> Option<AllowOrigin> {
        context.arg(PUBLIC_ALLOW_ORIGIN).ok()
    }

    fn private_allow_origin(context: &Context) -> Option<AllowOrigin> {
        context.arg(PRIVATE_ALLOW_ORIGIN).ok()
    }
}

impl Command for Finalize {
    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::new_named(
                "PUBLIC_CONFIGS",
                true,
                "Path to validators public configs",
                "p",
                "public-configs",
                true,
            ),
            Argument::new_named(
                PUBLIC_API_ADDRESS,
                false,
                "Listen address for public api.",
                None,
                "public-api-address",
                false,
            ),
            Argument::new_named(
                PRIVATE_API_ADDRESS,
                false,
                "Listen address for private api.",
                None,
                "private-api-address",
                false,
            ),
            Argument::new_named(
                PUBLIC_ALLOW_ORIGIN,
                false,
                "Cross-origin resource sharing options for responses returned by public API handlers.",
                None,
                "public-allow-origin",
                false,
            ),
            Argument::new_named(
                PRIVATE_ALLOW_ORIGIN,
                false,
                "Cross-origin resource sharing options for responses returned by private API handlers.",
                None,
                "private-allow-origin",
                false,
            ),
            Argument::new_positional("SECRET_CONFIG", true, "Path to our secret config."),
            Argument::new_positional("OUTPUT_CONFIG_PATH", true, "Path to output node config."),
        ]
    }

    fn name(&self) -> CommandName {
        "finalize"
    }

    fn about(&self) -> &str {
        "Collect public and secret configs into node config."
    }

    fn execute(
        &self,
        _commands: &HashMap<CommandName, CollectedCommand>,
        mut context: Context,
        exts: &dyn Fn(Context) -> Context,
    ) -> Feedback {
        let public_configs_path = context
            .arg_multiple::<String>("PUBLIC_CONFIGS")
            .expect("public config path not found");
        let secret_config_path = context
            .arg::<String>("SECRET_CONFIG")
            .expect("private config path not found");
        let output_config_path = context
            .arg::<String>("OUTPUT_CONFIG_PATH")
            .expect("output config path not found");

        let public_api_address = Run::public_api_address(&context);
        let private_api_address = Run::private_api_address(&context);
        let public_allow_origin = Self::public_allow_origin(&context);
        let private_allow_origin = Self::private_allow_origin(&context);

        let secret_config: NodePrivateConfig =
            ConfigFile::load(secret_config_path).expect("Failed to load key config.");
        let public_configs: Vec<SharedConfig> = public_configs_path
            .into_iter()
            .map(|path| ConfigFile::load(path).expect("Failed to load validator public config."))
            .collect();

        let (common, list, our) = Self::reduce_configs(public_configs, &secret_config);

        let validators_count = common
            .general_config
            .get("validators_count")
            .expect("validators_count not found in common config.")
            .as_integer()
            .unwrap() as usize;

        if validators_count != list.len() {
            panic!(
                "The number of validators configs does not match the number of validators keys."
            );
        }

        context.set(keys::AUDITOR_MODE, our.is_none());

        let genesis = Self::genesis_from_template(common.clone(), &list);

        let connect_list = ConnectListConfig::from_node_config(&list, &secret_config);

        let config = {
            NodeConfig {
                listen_address: secret_config.listen_address,
                external_address: secret_config.external_address,
                network: Default::default(),
                consensus_public_key: secret_config.consensus_public_key,
                consensus_secret_key: secret_config.consensus_secret_key,
                service_public_key: secret_config.service_public_key,
                service_secret_key: secret_config.service_secret_key,
                genesis,
                api: NodeApiConfig {
                    public_api_address,
                    private_api_address,
                    public_allow_origin,
                    private_allow_origin,
                    ..Default::default()
                },
                mempool: Default::default(),
                services_configs: Default::default(),
                database: Default::default(),
                connect_list,
                thread_pool_size: Default::default(),
            }
        };

        context.set(keys::PUBLIC_CONFIG_LIST, list);
        context.set(keys::NODE_CONFIG, config);
        context.set(keys::COMMON_CONFIG, common);
        context.set(
            keys::SERVICES_SECRET_CONFIGS,
            secret_config.services_secret_configs,
        );

        let new_context = exts(context);

        let config = new_context
            .get(keys::NODE_CONFIG)
            .expect("Could not create config from template, services return error");
        ConfigFile::save(&config, output_config_path).expect("Could not write config file.");

        Feedback::None
    }
}

/// Command for the testnet generation.
pub struct GenerateTestnet;

impl Command for GenerateTestnet {
    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::new_named(
                OUTPUT_DIR,
                true,
                "Path to directory where save configs.",
                "o",
                "output-dir",
                false,
            ),
            Argument::new_named(
                "START_PORT",
                false,
                "Port number started from which should validators listen.",
                "p",
                "start",
                false,
            ),
            Argument::new_positional("COUNT", true, "Count of validators in testnet."),
        ]
    }

    fn name(&self) -> CommandName {
        "generate-testnet"
    }

    fn about(&self) -> &str {
        "Generates genesis configuration for testnet"
    }

    fn execute(
        &self,
        _commands: &HashMap<CommandName, CollectedCommand>,
        mut context: Context,
        exts: &dyn Fn(Context) -> Context,
    ) -> Feedback {
        let dir = context.arg::<String>(OUTPUT_DIR).expect("output dir");
        let validators_count: u16 = context.arg("COUNT").expect("count as int");
        let start_port = context
            .arg::<u16>("START_PORT")
            .unwrap_or(DEFAULT_EXONUM_LISTEN_PORT);

        if validators_count == 0 {
            panic!("Can't generate testnet with zero nodes count.");
        }

        let dir = Path::new(&dir);
        let dir = dir.join("validators");
        if !dir.exists() {
            fs::create_dir_all(&dir).unwrap();
        }

        let configs = generate_testnet_config(validators_count, start_port);
        context.set(keys::CONFIGS, configs);
        let new_context = exts(context);
        let configs = new_context
            .get(keys::CONFIGS)
            .expect("Couldn't read testnet configs after exts call.");

        for (idx, cfg) in configs.into_iter().enumerate() {
            let cfg_filename = format!("{}.toml", idx);
            let consensus_key_filename = format!("consensus{}.toml", idx);
            let service_key_filename = format!("service{}.toml", idx);

            let consensus_secret_key_path = dir.join(&consensus_key_filename);
            let service_secret_key_path = dir.join(&service_key_filename);
            let consensus_public_key = create_secret_key_file(&consensus_secret_key_path, &[]);
            let service_public_key = create_secret_key_file(&service_secret_key_path, &[]);

            let config_file_path = dir.join(cfg_filename);
            let config: NodeConfig<PathBuf> = NodeConfig {
                consensus_secret_key: consensus_key_filename.into(),
                service_secret_key: service_key_filename.into(),
                consensus_public_key,
                service_public_key,
                genesis: cfg.genesis,
                listen_address: cfg.listen_address,
                external_address: cfg.external_address,
                network: cfg.network,
                api: cfg.api,
                mempool: cfg.mempool,
                services_configs: cfg.services_configs,
                database: cfg.database,
                connect_list: cfg.connect_list,
                thread_pool_size: cfg.thread_pool_size,
            };

            ConfigFile::save(&config, &config_file_path).unwrap();
        }

        Feedback::None
    }
}

fn create_secret_key_file(
    secret_key_path: impl AsRef<Path>,
    passphrase: impl AsRef<[u8]>,
) -> PublicKey {
    let secret_key_path = secret_key_path.as_ref();
    if secret_key_path.exists() {
        panic!(
            "Failed to create secret key file. File exists: {}",
            secret_key_path.to_string_lossy(),
        );
    } else {
        if let Some(dir) = secret_key_path.parent() {
            fs::create_dir_all(dir).unwrap();
        }
        generate_keys_file(&secret_key_path, &passphrase).unwrap()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_generate_node_config_addresses() {
        let mut ctx = Context::default();

        let external = "127.0.0.1:1234";
        ctx.set_arg(PEER_ADDRESS, external.to_string());
        assert_eq!(
            GenerateNodeConfig::addresses(&ctx),
            (external.to_string(), "0.0.0.0:1234".parse().unwrap())
        );

        let external = "127.0.0.1";
        ctx.set_arg(PEER_ADDRESS, external.to_string());
        assert_eq!(
            GenerateNodeConfig::addresses(&ctx),
            (
                SocketAddr::new(external.parse().unwrap(), DEFAULT_EXONUM_LISTEN_PORT).to_string(),
                SocketAddr::new("0.0.0.0".parse().unwrap(), DEFAULT_EXONUM_LISTEN_PORT)
            )
        );

        let external = "2001:db8::1";
        ctx.set_arg(PEER_ADDRESS, external.to_string());
        assert_eq!(
            GenerateNodeConfig::addresses(&ctx),
            (
                SocketAddr::new(external.parse().unwrap(), DEFAULT_EXONUM_LISTEN_PORT).to_string(),
                SocketAddr::new("::".parse().unwrap(), DEFAULT_EXONUM_LISTEN_PORT)
            )
        );

        let external = "[2001:db8::1]:1234";
        ctx.set_arg(PEER_ADDRESS, external.to_string());
        assert_eq!(
            GenerateNodeConfig::addresses(&ctx),
            (external.to_string(), "[::]:1234".parse().unwrap())
        );

        let external = "localhost";
        ctx.set_arg(PEER_ADDRESS, external.to_string());
        assert_eq!(
            GenerateNodeConfig::addresses(&ctx),
            (
                format!("{}:{}", external, DEFAULT_EXONUM_LISTEN_PORT),
                SocketAddr::new("0.0.0.0".parse().unwrap(), DEFAULT_EXONUM_LISTEN_PORT)
            )
        );

        let external = "localhost:1234";
        ctx.set_arg(PEER_ADDRESS, external.to_string());
        assert_eq!(
            GenerateNodeConfig::addresses(&ctx),
            (
                external.to_string(),
                SocketAddr::new("0.0.0.0".parse().unwrap(), 1234)
            )
        );

        let external = "127.0.0.1:1234";
        let listen = "1.2.3.4:5678";
        ctx.set_arg(PEER_ADDRESS, external.to_string());
        ctx.set_arg(LISTEN_ADDRESS, listen.to_string());
        assert_eq!(
            GenerateNodeConfig::addresses(&ctx),
            (external.to_string(), listen.parse().unwrap())
        );

        let external = "127.0.0.1:1234";
        let listen = "1.2.3.4:5678";
        ctx.set_arg(PEER_ADDRESS, external.to_string());
        ctx.set_arg(LISTEN_ADDRESS, listen.to_string());
        assert_eq!(
            GenerateNodeConfig::addresses(&ctx),
            (external.to_string(), listen.parse().unwrap())
        );

        let external = "example.com";
        let listen = "[2001:db8::1]:5678";
        ctx.set_arg(PEER_ADDRESS, external.to_string());
        ctx.set_arg(LISTEN_ADDRESS, listen.to_string());
        assert_eq!(
            GenerateNodeConfig::addresses(&ctx),
            (
                format!("{}:{}", external, DEFAULT_EXONUM_LISTEN_PORT),
                listen.parse().unwrap()
            )
        );
    }

}
