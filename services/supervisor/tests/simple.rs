// Copyright 2020 The Exonum Team
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

//! This file contains tests for the Supervisor in the simple mode.
//! Since simple Supervisor only difference from decentralized one is
//! decision-making algorithm, the tests affect only this aspect.

use exonum::{
    crypto::Hash,
    helpers::{Height, ValidatorId},
    merkledb::access::AccessExt,
    messages::{AnyTx, Verified},
    runtime::{
        CommonError, ErrorMatch, ExecutionContext, ExecutionError, InstanceId, SnapshotExt,
        SUPERVISOR_INSTANCE_ID,
    },
};
use exonum_derive::*;
use exonum_rust_runtime::{DefaultInstance, Service, ServiceFactory as _};
use exonum_testkit::{ApiKind, TestKit, TestKitBuilder};

use exonum_supervisor::{
    supervisor_name, ConfigPropose, Configure, DeployRequest, Schema, Supervisor,
    SupervisorInterface,
};

pub fn sign_config_propose_transaction(
    testkit: &TestKit,
    config: ConfigPropose,
    initiator_id: ValidatorId,
) -> Verified<AnyTx> {
    let keypair = testkit.validator(initiator_id).service_keypair();
    keypair.propose_config_change(SUPERVISOR_INSTANCE_ID, config)
}

pub fn sign_config_propose_transaction_by_us(
    testkit: &TestKit,
    config: ConfigPropose,
) -> Verified<AnyTx> {
    let initiator_id = testkit.us().validator_id().unwrap();
    sign_config_propose_transaction(&testkit, config, initiator_id)
}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_factory(artifact_name = "deployable-test-service", artifact_version = "0.1.0")]
pub struct DeployableService;

impl Service for DeployableService {}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_dispatcher(implements(raw = "Configure<Params = String>"))]
#[service_factory(artifact_name = "config-change-test-service")]
pub struct ConfigChangeService;

impl DefaultInstance for ConfigChangeService {
    const INSTANCE_ID: InstanceId = 119;
    const INSTANCE_NAME: &'static str = "config-change";
}

impl Service for ConfigChangeService {}

impl Configure for ConfigChangeService {
    type Params = String;

    fn verify_config(
        &self,
        context: ExecutionContext<'_>,
        params: Self::Params,
    ) -> Result<(), ExecutionError> {
        context
            .caller()
            .as_supervisor()
            .ok_or(CommonError::UnauthorizedCaller)?;

        match params.as_str() {
            "error" => Err(CommonError::malformed_arguments("Error!")),
            "panic" => panic!("Aaaa!"),
            _ => Ok(()),
        }
    }

    fn apply_config(
        &self,
        context: ExecutionContext<'_>,
        params: Self::Params,
    ) -> Result<(), ExecutionError> {
        context
            .caller()
            .as_supervisor()
            .ok_or(CommonError::UnauthorizedCaller)?;

        context
            .service_data()
            .get_entry("params")
            .set(params.clone());

        match params.as_str() {
            "apply_error" => Err(CommonError::malformed_arguments("Error!")),
            "apply_panic" => panic!("Aaaa!"),
            _ => Ok(()),
        }
    }
}

fn assert_config_change_is_applied(testkit: &TestKit) {
    let snapshot = testkit.snapshot();
    let schema: Schema<_> = snapshot.service_schema(supervisor_name()).unwrap();
    assert!(!schema.pending_proposal.exists());
}

/// Attempts to change consensus config with only one confirmation.
#[test]
fn change_consensus_config_with_one_confirmation() {
    let initial_validator_count = 4;
    let expected_new_validator_number = initial_validator_count;

    let mut testkit = TestKitBuilder::auditor()
        .with_validators(initial_validator_count)
        .with_rust_service(Supervisor)
        .with_artifact(Supervisor.artifact_id())
        .with_instance(Supervisor::simple())
        .build();

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

    // Sign request by validator (we're an auditor yet).
    let initiator_id = testkit.network().validators()[0].validator_id().unwrap();
    let config_propose =
        ConfigPropose::new(0, cfg_change_height).consensus_config(new_consensus_config.clone());

    testkit.create_block_with_transaction(sign_config_propose_transaction(
        &testkit,
        config_propose,
        initiator_id,
    ));

    testkit.create_blocks_until(cfg_change_height.previous());
    assert_eq!(testkit.network().us().validator_id(), None);
    testkit.create_block();

    // We did not send (2/3+1) confirmations, but expect config to be applied.
    assert_config_change_is_applied(&testkit);
    assert_eq!(
        testkit.network().us().validator_id(),
        Some(ValidatorId(expected_new_validator_number))
    );
    assert_eq!(
        &testkit.network().validators()[expected_new_validator_number as usize],
        testkit.network().us()
    );
    assert_eq!(testkit.consensus_config(), new_consensus_config);
}

