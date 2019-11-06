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
    merkledb::{Entry, ObjectHash, Snapshot},
    messages::{AnyTx, Verified},
    runtime::{
        rust::{CallContext, Service, Transaction},
        ArtifactId, Caller, DispatcherError, ExecutionError, InstanceDescriptor, InstanceId,
    },
};
use exonum_derive::{exonum_service, ServiceFactory};
use exonum_testkit::{TestKit, TestKitBuilder};

use exonum_supervisor::{
    simple::{Error as SimpleSupervisorError, Schema, SimpleSupervisor, SimpleSupervisorInterface},
    ConfigPropose, Configure, DeployRequest, StartService,
};

pub fn sign_config_propose_transaction(
    testkit: &TestKit,
    config: ConfigPropose,
    initiator_id: ValidatorId,
) -> Verified<AnyTx> {
    let (pub_key, sec_key) = &testkit.validator(initiator_id).service_keypair();
    config.sign_for_simple_supervisor(*pub_key, sec_key)
}

pub fn sign_config_propose_transaction_by_us(
    testkit: &TestKit,
    config: ConfigPropose,
) -> Verified<AnyTx> {
    let initiator_id = testkit.network().us().validator_id().unwrap();

    sign_config_propose_transaction(&testkit, config, initiator_id)
}

fn create_deploy_request(artifact: ArtifactId) -> DeployRequest {
    DeployRequest {
        artifact,
        spec: Vec::default(),
        // Deadline height is ignored within simple supervisor.
        deadline_height: Height(0),
    }
}

fn create_start_service(artifact: ArtifactId, name: String) -> StartService {
    StartService {
        artifact,
        name,
        config: Vec::default(),
        // Deadline height is ignored within simple supervisor.
        deadline_height: Height(0),
    }
}

#[derive(Debug, ServiceFactory)]
#[exonum(
    artifact_name = "deployable-test-service",
    artifact_version = "0.1.0",
    implements("DeployableServiceInterface")
)]
pub struct DeployableService;

impl Service for DeployableService {
    fn state_hash(&self, _: InstanceDescriptor, _: &dyn Snapshot) -> Vec<Hash> {
        Vec::new()
    }
}

impl From<DeployableService> for InstanceCollection {
    fn from(instance: DeployableService) -> Self {
        InstanceCollection::new(instance)
    }
}

#[exonum_service]
pub trait DeployableServiceInterface {}

impl DeployableServiceInterface for DeployableService {}

#[derive(Debug, ServiceFactory)]
#[exonum(
    artifact_name = "config-change-test-service",
    implements("Configure<Params = String>")
)]
pub struct ConfigChangeService;

