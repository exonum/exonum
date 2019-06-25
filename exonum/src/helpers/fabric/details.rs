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

use super::{
    internal::{CollectedCommand, Command, Feedback},
    keys,
    password::{PassInputMethod, SecretKeyType},
    shared::{
        AbstractConfig, CommonConfigTemplate, NodePrivateConfig, NodePublicConfig, NodeRunConfig,
        SharedConfig,
    },
    Argument, CommandName, Context, DEFAULT_EXONUM_LISTEN_PORT,
};
use crate::api::node::private::AddAuditorRequest;
use crate::api::node::public::system::SharedConfiguration;
use crate::api::{backends::actix::AllowOrigin, node::public::system::KeyInfo};
use crate::blockchain::{config::ValidatorKeys, GenesisConfig};
use crate::crypto::{generate_keys_file, PublicKey};
use crate::helpers::fabric::shared::{AddAuditorInfo, AuditorPrimaryConfig};
use crate::helpers::{config::ConfigFile, ZeroizeOnDrop};
use crate::node::{ConnectInfo, ConnectListConfig, NodeApiConfig, NodeConfig};
use actix::SystemRunner;
use actix_web::{client, HttpMessage};
use exonum_merkledb::{Database, DbOptions, RocksDB};
use futures::future::{lazy, Future};
use hex::FromHex;
use std::ffi::OsStr;
use std::time::Duration;
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
    thread,
};

const CONSENSUS_KEY_PASS_METHOD: &str = "CONSENSUS_KEY_PASS_METHOD";
const DATABASE_PATH: &str = "DATABASE_PATH";
const LISTEN_ADDRESS: &str = "LISTEN_ADDRESS";
const NO_PASSWORD: &str = "NO_PASSWORD";
const NODE_CONFIG_PATH: &str = "NODE_CONFIG_PATH";
const PEER_ADDRESS: &str = "PEER_ADDRESS";
const PRIVATE_ALLOW_ORIGIN: &str = "PRIVATE_ALLOW_ORIGIN";
const PRIVATE_API_ADDRESS: &str = "PRIVATE_API_ADDRESS";
const PUBLIC_ALLOW_ORIGIN: &str = "PUBLIC_ALLOW_ORIGIN";
const PUBLIC_API_ADDRESS: &str = "PUBLIC_API_ADDRESS";
const SERVICE_KEY_PASS_METHOD: &str = "SERVICE_KEY_PASS_METHOD";
const VALIDATORS_API: &str = "VALIDATORS_API";
const CONNECT_ALL: &str = "CONNECT_ALL";
const CONSENSUS_KEY: &str = "CONSENSUS_KEY";
const VALIDATORS_KEYS: &str = "VALIDATORS_KEYS";
const VALIDATORS_KEY_PATHS: &str = "VALIDATORS_KEY_PATH";
const WAIT: &str = "WAIT";

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
        let common_config_path = Self::artifacts_path("template.toml", &ctx);
        let output_path = Self::artifacts_directory(&ctx).join("cfg");
        let validators_count = "1";
        let peer_addr = "127.0.0.1";
        let pub_config_path = output_path
            .join("pub.toml")
            .to_str()
            .expect("Expected correct path")
            .to_owned();
        let sec_config_path = output_path
            .join("sec.toml")
            .to_str()
            .expect("Expected correct path")
            .to_owned();
        let output_config_path = Self::artifacts_path("node.toml", &ctx);

        // Arguments for common config command.
        ctx.set_arg("COMMON_CONFIG", common_config_path.clone());
        ctx.set_arg("VALIDATORS_COUNT", validators_count.into());

        // Arguments for node config command.
        ctx.set_arg("COMMON_CONFIG", common_config_path.clone());
        ctx.set_arg(
            "OUTPUT_DIR",
            output_path
                .to_str()
                .expect("Expected correct path")
                .to_owned(),
        );
        ctx.set_arg("SEC_CONFIG", sec_config_path.clone());
        ctx.set_arg(PEER_ADDRESS, peer_addr.into());
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