/// Attempts to change service config with only one confirmation.
#[test]
fn service_config_change() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(2)
        .with_rust_service(Supervisor)
        .with_artifact(Supervisor.artifact_id())
        .with_instance(Supervisor::simple())
        .with_default_rust_service(ConfigChangeService)
        .build();

    let cfg_change_height = Height(5);
    let params = "I am a new parameter".to_owned();

    let config_propose = ConfigPropose::new(0, cfg_change_height)
        .service_config(ConfigChangeService::INSTANCE_ID, params.clone());

    testkit.create_block_with_transaction(sign_config_propose_transaction_by_us(
        &testkit,
        config_propose,
    ));
    testkit.create_blocks_until(cfg_change_height);

    let actual_params: String = testkit
        .snapshot()
        .for_service(ConfigChangeService::INSTANCE_NAME)
        .unwrap()
        .get_entry("params")
        .get()
        .unwrap();

    assert_eq!(actual_params, params);
}

#[test]
fn incorrect_actual_from_field() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(2)
        .with_rust_service(Supervisor)
        .with_artifact(Supervisor.artifact_id())
        .with_instance(Supervisor::simple())
        .with_default_rust_service(ConfigChangeService)
        .build();

    let cfg_change_height = Height(5);
    let params = "I am a new parameter".to_owned();

    testkit.create_blocks_until(cfg_change_height);

    let config_propose = ConfigPropose::new(0, cfg_change_height)
        .service_config(ConfigChangeService::INSTANCE_ID, params.clone());

    testkit
        .create_block_with_transaction(sign_config_propose_transaction_by_us(
            &testkit,
            config_propose,
        ))
        .transactions[0]
        .status()
        .unwrap_err();
}

/// Checks that config propose signed by auditor is discarded.
#[test]
fn discard_config_propose_from_auditor() {
    let mut testkit = TestKitBuilder::auditor()
        .with_validators(2)
        .with_rust_service(Supervisor)
        .with_artifact(Supervisor.artifact_id())
        .with_instance(Supervisor::simple())
        .build();

    let cfg_change_height = Height(5);
    let old_consensus_config = testkit.consensus_config();
    // Attempt to add ourselves into validators list.
    let new_consensus_config = {
        let mut cfg = testkit.consensus_config();
        cfg.validator_keys.push(testkit.us().public_keys());
        cfg
    };

    let old_validators = testkit.network().validators();

    // Sign request by an auditor.
    let propose =
        ConfigPropose::new(0, cfg_change_height).consensus_config(new_consensus_config.clone());
    let keys = testkit.us().service_keypair();
    let propose = keys.propose_config_change(SUPERVISOR_INSTANCE_ID, propose);
    let block = testkit.create_block_with_transaction(propose);
    // Verify that transaction failed.
    let expected_err =
        ErrorMatch::from_fail(&CommonError::UnauthorizedCaller).for_service(SUPERVISOR_INSTANCE_ID);
    assert_eq!(*block[0].status().unwrap_err(), expected_err);

    testkit.create_blocks_until(cfg_change_height);
    // Verify that no changes have been applied.
    let new_validators = testkit.network().validators();

    assert_eq!(testkit.consensus_config(), old_consensus_config);
    assert_eq!(testkit.network().us().validator_id(), None);
    assert_eq!(old_validators, new_validators);
}