impl ConfigChangeService {
    pub const INSTANCE_ID: InstanceId = 119;
    pub const INSTANCE_NAME: &'static str = "config-change";
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
    fn state_hash(&self, _: InstanceDescriptor<'_>, _: &dyn Snapshot) -> Vec<Hash> {
        Vec::new()
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
            .verify_caller(Caller::as_supervisor)
            .ok_or(DispatcherError::UnauthorizedCaller)?;

        match params.as_ref() {
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
        let (_, fork) = context
            .verify_caller(Caller::as_supervisor)
            .ok_or(DispatcherError::UnauthorizedCaller)?;

        Entry::new(format!("{}.params", context.instance().name), fork).set(params.clone());

        match params.as_ref() {
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
    assert!(!Schema::new(&snapshot).config_propose_entry().exists());
}

#[test]
fn add_nodes_to_validators() {
    let mut testkit = TestKitBuilder::auditor()
        .with_validators(2)
        .with_rust_service(SimpleSupervisor)
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
    let config_propose = ConfigPropose::actual_from(cfg_change_height)
        .consensus_config(new_consensus_config.clone());

    testkit.create_block_with_transaction(sign_config_propose_transaction(
        &testkit,
        config_propose,
        initiator_id,
    ));

    testkit.create_blocks_until(cfg_change_height.previous());
    assert_eq!(testkit.network().us().validator_id(), None);
    testkit.create_block();

    assert_config_change_is_applied(&testkit);
    assert_eq!(testkit.network().us().validator_id(), Some(ValidatorId(2)));
    assert_eq!(&testkit.network().validators()[2], testkit.network().us());
    assert_eq!(testkit.consensus_config(), new_consensus_config);
}

#[test]
fn exclude_us_from_validators() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(2)
        .with_rust_service(SimpleSupervisor)
        .create();

    let cfg_change_height = Height(5);
    let new_consensus_config = {
        let mut cfg = testkit.consensus_config();
        // Remove us from validators
        cfg.validator_keys.remove(0);
        cfg
    };

    let old_validators = testkit.network().validators();

    let config_propose = ConfigPropose::actual_from(cfg_change_height)
        .consensus_config(new_consensus_config.clone());

    testkit.create_block_with_transaction(sign_config_propose_transaction_by_us(
        &testkit,
        config_propose,
    ));
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
        .with_rust_service(SimpleSupervisor)
        .create();

    let cfg_change_height = Height(5);
    let new_consensus_config = {
        let mut cfg = testkit.consensus_config();
        // Remove one of node from validators
        cfg.validator_keys.remove(1);
        cfg
    };

    let config_propose = ConfigPropose::actual_from(cfg_change_height)
        .consensus_config(new_consensus_config.clone());

    testkit.create_block_with_transaction(sign_config_propose_transaction_by_us(
        &testkit,
        config_propose,
    ));
    testkit.create_blocks_until(cfg_change_height);

    assert_eq!(testkit.consensus_config(), new_consensus_config);
    assert_eq!(testkit.network().us().validator_id(), Some(ValidatorId(0)));
}

#[test]
fn change_us_validator_id() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(2)
        .with_rust_service(SimpleSupervisor)
        .create();

    let cfg_change_height = Height(5);
    let new_consensus_config = {
        let mut cfg = testkit.consensus_config();
        // Remove one of node from validators
        cfg.validator_keys.swap(0, 1);
        cfg
    };

    let config_propose = ConfigPropose::actual_from(cfg_change_height)
        .consensus_config(new_consensus_config.clone());

    testkit.create_block_with_transaction(sign_config_propose_transaction_by_us(
        &testkit,
        config_propose,
    ));
    testkit.create_blocks_until(cfg_change_height);

    assert_eq!(testkit.network().us().validator_id(), Some(ValidatorId(1)));
    assert_eq!(&testkit.network().validators()[1], testkit.network().us());
    assert_eq!(testkit.consensus_config(), new_consensus_config);
}

#[test]
fn service_config_change() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(2)
        .with_rust_service(SimpleSupervisor)
        .with_rust_service(ConfigChangeService)
        .create();

    let cfg_change_height = Height(5);
    let params = "I am a new parameter".to_owned();

    let config_propose = ConfigPropose::actual_from(cfg_change_height)
        .service_config(ConfigChangeService::INSTANCE_ID, params.clone());

    testkit.create_block_with_transaction(sign_config_propose_transaction_by_us(
        &testkit,
        config_propose,
    ));
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
        .with_rust_service(SimpleSupervisor)
        .with_rust_service(ConfigChangeService)
        .create();

    let cfg_change_height = Height(5);
    let params = "I am a discarded parameter".to_owned();
    let new_consensus_config = {
        let mut cfg = testkit.consensus_config();
        // Remove us from validators
        cfg.validator_keys.remove(0);
        cfg
    };

    let config_propose = ConfigPropose::actual_from(cfg_change_height)
        .service_config(ConfigChangeService::INSTANCE_ID, params)
        .service_config(ConfigChangeService::INSTANCE_ID, "error".to_owned())
        .consensus_config(new_consensus_config);

    testkit.create_block_with_transaction(sign_config_propose_transaction_by_us(
        &testkit,
        config_propose,
    ));
    testkit.create_blocks_until(cfg_change_height);
    assert_config_change_is_applied(&testkit);

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
        .with_rust_service(SimpleSupervisor)
        .with_rust_service(ConfigChangeService)
        .create();

    let cfg_change_height = Height(5);
    let params = "I am a discarded parameter".to_owned();
    let new_consensus_config = {
        let mut cfg = testkit.consensus_config();
        // Remove us from validators
        cfg.validator_keys.remove(0);
        cfg
    };

    let config_propose = ConfigPropose::actual_from(cfg_change_height)
        .service_config(ConfigChangeService::INSTANCE_ID, params.clone())
        .service_config(ConfigChangeService::INSTANCE_ID, "panic".to_owned())
        .consensus_config(new_consensus_config);

    testkit.create_block_with_transaction(sign_config_propose_transaction_by_us(
        &testkit,
        config_propose,
    ));
    testkit.create_blocks_until(cfg_change_height);
    assert_config_change_is_applied(&testkit);

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
        .with_rust_service(SimpleSupervisor)
        .with_rust_service(ConfigChangeService)
        .create();

