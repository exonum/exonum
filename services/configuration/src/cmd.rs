// Copyright 2018 The Exonum Team
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

use failure;
use toml::Value;

use exonum::{
    blockchain::{GenesisConfig, ValidatorKeys},
    crypto::gen_keypair,
    helpers::fabric::{keys, Argument, CommandExtension, Context, DEFAULT_EXONUM_LISTEN_PORT},
    node::State,
    node::{ConnectListConfig, NodeConfig},
};

use std::collections::BTreeMap;

use config::ConfigurationServiceConfig;
use errors::Error as ServiceError;

pub struct GenerateCommonConfig;

impl CommandExtension for GenerateCommonConfig {
    fn args(&self) -> Vec<Argument> {
        vec![Argument::new_named(
            "MAJORITY_COUNT",
            false,
            "Number of votes required to commit new configuration",
            None,
            "majority-count",
            false,
        )]
    }

    fn execute(&self, mut context: Context) -> Result<Context, failure::Error> {
        let validators_count = context
            .arg::<u16>("VALIDATORS_COUNT")
            .expect("VALIDATORS_COUNT not found");

        let majority_count = context.arg::<u16>("MAJORITY_COUNT").ok();

        let mut values: BTreeMap<String, Value> = context.get(keys::SERVICES_CONFIG).expect(
            "Expected services_config \
             in context.",
        );

        let byzantine_majority_count =
            State::byzantine_majority_count(validators_count as usize) as u16;

        validate_majority_count(majority_count, validators_count, byzantine_majority_count)
            .unwrap();

        if let Some(majority_count) = majority_count {
            values.extend(
                vec![(
                    "majority_count".to_owned(),
                    Value::try_from(majority_count).unwrap(),
                )]
                .into_iter(),
            );
        };

        context.set(keys::SERVICES_CONFIG, values);
        Ok(context)
    }
}

pub struct Finalize;

impl CommandExtension for Finalize {
    fn args(&self) -> Vec<Argument> {
        vec![]
    }

    fn execute(&self, mut context: Context) -> Result<Context, failure::Error> {
        let mut node_config: NodeConfig = context.get(keys::NODE_CONFIG).unwrap();
        let common_config = context.get(keys::COMMON_CONFIG).unwrap();

        // Local config section
        let majority_count =
            if let Some(majority_count) = common_config.services_config.get("majority_count") {
                Value::try_into(majority_count.clone()).unwrap_or_default()
            } else {
                Default::default()
            };

        node_config.services_configs.insert(
            "configuration_service".to_owned(),
            Value::try_from(ConfigurationServiceConfig { majority_count })
                .expect("Could not serialize configuration service config"),
        );
        context.set(keys::NODE_CONFIG, node_config);
        Ok(context)
    }
}

pub struct GenerateTestnet;

impl CommandExtension for GenerateTestnet {
    fn args(&self) -> Vec<Argument> {
        vec![Argument::new_named(
            "MAJORITY_COUNT",
            false,
            "Number of votes required to commit new configuration",
            None,
            "majority-count",
            false,
        )]
    }

    fn execute(&self, mut context: Context) -> Result<Context, failure::Error> {
        let validators_count: u16 = context.arg("COUNT").expect("count as int");
        let start_port = context
            .arg::<u16>("START_PORT")
            .unwrap_or(DEFAULT_EXONUM_LISTEN_PORT);

        if validators_count == 0 {
            panic!("Can't generate testnet with zero nodes count.");
        }

        let majority_count = context.arg::<u16>("MAJORITY_COUNT").ok();

        let byzantine_majority_count =
            State::byzantine_majority_count(validators_count as usize) as u16;
        validate_majority_count(majority_count, validators_count, byzantine_majority_count)
            .unwrap();

        let configs = generate_testnet_config(validators_count, start_port, majority_count);
        context.set(keys::CONFIGS, configs);

        Ok(context)
    }
}

/// Validate majority count
fn validate_majority_count(
    majority_count: Option<u16>,
    validators_count: u16,
    byzantine_majority_count: u16,
) -> Result<(), ServiceError> {
    use self::ServiceError::*;
    if let Some(majority_count) = majority_count {
        if majority_count > validators_count || majority_count < byzantine_majority_count {
            return Err(InvalidMajorityCount {
                min: byzantine_majority_count as usize,
                max: validators_count as usize,
                proposed: majority_count as usize,
            })?;
        }
    }
    Ok(())
}

/// Generates testnet configuration.
pub fn generate_testnet_config(
    count: u16,
    start_port: u16,
    majority_count: Option<u16>,
) -> Vec<NodeConfig> {
    let (validators, services): (Vec<_>, Vec<_>) = (0..count as usize)
        .map(|_| (gen_keypair(), gen_keypair()))
        .unzip();
    let genesis =
        GenesisConfig::new(
            validators
                .iter()
                .zip(services.iter())
                .map(|x| ValidatorKeys {
                    consensus_key: (x.0).0,
                    service_key: (x.1).0,
                }),
        );

    let mut service_config: BTreeMap<String, Value> = BTreeMap::new();

    service_config.insert(
        "configuration_service".to_owned(),
        Value::try_from(ConfigurationServiceConfig { majority_count })
            .expect("Could not serialize configuration service config"),
    );

    let peers = (0..validators.len())
        .map(|x| format!("127.0.0.1:{}", start_port + x as u16))
        .collect::<Vec<_>>();

    validators
        .into_iter()
        .zip(services.into_iter())
        .enumerate()
        .map(|(idx, (validator, service))| NodeConfig {
            listen_address: peers[idx].parse().unwrap(),
            external_address: peers[idx].clone(),
            network: Default::default(),
            consensus_public_key: validator.0,
            consensus_secret_key: validator.1,
            service_public_key: service.0,
            service_secret_key: service.1,
            genesis: genesis.clone(),
            connect_list: ConnectListConfig::from_validator_keys(&genesis.validator_keys, &peers),
            api: Default::default(),
            mempool: Default::default(),
            services_configs: service_config.clone(),
            database: Default::default(),
            thread_pool_size: Default::default(),
        })
        .collect::<Vec<_>>()
}