/// Checks that config proposal sent through api is executed correctly.
#[test]
fn test_send_proposal_with_api() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(2)
        .with_rust_service(Supervisor)
        .with_artifact(Supervisor.artifact_id())
        .with_instance(Supervisor::simple())
        .build();

    let old_validators = testkit.network().validators();

    let cfg_change_height = Height(5);
    let new_consensus_config = {
        let mut cfg = testkit.consensus_config();
        // Remove us from validators
        cfg.validator_keys.remove(0);
        cfg
    };

    let config_propose =
        ConfigPropose::new(0, cfg_change_height).consensus_config(new_consensus_config.clone());

    // Create proposal
    let hash: Hash = testkit
        .api()
        .private(ApiKind::Service("supervisor"))
        .query(&config_propose)
        .post("propose-config")
        .unwrap();
    let block = testkit.create_block();
    block[hash].status().unwrap();

    // Assert that config is now pending.
    let snapshot = testkit.snapshot();
    let schema: Schema<_> = snapshot.service_schema(supervisor_name()).unwrap();
    assert_eq!(
        schema.pending_proposal.get().unwrap().config_propose,
        config_propose
    );

    testkit.create_blocks_until(cfg_change_height);

    // Assert that config sent through the api is applied.
    assert_config_change_is_applied(&testkit);
    assert_eq!(testkit.consensus_config(), new_consensus_config);
    assert_eq!(testkit.network().us().validator_id(), None);
    assert_ne!(old_validators, testkit.network().validators());
}

/// Tests that deploy request with only one approval (initial) is executed successfully.
#[test]
fn deploy_service() {
    let mut testkit = TestKitBuilder::validator()
        .with_rust_service(Supervisor)
        .with_artifact(Supervisor.artifact_id())
        .with_instance(Supervisor::simple())
        .with_rust_service(DeployableService)
        .build();

    let deadline_height = Height(5);

    let artifact = DeployableService.artifact_id();
    let deploy_request = DeployRequest {
        artifact: artifact.clone(),
        spec: Vec::new(),
        deadline_height,
    };

    // Create deploy request
    let hash: Hash = testkit
        .api()
        .private(ApiKind::Service("supervisor"))
        .query(&deploy_request)
        .post("deploy-artifact")
        .unwrap();
    let block = testkit.create_block();
    // Check that request was executed.
    block[hash].status().unwrap();

    testkit.create_blocks_until(deadline_height);
    // Verify that after reaching the deadline height artifact is deployed.
    let snapshot = testkit.snapshot();
    assert!(snapshot
        .for_dispatcher()
        .service_artifacts()
        .contains(&artifact));
}

/// Attempts to change config without `actual_from` height set.
/// When `actual_from` is not set, it is expected to be treated as the next height.
#[test]
fn actual_from_is_zero() {
    let initial_validator_count = 4;
    let expected_new_validator_number = initial_validator_count;

    let mut testkit = TestKitBuilder::auditor()
        .with_validators(initial_validator_count)
        .with_rust_service(Supervisor)
        .with_artifact(Supervisor.artifact_id())
        .with_instance(Supervisor::simple())
        .build();

    // Change height set to 0
    let cfg_change_height = Height(0);

    // Sample config change, we don't actually care about what se change.
    let new_consensus_config = {
        let mut cfg = testkit.consensus_config();
        cfg.validator_keys.push(testkit.us().public_keys());
        cfg.validator_keys
            .push(testkit.network_mut().add_node().public_keys());
        cfg
    };

    let initiator_id = testkit.network().validators()[0].validator_id().unwrap();
    let config_propose =
        ConfigPropose::new(0, cfg_change_height).consensus_config(new_consensus_config.clone());

    testkit.create_block_with_transaction(sign_config_propose_transaction(
        &testkit,
        config_propose,
        initiator_id,
    ));

    // Create one block.
    testkit.create_block();

    // Check that config is applied.
    assert_config_change_is_applied(&testkit);
    assert_eq!(
        testkit.network().us().validator_id(),
        Some(ValidatorId(expected_new_validator_number))
    );
    assert_eq!(
        &testkit.network().validators()[expected_new_validator_number as usize],
        testkit.network().us()
    );
    assert_eq!(testkit.consensus_config(), new_consensus_config);
}