    let cfg_change_height = Height(5);
    let params = "I am a new parameter".to_owned();

    testkit.create_blocks_until(cfg_change_height);

    let config_propose = ConfigPropose::actual_from(cfg_change_height)
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

#[test]
fn another_configuration_change_proposal() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(2)
        .with_rust_service(SimpleSupervisor)
        .with_rust_service(ConfigChangeService)
        .create();

    let cfg_change_height = Height(5);
    let params = "I am a new parameter".to_owned();

    let config_propose = ConfigPropose::actual_from(cfg_change_height)
        .service_config(ConfigChangeService::INSTANCE_ID, params.clone());

    testkit.create_block_with_transaction(sign_config_propose_transaction_by_us(
        &testkit,
        config_propose,
    ));

    let config_propose = ConfigPropose::actual_from(cfg_change_height).service_config(
        ConfigChangeService::INSTANCE_ID,
        "I am an overridden parameter".to_owned(),
    );

    // Try to commit second config change propose.
    testkit
        .create_block_with_transaction(sign_config_propose_transaction_by_us(
            &testkit,
            config_propose,
        ))
        .transactions[0]
        .status()
        .unwrap_err();
    testkit.create_blocks_until(cfg_change_height);
    assert_config_change_is_applied(&testkit);

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

    let (pub_key, sec_key) = crypto::gen_keypair();

    let mut testkit = TestKitBuilder::validator()
        .with_validators(2)
        .with_rust_service(InstanceCollection::new(SimpleSupervisor).with_instance(
            FAKE_SUPERVISOR_ID,
            "fake-supervisor",
            Vec::default(),
        ))
        .with_rust_service(ConfigChangeService)
        .create();

    let cfg_change_height = Height(5);
    let params = "I am a new parameter".to_owned();

    let propose = ConfigPropose::actual_from(cfg_change_height)
        .service_config(ConfigChangeService::INSTANCE_ID, params.clone());
    let tx = Transaction::<dyn SimpleSupervisorInterface>::sign(
        propose,
        FAKE_SUPERVISOR_ID,
        pub_key,
        &sec_key,
    );
    testkit.create_block_with_transaction(tx).transactions[0]
        .status()
        .unwrap_err();
}

#[test]
fn test_configuration_and_rollbacks() {
    let mut testkit = TestKitBuilder::auditor()
        .with_validators(1)
        .with_rust_service(SimpleSupervisor)
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

    // Sign request by validator (we're an auditor yet).
    let initiator_id = testkit.network().validators()[0].validator_id().unwrap();
    let config_propose =
        ConfigPropose::actual_from(cfg_change_height).consensus_config(new_config.clone());

    testkit.create_block_with_transaction(sign_config_propose_transaction(
        &testkit,
        config_propose,
        initiator_id,
    ));

    testkit.create_blocks_until(cfg_change_height);
    assert_config_change_is_applied(&testkit);
    assert_eq!(testkit.consensus_config(), new_config);

    testkit.checkpoint();
    testkit.create_block();
    testkit.rollback();
    assert_eq!(testkit.consensus_config(), new_config);
    assert_config_change_is_applied(&testkit);

    testkit.rollback();

    // As rollback is behind the time a proposal entered the blockchain,
    // the proposal is effectively forgotten.
    testkit.create_blocks_until(Height(10));
    assert_eq!(testkit.consensus_config(), old_config);
    assert_config_change_is_applied(&testkit);
}

#[test]
fn service_config_rollback_apply_error() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(2)
        .with_rust_service(SimpleSupervisor)
        .with_rust_service(ConfigChangeService)
        .create();

    let cfg_change_height = Height(5);
    let params = "apply_error".to_owned();

    let config_propose = ConfigPropose::actual_from(cfg_change_height)
        .service_config(ConfigChangeService::INSTANCE_ID, params.clone());

    testkit.create_block_with_transaction(sign_config_propose_transaction_by_us(
        &testkit,
        config_propose,
    ));
    testkit.create_blocks_until(cfg_change_height);
    assert_config_change_is_applied(&testkit);

    let actual_params: Option<String> = Entry::new(
        format!("{}.params", ConfigChangeService::INSTANCE_NAME),
        &testkit.snapshot(),
    )
    .get();

    assert!(actual_params.is_none());
}