impl Command for GenerateNodeConfig {
    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::new_positional("COMMON_CONFIG", true, "Path to common config."),
            Argument::new_positional(
                "OUTPUT_DIR",
                true,
                "Path where the node configuration will be saved.",
            ),
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
        let output_dir: PathBuf = context
            .arg("OUTPUT_DIR")
            .expect("expected output directory for the node configuration");
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

        let pub_config_path = output_dir.join("pub.toml");
        let private_config_path = output_dir.join("sec.toml");
        let consensus_secret_key_name = "consensus.key.toml";
        let service_secret_key_name = "service.key.toml";
        let consensus_secret_key_path = output_dir.join(consensus_secret_key_name);
        let service_secret_key_path = output_dir.join(service_secret_key_name);

        let addresses = addresses(&context);
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
            let passphrase = get_passphrase(
                &new_context,
                consensus_key_pass_method,
                SecretKeyType::Consensus,
            );
            create_secret_key_file(&consensus_secret_key_path, passphrase.as_bytes())
        };
        let service_public_key = {
            let passphrase = get_passphrase(
                &new_context,
                service_key_pass_method,
                SecretKeyType::Service,
            );
            create_secret_key_file(&service_secret_key_path, passphrase.as_bytes())
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
            consensus_secret_key: consensus_secret_key_name.into(),
            service_public_key,
            service_secret_key: service_secret_key_name.into(),
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
            ConfigFile::load(&secret_config_path).expect("Failed to load key config.");
        let secret_config_dir = std::env::current_dir()
            .expect("Failed to get current dir")
            .join(PathBuf::from(&secret_config_path).parent().unwrap());
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
                consensus_secret_key: secret_config_dir.join(&secret_config.consensus_secret_key),
                service_public_key: secret_config.service_public_key,
                service_secret_key: secret_config_dir.join(&secret_config.service_secret_key),
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
                auditor: Default::default(),
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

/// Connect auditor command.
pub struct RequestConnectAuditor;

impl RequestConnectAuditor {
    fn exe_name() -> String {
        std::env::current_exe()
            .ok()
            .as_ref()
            .map(Path::new)
            .and_then(Path::file_name)
            .and_then(OsStr::to_str)
            .map(|s| format!("./{}", s))
            .unwrap_or("".to_owned())
    }
}

impl Command for RequestConnectAuditor {
    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::new_named(
                VALIDATORS_API,
                true,
                "Validators api addresses",
                "v",
                "validators-api",
                true,
            ),
            Argument::new_named(
                CONNECT_ALL,
                false,
                "Connect to all validators.",
                None,
                "connect-all",
                false,
            ),
            Argument::new_named(
                PEER_ADDRESS,
                true,
                "Remote peer address",
                "a",
                "peer-address",
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
            Argument::new_positional(
                "OUTPUT_DIR",
                true,
                "Path where the node configuration will be saved.",
            ),
        ]
    }

    fn name(&self) -> CommandName {
        "request_connect_auditor"
    }

    fn about(&self) -> &str {
        "Create primary auditor configuration and send connect auditor request"
    }

    fn execute(
        &self,
        _commands: &HashMap<CommandName, CollectedCommand>,
        context: Context,
        exts: &dyn Fn(Context) -> Context,
    ) -> Feedback {
        let connect_all: bool = context
            .arg::<String>(CONNECT_ALL)
            .unwrap_or_else(|_| "false".into())
            .parse()
            .expect("expected correct passphrase input method for connect_all flag");
        let output_dir: PathBuf = context
            .arg("OUTPUT_DIR")
            .expect("expected output directory for the node configuration");
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

        let consensus_secret_key_name = "consensus.key.toml";
        let service_secret_key_name = "service.key.toml";
        let add_auditor_request_key_name = "request.toml";
        let consensus_secret_key_path = output_dir.join(consensus_secret_key_name);
        let service_secret_key_path = output_dir.join(service_secret_key_name);
        let output_config_path = output_dir.join(add_auditor_request_key_name);

        let (external_address, listen_address) = addresses(&context);

        let validators_api = validators(&context);

        let new_context = exts(context);

        let consensus_public_key = {
            let passphrase = get_passphrase(
                &new_context,
                consensus_key_pass_method,
                SecretKeyType::Consensus,
            );
            create_secret_key_file(&consensus_secret_key_path, passphrase.as_bytes())
        };
        let service_public_key = {
            let passphrase = get_passphrase(
                &new_context,
                service_key_pass_method,
                SecretKeyType::Service,
            );
            create_secret_key_file(&service_secret_key_path, passphrase.as_bytes())
        };

        let validators_api =
            validators_api.expect("expected correct passphrase input method for validators_api");

        let validators_api_param = validators_api.join(" ");

        let consensus_secret_key =
            fs::canonicalize(consensus_secret_key_path).expect("Failed to create canonical path.");
        let service_secret_key =
            fs::canonicalize(service_secret_key_path).expect("Failed to create canonical path.");

        let config = AuditorPrimaryConfig {
            listen_address,
            external_address: external_address.clone(),
            consensus_public_key,
            consensus_secret_key,
            service_public_key,
            service_secret_key,
            add_auditor_request: AddAuditorInfo {
                validators_api,
                connect_all,
            },
        };

        ConfigFile::save(&config, output_config_path).expect("Could not write config file.");

        println!(
            "{} add_auditor --connect-all {} --peer-address {} --consensus-pub-key {} --validators-api {} --private-api-address ",
            Self::exe_name(), connect_all, external_address, consensus_public_key.to_hex(), validators_api_param
        );

        Feedback::None
    }
}

