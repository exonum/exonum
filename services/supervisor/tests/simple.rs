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

//! This file contains tests for the Supervisor in the simple mode.
//! Since simple Supervisor only difference from decentralized one is
//! decision-making algorithm, the tests affect only this aspect.

use exonum::{
    crypto::Hash,
    helpers::{Height, ValidatorId},
    messages::{AnyTx, Verified},
    runtime::{
        rust::{BuiltinInstance, CallContext, Service},
        ArtifactId, BlockchainData, DispatcherError, ExecutionError, InstanceId, SnapshotExt,
    },
};
use exonum_derive::{exonum_interface, ServiceDispatcher, ServiceFactory};
use exonum_merkledb::{access::AccessExt, ObjectHash, Snapshot};
use exonum_testkit::{TestKit, TestKitBuilder};

use exonum_supervisor::{
    supervisor_name, ConfigPropose, Configure, DeployRequest, Schema, SimpleSupervisor,
};

pub fn sign_config_propose_transaction(
    testkit: &TestKit,
    config: ConfigPropose,
    initiator_id: ValidatorId,
) -> Verified<AnyTx> {
    let (pub_key, sec_key) = &testkit.validator(initiator_id).service_keypair();
    config.sign_for_supervisor(*pub_key, sec_key)
}

pub fn sign_config_propose_transaction_by_us(
    testkit: &TestKit,
    config: ConfigPropose,
) -> Verified<AnyTx> {
    let initiator_id = testkit.network().us().validator_id().unwrap();

    sign_config_propose_transaction(&testkit, config, initiator_id)
}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_dispatcher(implements("Configure<Params=String>"))]
#[service_factory(artifact_name = "config-change-test-service")]
pub struct ConfigChangeService;

impl BuiltinInstance for ConfigChangeService {
    const INSTANCE_ID: InstanceId = 119;
    const INSTANCE_NAME: &'static str = "config-change";
}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_dispatcher(implements("DeployableServiceInterface"))]
#[service_factory(artifact_name = "deployable-test-service", artifact_version = "0.1.0")]
pub struct DeployableService;

impl Service for DeployableService {
    fn state_hash(&self, _data: BlockchainData<&dyn Snapshot>) -> Vec<Hash> {
        vec![]
    }
}

#[exonum_interface]
pub trait DeployableServiceInterface {}

impl DeployableServiceInterface for DeployableService {}

impl Service for ConfigChangeService {
    fn state_hash(&self, _data: BlockchainData<&dyn Snapshot>) -> Vec<Hash> {
        vec![]
    }
}

impl Configure for ConfigChangeService {
    type Params = String;

    fn verify_config(
        &self,
        context: CallContext<'_>,
        params: Self::Params,
    ) -> Result<(), ExecutionError> {
        context
            .caller()
            .as_supervisor()
            .ok_or(DispatcherError::UnauthorizedCaller)?;

        match params.as_str() {
            "error" => Err(DispatcherError::malformed_arguments("Error!")).map_err(From::from),
            "panic" => panic!("Aaaa!"),
            _ => Ok(()),
        }
    }

    fn apply_config(
        &self,
        context: CallContext<'_>,
        params: Self::Params,
    ) -> Result<(), ExecutionError> {
        context
            .caller()
            .as_supervisor()
            .ok_or(DispatcherError::UnauthorizedCaller)?;

        context
            .service_data()
            .get_entry("params")
            .set(params.clone());

        match params.as_str() {
            "apply_error" => {
                Err(DispatcherError::malformed_arguments("Error!")).map_err(From::from)
            }
            "apply_panic" => panic!("Aaaa!"),
            _ => Ok(()),
        }
    }
}

fn assert_config_change_is_applied(testkit: &TestKit) {
    let snapshot = testkit.snapshot();
    assert!(
        !Schema::new(snapshot.for_service(supervisor_name()).unwrap())
            .pending_proposal
            .exists()
    );
}

/// Attempts to change consensus config with only one confirmation.
#[test]
fn change_consensus_config_with_one_confirmation() {
    let initial_validator_count = 4;
    let expected_new_validator_number = initial_validator_count;

    let mut testkit = TestKitBuilder::auditor()
        .with_validators(initial_validator_count)
        .with_builtin_rust_service(SimpleSupervisor::new())
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
        .with_builtin_rust_service(SimpleSupervisor::new())
        .with_builtin_rust_service(ConfigChangeService)
        .create();

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
        .with_builtin_rust_service(SimpleSupervisor::new())
        .with_builtin_rust_service(ConfigChangeService)
        .create();

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
        .with_builtin_rust_service(SimpleSupervisor::new())
        .create();

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
    let keys = &testkit.network().us().service_keypair();
    let config_propose = ConfigPropose::new(0, cfg_change_height)
        .consensus_config(new_consensus_config.clone())
        .sign_for_supervisor(keys.0, &keys.1);

    let tx_hash = config_propose.object_hash();

    testkit.create_block_with_transaction(config_propose);
    testkit.create_blocks_until(cfg_change_height);

    // Verify that transaction failed.
    let api = testkit.api();
    let system_api = api.exonum_api();
    let expected_status = Err(exonum::blockchain::ExecutionError {
        kind: exonum::blockchain::ExecutionErrorKind::Service { code: 1 },
        description: "Transaction author is not a validator.".into(),
    });
    system_api.assert_tx_status(tx_hash, &expected_status.into());

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
        .with_builtin_rust_service(SimpleSupervisor::new())
        .create();

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
    let hash = {
        let hash: Hash = testkit
            .api()
            .private(exonum_testkit::ApiKind::Service("supervisor"))
            .query(&config_propose)
            .post("propose-config")
            .unwrap();
        hash
    };
    testkit.create_block();
    testkit.api().exonum_api().assert_tx_success(hash);

    // Assert that config is now pending.
    let snapshot = testkit.snapshot();
    let snapshot = snapshot.for_service(supervisor_name()).unwrap();
    assert_eq!(
        Schema::new(snapshot)
            .pending_proposal
            .get()
            .unwrap()
            .config_propose,
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
        .with_builtin_rust_service(SimpleSupervisor::new())
        .with_rust_service(DeployableService)
        .create();

    let deadline_height = Height(5);

    let artifact = ArtifactId::new(0_u32, "deployable-test-service:0.1.0").unwrap();
    let deploy_request = DeployRequest {
        artifact: artifact.clone(),
        spec: Vec::new(),
        deadline_height,
    };

    // Create deploy request
    let hash = testkit
        .api()
        .private(exonum_testkit::ApiKind::Service("supervisor"))
        .query(&deploy_request)
        .post("deploy-artifact")
        .unwrap();
    testkit.create_block();

    testkit.create_blocks_until(deadline_height);

    // Check that request was executed.
    let api = testkit.api();
    let system_api = api.exonum_api();
    system_api.assert_tx_success(hash);

    // Verify that after reaching the deadline height artifact is deployed.
    assert_eq!(system_api.services().artifacts.contains(&artifact), true);
}

/// Attempts to change config without `actual_from` height set.
/// When `actual_from` is not set, it is expected to be treated as the next height.
#[test]
fn actual_from_is_zero() {
    let initial_validator_count = 4;
    let expected_new_validator_number = initial_validator_count;

    let mut testkit = TestKitBuilder::auditor()
        .with_validators(initial_validator_count)
        .with_builtin_rust_service(SimpleSupervisor::new())
        .create();

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
