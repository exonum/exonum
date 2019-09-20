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

use exonum::{
    blockchain::InstanceCollection,
    crypto::{self, Hash},
    helpers::{Height, ValidatorId},
    merkledb::{Entry, Snapshot},
    runtime::{
        rust::{
            interfaces::verify_caller_is_supervisor, Configure, Service, Transaction,
            TransactionContext,
        },
        DispatcherError, ExecutionError, InstanceDescriptor, InstanceId,
    },
};
use exonum_derive::ServiceFactory;

use crate::{
    simple_supervisor::{ConfigPropose, SimpleSupervisor},
    TestKitBuilder,
};

#[derive(Debug, ServiceFactory)]
#[exonum(
    proto_sources = "super::proto::schema",
    artifact_name = "config-change-test-service",
    implements("Configure<Params = String>")
)]
struct ConfigChangeService;

impl ConfigChangeService {
    const INSTANCE_ID: InstanceId = 119;
    const INSTANCE_NAME: &'static str = "config-change";
}

impl From<ConfigChangeService> for InstanceCollection {
    fn from(instance: ConfigChangeService) -> Self {
        InstanceCollection::new(instance).with_instance(
            ConfigChangeService::INSTANCE_ID,
            ConfigChangeService::INSTANCE_NAME,
            Vec::default(),
        )
    }
}

impl Service for ConfigChangeService {
    fn state_hash(&self, _: InstanceDescriptor, _: &dyn Snapshot) -> Vec<Hash> {
        Vec::new()
    }
}

impl Configure for ConfigChangeService {
    type Params = String;

    fn verify_config(
        &self,
        context: TransactionContext,
        params: Self::Params,
    ) -> Result<(), ExecutionError> {
        context
            .verify_caller(verify_caller_is_supervisor)
            .ok_or(DispatcherError::UnauthorizedCaller)?;

        match params.as_ref() {
            "error" => Err(DispatcherError::malformed_arguments("Error!")).map_err(From::from),
            "panic" => panic!("Aaaa!"),
            _ => Ok(()),
        }
    }

    fn apply_config(
        &self,
        context: TransactionContext,
        params: Self::Params,
    ) -> Result<(), ExecutionError> {
        let (_, fork) = context
            .verify_caller(verify_caller_is_supervisor)
            .ok_or(DispatcherError::UnauthorizedCaller)?;

        Entry::new(format!("{}.params", context.instance.name), fork).set(params);
        Ok(())
    }
}

#[test]
fn add_nodes_to_validators() {
    let mut testkit = TestKitBuilder::auditor()
        .with_validators(1)
        .with_service(SimpleSupervisor)
        .create();

    let cfg_change_height = Height(5);
    let new_consensus_config = {
        let mut cfg = testkit.consensus_config();
        // Add us node.
        cfg.validator_keys.push(testkit.us().public_keys());
        // Add new node.
        cfg.validator_keys
            .push(testkit.network_mut().add_node().public_keys());
        cfg
    };

    testkit.create_block_with_transaction(
        ConfigPropose::actual_from(cfg_change_height)
            .consensus_config(new_consensus_config.clone())
            .into_tx(),
    );

    testkit.create_blocks_until(cfg_change_height.previous());
    assert_eq!(testkit.network().us().validator_id(), None);
    testkit.create_block();

    assert_eq!(testkit.network().us().validator_id(), Some(ValidatorId(1)));
    assert_eq!(&testkit.network().validators()[1], testkit.network().us());
    assert_eq!(testkit.consensus_config(), new_consensus_config);
}

#[test]
fn exclude_us_from_validators() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(2)
        .with_service(SimpleSupervisor)
        .create();

    let cfg_change_height = Height(5);
    let new_consensus_config = {
        let mut cfg = testkit.consensus_config();
        // Remove us from validators
        cfg.validator_keys.remove(0);
        cfg
    };

    let old_validators = testkit.network().validators();

    testkit.create_block_with_transaction(
        ConfigPropose::actual_from(cfg_change_height)
            .consensus_config(new_consensus_config.clone())
            .into_tx(),
    );
    testkit.create_blocks_until(cfg_change_height);

    let new_validators = testkit.network().validators();

    assert_eq!(testkit.consensus_config(), new_consensus_config);
    assert_eq!(testkit.network().us().validator_id(), None);
    assert_ne!(old_validators, new_validators);
}