pub struct AddAuditor;

impl AddAuditor {
    fn parse_validator_api(context: &Context, sys: &mut SystemRunner) -> Option<Vec<PublicKey>> {
        validators(context).map(|validators_api| {
            validators_api
                .iter()
                .map(|api| format!("http://{}/api/system/v1/service_key", api))
                .map(|url| {
                    match sys.block_on(lazy(|| {
                        client::get(url.clone())
                            .finish()
                            .expect("Failed to create http client.")
                            .send()
                            .map_err(|err| format!("{:?}", err))
                            .and_then(|response| {
                                response
                                    .body()
                                    .map_err(|err| format!("{:?}", err))
                                    .and_then(|body| {
                                        serde_json::from_slice(body.as_ref())
                                            .map_err(|err| format!("{:?}", err))
                                    })
                            })
                            .map(|key: KeyInfo| key.pub_key)
                    })) {
                        Ok(key) => key,
                        Err(err) => {
                            panic!("Failed to perform request [{}]; Error [{:?}]", url, err)
                        }
                    }
                })
                .collect()
        })
    }

    fn parse_validators_keys(context: &Context) -> Option<Vec<PublicKey>> {
        context
            .arg_multiple::<String>(VALIDATORS_KEYS)
            .map(|keys| {
                keys.iter()
                    .map(|key| {
                        PublicKey::from_hex(key).expect(
                            "expected correct passphrase input method for validator public key",
                        )
                    })
                    .collect()
            })
            .ok()
    }

    fn parse_validator_key_path(context: &Context) -> Option<Vec<PublicKey>> {
        context
            .arg_multiple::<String>(VALIDATORS_KEY_PATHS)
            .map(|path_vec| {
                path_vec
                    .iter()
                    .map(|path| {
                        ConfigFile::load(path).expect("Failed to load validator public config.")
                    })
                    .map(|config: SharedConfig| config.node.validator_keys.service_key)
                    .collect()
            })
            .ok()
    }

    fn validator_keys(context: &Context, sys: &mut SystemRunner) -> Vec<PublicKey> {
        if let Some(keys) = Self::parse_validators_keys(context) {
            return keys;
        }

        if let Some(keys) = Self::parse_validator_key_path(context) {
            return keys;
        }

        if let Some(keys) = Self::parse_validator_api(context, sys) {
            return keys;
        }

        panic!("Failed to get validator list to add auditor. See validators_api, validators_key or validators_key_paths")
    }
}