#[test]
fn service_config_rollback_apply_panic() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(2)
        .with_rust_service(SimpleSupervisor)
        .with_rust_service(ConfigChangeService)
        .create();

    let cfg_change_height = Height(5);
    let params = "apply_panic".to_owned();

    let config_propose = ConfigPropose::actual_from(cfg_change_height)
        .service_config(ConfigChangeService::INSTANCE_ID, params.clone());

    testkit.create_block_with_transaction(sign_config_propose_transaction_by_us(
        &testkit,
        config_propose,
    ));
    testkit.create_blocks_until(cfg_change_height);
    assert_config_change_is_applied(&testkit);

    let actual_params: Option<String> = Entry::new(
        format!("{}.params", ConfigChangeService::INSTANCE_NAME),
        &testkit.snapshot(),
    )
    .get();

    assert!(actual_params.is_none());
}

#[test]
fn service_config_apply_multiple_configs() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(2)
        .with_rust_service(SimpleSupervisor)
        .with_rust_service(ConfigChangeService)
        .create();

    let cfg_change_height = Height(5);
    let params = "I am a new parameter".to_owned();

    let config_propose = ConfigPropose::actual_from(cfg_change_height)
        .service_config(ConfigChangeService::INSTANCE_ID, params.clone())
        .service_config(ConfigChangeService::INSTANCE_ID, "apply_panic".to_owned())
        .service_config(ConfigChangeService::INSTANCE_ID, "apply_error".to_owned());

    testkit.create_block_with_transaction(sign_config_propose_transaction_by_us(
        &testkit,
        config_propose,
    ));
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
fn several_service_config_changes() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(2)
        .with_rust_service(SimpleSupervisor)
        .with_rust_service(ConfigChangeService)
        .create();

    for i in 1..5 {
        let cfg_change_height = Height(5 * i);
        let params = format!("Change {}", i);

        let config_propose = ConfigPropose::actual_from(cfg_change_height)
            .service_config(ConfigChangeService::INSTANCE_ID, params.clone());

        let tx = sign_config_propose_transaction_by_us(&testkit, config_propose);

        testkit.create_block_with_transaction(tx)[0]
            .status()
            .unwrap();

        testkit.create_blocks_until(cfg_change_height);
        assert_config_change_is_applied(&testkit);
    }

    let actual_params: String = Entry::new(
        format!("{}.params", ConfigChangeService::INSTANCE_NAME),
        &testkit.snapshot(),
    )
    .get()
    .unwrap();

    assert_eq!(actual_params, "Change 4");
}

#[test]
fn discard_config_propose_from_auditor() {
    let mut testkit = TestKitBuilder::auditor()
        .with_validators(2)
        .with_rust_service(SimpleSupervisor)
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
    let config_propose = ConfigPropose::actual_from(cfg_change_height)
        .consensus_config(new_consensus_config.clone())
        .sign_for_simple_supervisor(keys.0, &keys.1);

    let tx_hash = config_propose.object_hash();

    testkit.create_block_with_transaction(config_propose);
    testkit.create_blocks_until(cfg_change_height);

    // Verify that transaction failed.
    let api = testkit.api();
    let system_api = api.exonum_api();
    let expected_status = Err(exonum::blockchain::ExecutionError {
        kind: exonum::blockchain::ExecutionErrorKind::Service { code: 5 },
        description: "Transaction author is not a validator.".into(),
    });
    system_api.assert_tx_status(tx_hash, &expected_status.into());

    // Verify that no changes have been applied.
    let new_validators = testkit.network().validators();

    assert_eq!(testkit.consensus_config(), old_consensus_config);
    assert_eq!(testkit.network().us().validator_id(), None);
    assert_eq!(old_validators, new_validators);
}

#[test]
fn test_send_proposal_with_api() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(2)
        .with_rust_service(SimpleSupervisor)
        .create();

    let old_validators = testkit.network().validators();

    let cfg_change_height = Height(5);
    let new_consensus_config = {
        let mut cfg = testkit.consensus_config();
        // Remove us from validators
        cfg.validator_keys.remove(0);
        cfg
    };

    let config_propose = ConfigPropose::actual_from(cfg_change_height)
        .consensus_config(new_consensus_config.clone());

    // Create proposal
    let hash = {
        let hash: Hash = testkit
            .api()
            .private(exonum_testkit::ApiKind::Service("simple-supervisor"))
            .query(&config_propose)
            .post("propose-config")
            .unwrap();
        hash
    };
    testkit.create_block();
    testkit.api().exonum_api().assert_tx_success(hash);

    // Assert that config is now pending.
    assert_eq!(
        Schema::new(&testkit.snapshot())
            .config_propose_entry()
            .get()
            .unwrap(),
        config_propose
    );

    testkit.create_blocks_until(cfg_change_height);

    // Assert that config sent through the api is applied.
    assert_config_change_is_applied(&testkit);
    assert_eq!(testkit.consensus_config(), new_consensus_config);
    assert_eq!(testkit.network().us().validator_id(), None);
    assert_ne!(old_validators, testkit.network().validators());
}