#[test]
fn exclude_other_from_validators() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(2)
        .with_service(SimpleSupervisor)
        .create();

    let cfg_change_height = Height(5);
    let new_consensus_config = {
        let mut cfg = testkit.consensus_config();
        // Remove one of node from validators
        cfg.validator_keys.remove(1);
        cfg
    };

    testkit.create_block_with_transaction(
        ConfigPropose::actual_from(cfg_change_height)
            .consensus_config(new_consensus_config.clone())
            .into_tx(),
    );
    testkit.create_blocks_until(cfg_change_height);

    assert_eq!(testkit.consensus_config(), new_consensus_config);
    assert_eq!(testkit.network().us().validator_id(), Some(ValidatorId(0)));
}

#[test]
fn change_us_validator_id() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(2)
        .with_service(SimpleSupervisor)
        .create();

    let cfg_change_height = Height(5);
    let new_consensus_config = {
        let mut cfg = testkit.consensus_config();
        // Remove one of node from validators
        cfg.validator_keys.swap(0, 1);
        cfg
    };

    testkit.create_block_with_transaction(
        ConfigPropose::actual_from(cfg_change_height)
            .consensus_config(new_consensus_config.clone())
            .into_tx(),
    );
    testkit.create_blocks_until(cfg_change_height);

    assert_eq!(testkit.network().us().validator_id(), Some(ValidatorId(1)));
    assert_eq!(&testkit.network().validators()[1], testkit.network().us());
    assert_eq!(testkit.consensus_config(), new_consensus_config);
}

#[test]
fn service_config_change() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(2)
        .with_service(SimpleSupervisor)
        .with_service(ConfigChangeService)
        .create();

    let cfg_change_height = Height(5);
    let params = "I am a new parameter".to_owned();

    testkit.create_block_with_transaction(
        ConfigPropose::actual_from(cfg_change_height)
            .service_config(ConfigChangeService::INSTANCE_ID, params.clone())
            .into_tx(),
    );
    testkit.create_blocks_until(cfg_change_height);

    let actual_params: String = Entry::new(
        format!("{}.params", ConfigChangeService::INSTANCE_NAME),
        &testkit.snapshot(),
    )
    .get()
    .unwrap();

    assert_eq!(actual_params, params);
}

#[test]
fn discard_errored_service_config_change() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(2)
        .with_service(SimpleSupervisor)
        .with_service(ConfigChangeService)
        .create();

    let cfg_change_height = Height(5);
    let params = "I am a discarded parameter".to_owned();
    let new_consensus_config = {
        let mut cfg = testkit.consensus_config();
        // Remove us from validators
        cfg.validator_keys.remove(0);
        cfg
    };

    testkit.create_block_with_transaction(
        ConfigPropose::actual_from(cfg_change_height)
            .service_config(ConfigChangeService::INSTANCE_ID, params.clone())
            .service_config(ConfigChangeService::INSTANCE_ID, "error".to_owned())
            .consensus_config(new_consensus_config)
            .into_tx(),
    );
    testkit.create_blocks_until(cfg_change_height);

    let actual_params: Option<String> = Entry::new(
        format!("{}.params", ConfigChangeService::INSTANCE_NAME),
        &testkit.snapshot(),
    )
    .get();

    assert!(actual_params.is_none());
    assert_eq!(testkit.network().us().validator_id(), Some(ValidatorId(0)));
}

#[test]
fn discard_panicked_service_config_change() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(2)
        .with_service(SimpleSupervisor)
        .with_service(ConfigChangeService)
        .create();

    let cfg_change_height = Height(5);
    let params = "I am a discarded parameter".to_owned();
    let new_consensus_config = {
        let mut cfg = testkit.consensus_config();
        // Remove us from validators
        cfg.validator_keys.remove(0);
        cfg
    };

    testkit.create_block_with_transaction(
        ConfigPropose::actual_from(cfg_change_height)
            .service_config(ConfigChangeService::INSTANCE_ID, params.clone())
            .service_config(ConfigChangeService::INSTANCE_ID, "panic".to_owned())
            .consensus_config(new_consensus_config)
            .into_tx(),
    );
    testkit.create_blocks_until(cfg_change_height);

    let actual_params: Option<String> = Entry::new(
        format!("{}.params", ConfigChangeService::INSTANCE_NAME),
        &testkit.snapshot(),
    )
    .get();

    assert!(actual_params.is_none());
    assert_eq!(testkit.network().us().validator_id(), Some(ValidatorId(0)));
}