impl Command for AddAuditor {
    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::new_named(
                CONNECT_ALL,
                false,
                "Connect to all validators.",
                None,
                "connect-all",
                false,
            ),
            Argument::new_named(
                VALIDATORS_API,
                false,
                "Validators api addresses",
                "v",
                "validators-api",
                true,
            ),
            Argument::new_named(
                VALIDATORS_KEYS,
                false,
                "Validators service key",
                "k",
                "validators-key",
                true,
            ),
            Argument::new_named(
                VALIDATORS_KEY_PATHS,
                false,
                "Validators service key",
                "p",
                "validators-key-paths",
                true,
            ),
            Argument::new_named(
                PEER_ADDRESS,
                true,
                "Remote peer address",
                "a",
                "peer-address",
                false,
            ),
            Argument::new_named(
                CONSENSUS_KEY,
                true,
                "Consensus public key",
                None,
                "consensus-pub-key",
                false,
            ),
            Argument::new_named(
                PRIVATE_API_ADDRESS,
                true,
                "Listen address for private api.",
                None,
                "private-api-address",
                false,
            ),
        ]
    }

    fn name(&self) -> CommandName {
        "add_auditor"
    }

    fn about(&self) -> &str {
        "Add auditor to the blockchain net"
    }

    fn execute(
        &self,
        _commands: &HashMap<CommandName, CollectedCommand>,
        context: Context,
        _exts: &dyn Fn(Context) -> Context,
    ) -> Feedback {
        let connect_all: bool = context
            .arg::<String>(CONNECT_ALL)
            .unwrap_or_else(|_| "false".into())
            .parse()
            .expect("expected correct passphrase input method for connect-all flag");

        let public_key = context
            .arg::<String>(CONSENSUS_KEY)
            .ok()
            .and_then(|hex| PublicKey::from_hex(hex).ok())
            .expect("expected correct passphrase input method for consensus-pub-key");

        let (external_address, _) = addresses(&context);

        let mut sys = actix::System::new("add_auditor");
        let validators = if connect_all {
            vec![]
        } else {
            Self::validator_keys(&context, &mut sys)
        };

        let req = AddAuditorRequest {
            address: external_address,
            public_key,
            connect_all,
            validators,
        };

        let private_api = context
            .arg::<String>(PRIVATE_API_ADDRESS)
            .expect("expected correct passphrase input method for private-api-address flag");

        if let Err(err) = sys.block_on(lazy(|| {
            client::post(format!("http://{}/api/system/v1/auditor/add", private_api))
                .header("Content-Type", "application/json")
                .body(serde_json::to_string(&req).unwrap())
                .expect("Failed to create http client.")
                .send()
                .map_err(|err| format!("{:?}", err))
                .and_then(|response| {
                    let is_success = response.status().is_success();
                    response
                        .body()
                        .map_err(|err| format!("{:?}", err))
                        .and_then(move |data| {
                            if is_success {
                                Ok(())
                            } else {
                                Err(String::from_utf8_lossy(data.as_ref()).to_string())
                            }
                        })
                })
        })) {
            panic!("Failed to add auditor: {}", err);
        }
        Feedback::None
    }
}

pub struct FinalizeAuditorConfig;

impl FinalizeAuditorConfig {
    fn load_node_configuration(
        public_api: &str,
        public_key: &PublicKey,
        sys: &mut SystemRunner,
    ) -> Option<SharedConfiguration> {
        sys.block_on(lazy(|| {
            client::get(format!(
                "http://{}/api/system/v1/remote_config?pub_key={}",
                public_api,
                public_key.to_hex()
            ))
            .finish()
            .expect("Failed to create http client.")
            .send()
            .map_err(|err| format!("{:?}", err))
            .and_then(|response| {
                response
                    .body()
                    .map_err(|err| format!("{:?}", err))
                    .and_then(|body| {
                        serde_json::from_slice(body.as_ref()).map_err(|err| format!("{:?}", err))
                    })
            })
        }))
        .ok()
    }