/// Tests that correct deploy request is executed successfully.
#[test]
fn deploy_service() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(2)
        .with_rust_service(SimpleSupervisor)
        .with_rust_service(DeployableService)
        .create();

    let artifact_id = ArtifactId::new(0_u32, "deployable-test-service:0.1.0").unwrap();
    let deploy_request = create_deploy_request(artifact_id.clone());

    let cfg_change_height = Height(3);

    let config_propose =
        ConfigPropose::actual_from(cfg_change_height).deploy_request(deploy_request);
    let deploy_tx = sign_config_propose_transaction_by_us(&testkit, config_propose);

    let tx_hash = deploy_tx.object_hash();

    testkit.create_block_with_transaction(deploy_tx);

    // Verify that transaction succeed.
    let api = testkit.api();
    let system_api = api.exonum_api();
    system_api.assert_tx_success(tx_hash);

    // Verify that after the block commit (but before config change height)
    // artifact is not deployed.
    assert_eq!(
        system_api.services().artifacts.contains(&artifact_id),
        false
    );

    // Reach config change height.
    testkit.create_blocks_until(cfg_change_height);

    // Verify that after the config change height artifact is deployed.
    assert_eq!(system_api.services().artifacts.contains(&artifact_id), true);
}

/// Tests that incorrect deploy requests result in the tx execution error.
#[test]
fn deploy_service_errors() {
    // Start with a deployment of the service.
    let mut testkit = TestKitBuilder::validator()
        .with_validators(2)
        .with_rust_service(SimpleSupervisor)
        .with_rust_service(DeployableService)
        .create();

    let artifact_id = ArtifactId::new(0_u32, "deployable-test-service:0.1.0").unwrap();
    let deploy_request = create_deploy_request(artifact_id.clone());

    let cfg_change_height = Height(3);

    let config_propose =
        ConfigPropose::actual_from(cfg_change_height).deploy_request(deploy_request);
    let deploy_tx = sign_config_propose_transaction_by_us(&testkit, config_propose);
    testkit.create_block_with_transaction(deploy_tx);
    testkit.create_blocks_until(cfg_change_height);

    // Prepare test data.
    let api = testkit.api();
    let system_api = api.exonum_api();

    let bad_name_msg =
        "Artifact name contains an illegal character, use only: a-zA-Z0-9 and one of _-.:";
    let incorrect_artifact_id_err = (SimpleSupervisorError::InvalidArtifactId, bad_name_msg).into();

    // Declare different malformed deploy requests and expected errors.
    let test_vector: Vec<(ArtifactId, ExecutionError)> = vec![
        // Already deployed artifact.
        (artifact_id, SimpleSupervisorError::AlreadyDeployed.into()),
        // Incorrect artifact name.
        (
            ArtifactId {
                runtime_id: 0,
                name: "$#@$:0.1.0".into(),
            },
            incorrect_artifact_id_err,
        ),
    ];

    // We don't really care about it, since all requests should fail.
    let cfg_change_height = Height(100);

    // For each pair in test vector check that transaction fails with expected result.
    for (artifact_id, expected) in test_vector.into_iter() {
        let deploy_request = create_deploy_request(artifact_id);
        let config_propose =
            ConfigPropose::actual_from(cfg_change_height).deploy_request(deploy_request);

        let deploy_tx = sign_config_propose_transaction_by_us(&testkit, config_propose);
        let tx_hash = deploy_tx.object_hash();
        testkit.create_block_with_transaction(deploy_tx);

        system_api.assert_tx_status(tx_hash, &Err(expected).into());
    }
}