#[test]
fn incorrect_actual_from_field() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(2)
        .with_service(SimpleSupervisor)
        .with_service(ConfigChangeService)
        .create();

    let cfg_change_height = Height(5);
    let params = "I am a new parameter".to_owned();

    testkit.create_blocks_until(cfg_change_height);
    testkit
        .create_block_with_transaction(
            ConfigPropose::actual_from(cfg_change_height)
                .service_config(ConfigChangeService::INSTANCE_ID, params.clone())
                .into_tx(),
        )
        .transactions[0]
        .status()
        .unwrap_err();
}

#[test]
fn another_configuration_change_proposal() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(2)
        .with_service(SimpleSupervisor)
        .with_service(ConfigChangeService)
        .create();

    let cfg_change_height = Height(5);
    let params = "I am a new parameter".to_owned();

    testkit.create_block_with_transaction(
        ConfigPropose::actual_from(cfg_change_height)
            .service_config(ConfigChangeService::INSTANCE_ID, params.clone())
            .into_tx(),
    );
    // Try to commit second config change propose.
    testkit
        .create_block_with_transaction(
            ConfigPropose::actual_from(cfg_change_height)
                .service_config(
                    ConfigChangeService::INSTANCE_ID,
                    "I am an overridden parameter".to_owned(),
                )
                .into_tx(),
        )
        .transactions[0]
        .status()
        .unwrap_err();
    testkit.create_blocks_until(cfg_change_height);

    let actual_params: String = Entry::new(
        format!("{}.params", ConfigChangeService::INSTANCE_NAME),
        &testkit.snapshot(),
    )
    .get()
    .unwrap();

    assert_eq!(actual_params, params);
}

#[test]
fn service_config_discard_fake_supervisor() {
    const FAKE_SUPERVISOR_ID: InstanceId = 5;

    let keypair = crypto::gen_keypair();

    let mut testkit = TestKitBuilder::validator()
        .with_validators(2)
        .with_service(InstanceCollection::new(SimpleSupervisor).with_instance(
            FAKE_SUPERVISOR_ID,
            "fake-supervisor",
            Vec::default(),
        ))
        .with_service(ConfigChangeService)
        .create();

    let cfg_change_height = Height(5);
    let params = "I am a new parameter".to_owned();

    testkit
        .create_block_with_transaction(
            ConfigPropose::actual_from(cfg_change_height)
                .service_config(ConfigChangeService::INSTANCE_ID, params.clone())
                .sign(FAKE_SUPERVISOR_ID, keypair.0, &keypair.1),
        )
        .transactions[0]
        .status()
        .unwrap_err();
}

#[test]
fn test_configuration_and_rollbacks() {
    let mut testkit = TestKitBuilder::auditor()
        .with_validators(1)
        .with_service(SimpleSupervisor)
        .create();

    testkit.create_blocks_until(Height(5));

    let cfg_change_height = Height(10);
    let new_config = {
        let mut cfg = testkit.consensus_config();
        // Add us node.
        cfg.validator_keys.push(testkit.us().public_keys());
        // Add new node.
        cfg.validator_keys
            .push(testkit.network_mut().add_node().public_keys());
        cfg
    };
    let old_config = testkit.consensus_config();

    testkit.checkpoint();

    testkit.create_block_with_transaction(
        ConfigPropose::actual_from(cfg_change_height)
            .consensus_config(new_config.clone())
            .into_tx(),
    );

    testkit.create_blocks_until(cfg_change_height);
    assert_eq!(testkit.consensus_config(), new_config);

    testkit.checkpoint();
    testkit.create_block();
    testkit.rollback();
    assert_eq!(testkit.consensus_config(), new_config);

    testkit.rollback();

    // As rollback is behind the time a proposal entered the blockchain,
    // the proposal is effectively forgotten.
    testkit.create_blocks_until(Height(10));
    assert_eq!(testkit.consensus_config(), old_config);
}