    fn merge_config(
        primary_cfg: AuditorPrimaryConfig,
        api: NodeApiConfig,
        node_cfg: SharedConfiguration,
        sys: &mut SystemRunner,
    ) -> NodeConfig<PathBuf> {
        let peers: Vec<_> = if primary_cfg.add_auditor_request.connect_all {
            let validator_consensus_keys: Vec<_> = node_cfg
                .genesis
                .validator_keys
                .iter()
                .map(|key| key.consensus_key.clone())
                .collect();

            let mut peers: Vec<_> = node_cfg
                .connect_list
                .peers
                .into_iter()
                .filter(|peer| validator_consensus_keys.contains(&peer.public_key))
                .collect();

            peers.push(ConnectInfo {
                address: node_cfg.external_address,
                public_key: node_cfg.consensus_public_key,
            });
            peers
        } else {
            // Waiting for waiting for state update.
            thread::sleep_ms(node_cfg.api.state_update_timeout as u32);

            primary_cfg
                .add_auditor_request
                .validators_api
                .iter()
                .filter_map(|api| {
                    Self::load_node_configuration(api, &primary_cfg.consensus_public_key, sys)
                })
                .map(|config| ConnectInfo {
                    address: config.external_address,
                    public_key: config.consensus_public_key,
                })
                .collect()
        };

        NodeConfig {
            genesis: node_cfg.genesis,
            listen_address: primary_cfg.listen_address,
            external_address: primary_cfg.external_address,
            network: node_cfg.network,
            consensus_public_key: primary_cfg.consensus_public_key,
            consensus_secret_key: primary_cfg.consensus_secret_key,
            service_public_key: primary_cfg.service_public_key,
            service_secret_key: primary_cfg.service_secret_key,
            api,
            mempool: node_cfg.mempool,
            services_configs: node_cfg.services_configs,
            database: node_cfg.database,
            connect_list: ConnectListConfig { peers },
            thread_pool_size: node_cfg.thread_pool_size,
            auditor: Default::default(),
        }
    }
}

impl Command for FinalizeAuditorConfig {
    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::new_flag(
                WAIT,
                "Wait for an auditor to be added.",
                "w",
                "wait",
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
            Argument::new_positional("PRIMARY_CONFIG", true, "Path to our primari config."),
            Argument::new_positional("OUTPUT_CONFIG_PATH", true, "Path to output node config."),
        ]
    }

    fn name(&self) -> CommandName {
        "finalize_auditor_config"
    }

    fn about(&self) -> &str {
        "Create node config."
    }

    fn execute(
        &self,
        _commands: &HashMap<CommandName, CollectedCommand>,
        context: Context,
        _exts: &dyn Fn(Context) -> Context,
    ) -> Feedback {
        let primary_config_path = context
            .arg::<String>("PRIMARY_CONFIG")
            .expect("primary config path not found");
        let output_config_path = context
            .arg::<String>("OUTPUT_CONFIG_PATH")
            .expect("output config path not found");
        let primary_config: AuditorPrimaryConfig =
            ConfigFile::load(primary_config_path).expect("Failed to load primary config.");

        let public_api_address = Run::public_api_address(&context);
        let private_api_address = Run::private_api_address(&context);
        let public_allow_origin = Finalize::public_allow_origin(&context);
        let private_allow_origin = Finalize::private_allow_origin(&context);

        let mut sys = actix::System::new("finalize_auditor_config");

        let wait = context.has_flag(WAIT);

        let validators_api = primary_config.add_auditor_request.validators_api.clone();
        let pub_key = primary_config.consensus_public_key.clone();

        let node_config = if wait {
            loop {
                match validators_api
                    .iter()
                    .find_map(|api| Self::load_node_configuration(api, &pub_key, &mut sys))
                {
                    Some(c) => {
                        break Some(c);
                    }
                    None => thread::sleep(Duration::from_secs(10)),
                }
            }
        } else {
            validators_api
                .iter()
                .find_map(|api| Self::load_node_configuration(api, &pub_key, &mut sys))
        };

        let node_config = node_config.expect("Auditor is not approved yet.");

        let api = NodeApiConfig {
            public_api_address,
            private_api_address,
            public_allow_origin,
            private_allow_origin,
            ..Default::default()
        };

        let node_config = Self::merge_config(primary_config, api, node_config, &mut sys);

        ConfigFile::save(&node_config, output_config_path).expect("Could not write config file.");

        Feedback::None
    }
}

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