/// Tests that correct service instance start request is executed successfully.
#[test]
fn init_service() {
    // At first, deploy the service (w/o any checks, since we're not testing deploy).
    let mut testkit = TestKitBuilder::validator()
        .with_validators(2)
        .with_rust_service(SimpleSupervisor)
        .with_rust_service(DeployableService)
        .create();

    let artifact_id = ArtifactId::new(0_u32, "deployable-test-service:0.1.0").unwrap();
    let deploy_request = create_deploy_request(artifact_id.clone());

    let cfg_change_height = Height(3);

    let config_propose =
        ConfigPropose::actual_from(cfg_change_height).deploy_request(deploy_request);
    let deploy_tx = sign_config_propose_transaction_by_us(&testkit, config_propose);
    testkit.create_block_with_transaction(deploy_tx);
    testkit.create_blocks_until(cfg_change_height);

    // Initialize the service.

    let cfg_change_height = Height(6);

    let start_service = create_start_service(artifact_id.clone(), "example".into());
    let config_propose = ConfigPropose::actual_from(cfg_change_height).start_service(start_service);

    let deploy_tx = sign_config_propose_transaction_by_us(&testkit, config_propose);
    let tx_hash = deploy_tx.object_hash();

    testkit.create_block_with_transaction(deploy_tx);

    // Verify that transaction succeed.
    let api = testkit.api();
    let system_api = api.exonum_api();
    system_api.assert_tx_success(tx_hash);

    // Verify that after the block commit (but before config change height)
    // instance is not running yet.
    assert_eq!(
        system_api
            .services()
            .services
            .iter()
            .any(|instance| instance.name == "example"),
        false
    );

    // Reach config change height.
    testkit.create_blocks_until(cfg_change_height);

    // Verify that after the config change height instance is running.
    let instance = system_api
        .services()
        .services
        .iter()
        .find(|instance| instance.name == "example")
        .cloned()
        .expect("Service did not start");

    assert_eq!(instance.artifact, artifact_id);
}

/// Tests that incorrect service instance start requests result in the tx execution error.
#[test]
fn init_service_errors() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(2)
        .with_rust_service(SimpleSupervisor)
        .with_rust_service(DeployableService)
        .create();

    // Deploy service.
    let cfg_change_height = Height(3);

    let artifact_id = ArtifactId::new(0_u32, "deployable-test-service:0.1.0").unwrap();
    let deploy_request = create_deploy_request(artifact_id.clone());

    let config_propose =
        ConfigPropose::actual_from(cfg_change_height).deploy_request(deploy_request);
    let deploy_tx = sign_config_propose_transaction_by_us(&testkit, config_propose);
    testkit.create_block_with_transaction(deploy_tx);
    testkit.create_blocks_until(cfg_change_height);

    // Init service.
    let cfg_change_height = Height(6);

    let start_service = create_start_service(artifact_id.clone(), "example".into());
    let config_propose = ConfigPropose::actual_from(cfg_change_height).start_service(start_service);
    let deploy_tx = sign_config_propose_transaction_by_us(&testkit, config_propose);
    testkit.create_block_with_transaction(deploy_tx);
    testkit.create_blocks_until(cfg_change_height);

    // Prepare test data.
    let api = testkit.api();
    let system_api = api.exonum_api();

    let bad_name_msg =
        "Service instance name contains illegal character, use only: a-zA-Z0-9 and one of _-.";
    let incorrect_instance_name_err =
        (SimpleSupervisorError::InvalidInstanceName, bad_name_msg).into();

    // Declare different malformed init requests and expected errors.
    let test_vector: Vec<(ArtifactId, String, ExecutionError)> = vec![
        // Unknown artifact ID.
        (
            ArtifactId {
                runtime_id: 0,
                name: "unknown-artifact:0.1.0".into(),
            },
            String::from("instance"),
            SimpleSupervisorError::UnknownArtifact.into(),
        ),
        // Bad instance name.
        (
            artifact_id.clone(),
            String::from("#$#&#"),
            incorrect_instance_name_err,
        ),
        // Already running instance.
        (
            artifact_id.clone(),
            String::from("example"),
            SimpleSupervisorError::InstanceExists.into(),
        ),
    ];

    // We don't really care about it, since all requests should fail.
    let cfg_change_height = Height(100);

    // For each pair in test vector check that transaction fails with expected result.
    for (artifact_id, name, expected) in test_vector.into_iter() {
        let start_service = create_start_service(artifact_id, name);
        let config_propose =
            ConfigPropose::actual_from(cfg_change_height).start_service(start_service);

        let deploy_tx = sign_config_propose_transaction_by_us(&testkit, config_propose);
        let tx_hash = deploy_tx.object_hash();
        testkit.create_block_with_transaction(deploy_tx);

        system_api.assert_tx_status(tx_hash, &Err(expected).into());
    }
}
