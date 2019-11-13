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
    messages::{AnyTx, Verified},
    runtime::{
        rust::{CallContext, Service, Transaction},
        BlockchainData, DispatcherError, ExecutionError, InstanceId, SnapshotExt,
        SUPERVISOR_INSTANCE_ID,
    },
};
use exonum_derive::{ServiceDispatcher, ServiceFactory};
use exonum_merkledb::{access::AccessExt, ObjectHash, Snapshot};
use exonum_testkit::{TestKit, TestKitBuilder};

use exonum_supervisor::{
    simple::{Schema, SimpleSupervisor, SimpleSupervisorInterface},
    ConfigPropose, Configure,
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

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_dispatcher(implements("Configure<Params=String>"))]
#[service_factory(artifact_name = "config-change-test-service")]
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
            vec![],
        )
    }
}

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
        !Schema::new(snapshot.for_service(SUPERVISOR_INSTANCE_ID).unwrap())
            .config_propose
            .exists()
    );
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

    assert!(!testkit
        .snapshot()
        .for_service(ConfigChangeService::INSTANCE_NAME)
        .unwrap()
        .get_entry::<_, String>("params")
        .exists());
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

    assert!(!testkit
        .snapshot()
        .for_service(ConfigChangeService::INSTANCE_NAME)
        .unwrap()
        .get_entry::<_, String>("params")
        .exists());
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

    let actual_params = testkit
        .snapshot()
        .for_service(ConfigChangeService::INSTANCE_NAME)
        .unwrap()
        .get_entry::<_, String>("params")
        .get();

    assert_eq!(actual_params, Some(params));
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
            vec![],
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

    assert!(!testkit
        .snapshot()
        .for_service(ConfigChangeService::INSTANCE_NAME)
        .unwrap()
        .get_entry::<_, String>("params")
        .exists());
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

    assert!(!testkit
        .snapshot()
        .for_service(ConfigChangeService::INSTANCE_NAME)
        .unwrap()
        .get_entry::<_, String>("params")
        .exists());
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

    let actual_params = testkit
        .snapshot()
        .for_service(ConfigChangeService::INSTANCE_NAME)
        .unwrap()
        .get_entry::<_, String>("params")
        .get();

    assert_eq!(actual_params, Some(params));
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

    let actual_params = testkit
        .snapshot()
        .for_service(ConfigChangeService::INSTANCE_NAME)
        .unwrap()
        .get_entry::<_, String>("params")
        .get();

    assert_eq!(actual_params, Some("Change 4".to_owned()));
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
    let snapshot = testkit.snapshot();
    let snapshot = snapshot.for_service(SUPERVISOR_INSTANCE_ID).unwrap();
    assert_eq!(
        Schema::new(snapshot).config_propose.get().unwrap(),
        config_propose
    );

    testkit.create_blocks_until(cfg_change_height);

    // Assert that config sent through the api is applied.
    assert_config_change_is_applied(&testkit);
    assert_eq!(testkit.consensus_config(), new_consensus_config);
    assert_eq!(testkit.network().us().validator_id(), None);
    assert_ne!(old_validators, testkit.network().validators());
}