/// Parse validator_api parameter.
fn validators(context: &Context) -> Option<Vec<String>> {
    context
        .arg_multiple::<String>(VALIDATORS_API)
        .map(|api| {
            api.iter()
                .map(|address| {
                    match address.parse().or_else(|_| {
                        address
                            .parse()
                            .map(|ip| SocketAddr::new(ip, DEFAULT_EXONUM_LISTEN_PORT))
                    }) {
                        Ok(address) => address.to_string(),
                        Err(_) => address
                            .rsplitn(2, ':')
                            .next()
                            .and_then(|p| p.parse::<u16>().ok())
                            .map(|_p| address.clone())
                            .unwrap_or_else(|| {
                                format!("{}:{}", address, DEFAULT_EXONUM_LISTEN_PORT)
                            }),
                    }
                })
                .collect()
        })
        .ok()
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
            addresses(&ctx),
            (external.to_string(), "0.0.0.0:1234".parse().unwrap())
        );

        let external = "127.0.0.1";
        ctx.set_arg(PEER_ADDRESS, external.to_string());
        assert_eq!(
            addresses(&ctx),
            (
                SocketAddr::new(external.parse().unwrap(), DEFAULT_EXONUM_LISTEN_PORT).to_string(),
                SocketAddr::new("0.0.0.0".parse().unwrap(), DEFAULT_EXONUM_LISTEN_PORT)
            )
        );

        let external = "2001:db8::1";
        ctx.set_arg(PEER_ADDRESS, external.to_string());
        assert_eq!(
            addresses(&ctx),
            (
                SocketAddr::new(external.parse().unwrap(), DEFAULT_EXONUM_LISTEN_PORT).to_string(),
                SocketAddr::new("::".parse().unwrap(), DEFAULT_EXONUM_LISTEN_PORT)
            )
        );

        let external = "[2001:db8::1]:1234";
        ctx.set_arg(PEER_ADDRESS, external.to_string());
        assert_eq!(
            addresses(&ctx),
            (external.to_string(), "[::]:1234".parse().unwrap())
        );

        let external = "localhost";
        ctx.set_arg(PEER_ADDRESS, external.to_string());
        assert_eq!(
            addresses(&ctx),
            (
                format!("{}:{}", external, DEFAULT_EXONUM_LISTEN_PORT),
                SocketAddr::new("0.0.0.0".parse().unwrap(), DEFAULT_EXONUM_LISTEN_PORT)
            )
        );

        let external = "localhost:1234";
        ctx.set_arg(PEER_ADDRESS, external.to_string());
        assert_eq!(
            addresses(&ctx),
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
            addresses(&ctx),
            (external.to_string(), listen.parse().unwrap())
        );

        let external = "127.0.0.1:1234";
        let listen = "1.2.3.4:5678";
        ctx.set_arg(PEER_ADDRESS, external.to_string());
        ctx.set_arg(LISTEN_ADDRESS, listen.to_string());
        assert_eq!(
            addresses(&ctx),
            (external.to_string(), listen.parse().unwrap())
        );

        let external = "example.com";
        let listen = "[2001:db8::1]:5678";
        ctx.set_arg(PEER_ADDRESS, external.to_string());
        ctx.set_arg(LISTEN_ADDRESS, listen.to_string());
        assert_eq!(
            addresses(&ctx),
            (
                format!("{}:{}", external, DEFAULT_EXONUM_LISTEN_PORT),
                listen.parse().unwrap()
            )
        );
    }
}
