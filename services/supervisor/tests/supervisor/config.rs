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

use exonum_merkledb::{Entry, ObjectHash, Snapshot};
use exonum_testkit::{TestKit, TestKitBuilder};

use exonum::runtime::ServiceConfig;
use exonum::{
    blockchain::{ConsensusConfig, ExecutionError, InstanceCollection},
    crypto::{self, Hash},
    helpers::{Height, ValidatorId},
    messages::{AnyTx, Verified},
    runtime::{
        rust::{
            interfaces::verify_caller_is_supervisor, Configure, Service, Transaction,
            TransactionContext,
        },
        ConfigChange, DispatcherError, InstanceDescriptor, InstanceId, SUPERVISOR_INSTANCE_ID,
        SUPERVISOR_INSTANCE_NAME,
    },
};
use exonum_derive::ServiceFactory;

use exonum_supervisor::{ConfigPropose, ConfigVote, Schema, Supervisor};

use crate::proto;

fn config_propose_entry(testkit: &TestKit) -> Option<ConfigPropose> {
    let snapshot = testkit.snapshot();
    Schema::new(SUPERVISOR_INSTANCE_NAME, &snapshot)
        .config_propose_entry()
        .get()
}

fn sign_config_propose_transaction(
    testkit: &TestKit,
    config: ConfigPropose,
    initiator_id: ValidatorId,
) -> Verified<AnyTx> {
    let keys = &testkit.validator(initiator_id).service_keypair();
    config.sign(SUPERVISOR_INSTANCE_ID, keys.0, &keys.1)
}

fn build_confirmation_transactions(
    testkit: &TestKit,
    proposal_hash: Hash,
    initiator_id: ValidatorId,
) -> Vec<Verified<AnyTx>> {
    testkit
        .network()
        .validators()
        .iter()
        .filter(|validator| validator.validator_id() != Some(initiator_id))
        .map(|validator| {
            let keys = validator.service_keypair();
            ConfigVote {
                propose_hash: proposal_hash,
            }
            .sign(SUPERVISOR_INSTANCE_ID, keys.0, &keys.1)
        })
        .collect()
}

fn build_consensus_config_propose(
    new_consensus_config: ConsensusConfig,
    actual_from: Height,
) -> ConfigPropose {
    ConfigPropose {
        actual_from,
        changes: vec![ConfigChange::Consensus(new_consensus_config.clone())],
    }
}

#[test]
fn test_add_nodes_to_validators() {
    let mut testkit = TestKitBuilder::auditor()
        .with_validators(1)
        .with_service(Supervisor)
        .create();

    let cfg_change_height = Height(5);
    let new_node_keys = testkit.network_mut().add_node().public_keys();
    let new_consensus_config = {
        let mut cfg = testkit.consensus_config();
        // Add us node.
        cfg.validator_keys.push(testkit.us().public_keys());
        // Add new node.
        cfg.validator_keys.push(new_node_keys);
        cfg
    };

    let new_config_propose =
        build_consensus_config_propose(new_consensus_config.clone(), cfg_change_height);
    let signed_proposal =
        sign_config_propose_transaction(&testkit, new_config_propose, ValidatorId(0));
    testkit.create_block_with_transaction(signed_proposal);

    testkit.create_blocks_until(cfg_change_height);

    assert_eq!(config_propose_entry(&testkit), None);
    assert_eq!(testkit.network().us().validator_id(), Some(ValidatorId(1)));
    assert_eq!(testkit.consensus_config(), new_consensus_config);
}

#[test]
fn exclude_us_from_validators() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(4)
        .with_service(Supervisor)
        .create();
    let initiator_id = testkit.network().us().validator_id().unwrap();

    let cfg_change_height = Height(5);
    let new_consensus_config = {
        let mut cfg = testkit.consensus_config();
        // Remove us from validators
        cfg.validator_keys.remove(0);
        cfg
    };

    let old_validators = testkit.network().validators();

    let config_proposal =
        build_consensus_config_propose(new_consensus_config.clone(), cfg_change_height);
    let proposal_hash = config_proposal.object_hash();
    testkit.create_block_with_transaction(sign_config_propose_transaction(
        &testkit,
        config_proposal,
        initiator_id,
    ));

    let signed_txs = build_confirmation_transactions(&testkit, proposal_hash, initiator_id);
    testkit.create_block_with_transactions(signed_txs);

    testkit.create_blocks_until(cfg_change_height);

    let new_validators = testkit.network().validators();

    assert_eq!(config_propose_entry(&testkit), None);
    assert_eq!(testkit.consensus_config(), new_consensus_config);
    assert_eq!(testkit.network().us().validator_id(), None);
    assert_ne!(old_validators, new_validators);
}

#[test]
fn exclude_other_from_validators() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(4)
        .with_service(Supervisor)
        .create();
    let initiator_id = testkit.network().us().validator_id().unwrap();

    let cfg_change_height = Height(5);
    let new_consensus_config = {
        let mut cfg = testkit.consensus_config();
        // Remove one of node from validators
        cfg.validator_keys.remove(1);
        cfg
    };

    let config_proposal =
        build_consensus_config_propose(new_consensus_config.clone(), cfg_change_height);
    let proposal_hash = config_proposal.object_hash();

    testkit.create_block_with_transaction(sign_config_propose_transaction(
        &testkit,
        config_proposal,
        initiator_id,
    ));

    let signed_txs = build_confirmation_transactions(&testkit, proposal_hash, initiator_id);
    testkit.create_block_with_transactions(signed_txs);

    testkit.create_blocks_until(cfg_change_height);

    assert_eq!(config_propose_entry(&testkit), None);
    assert_eq!(testkit.consensus_config(), new_consensus_config);
    assert_eq!(testkit.network().us().validator_id(), Some(initiator_id));
}

#[test]
fn change_us_validator_id() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(4)
        .with_service(Supervisor)
        .create();
    let initiator_id = testkit.network().us().validator_id().unwrap();

    let cfg_change_height = Height(5);
    let new_consensus_config = {
        let mut cfg = testkit.consensus_config();
        // Change us validator id with another
        cfg.validator_keys.swap(0, 1);
        cfg
    };

    let config_proposal =
        build_consensus_config_propose(new_consensus_config.clone(), cfg_change_height);
    let proposal_hash = config_proposal.object_hash();

    testkit.create_block_with_transaction(sign_config_propose_transaction(
        &testkit,
        config_proposal,
        initiator_id,
    ));

    let signed_txs = build_confirmation_transactions(&testkit, proposal_hash, initiator_id);
    testkit.create_block_with_transactions(signed_txs);

    testkit.create_blocks_until(cfg_change_height);

    assert_eq!(config_propose_entry(&testkit), None);
    assert_eq!(testkit.network().us().validator_id(), Some(ValidatorId(1)));
    assert_eq!(&testkit.network().validators()[1], testkit.network().us());
    assert_eq!(testkit.consensus_config(), new_consensus_config);
}

#[test]
fn deadline_config_exceeded() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(4)
        .with_service(Supervisor)
        .create();
    let initiator_id = testkit.network().us().validator_id().unwrap();

    let cfg_change_height = Height(5);
    let new_consensus_config = {
        let mut cfg = testkit.consensus_config();
        // Remove us from validators
        cfg.validator_keys.remove(0);
        cfg
    };

    let config_proposal =
        build_consensus_config_propose(new_consensus_config.clone(), cfg_change_height);
    testkit.create_block_with_transaction(sign_config_propose_transaction(
        &testkit,
        config_proposal,
        initiator_id,
    ));
    testkit.create_blocks_until(cfg_change_height.next());

    assert_eq!(config_propose_entry(&testkit), None);
}

#[test]
fn sent_new_config_after_expired_one() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(4)
        .with_service(Supervisor)
        .create();
    let initiator_id = testkit.network().us().validator_id().unwrap();

    let cfg_change_height = Height(5);
    let new_consensus_config = {
        let mut cfg = testkit.consensus_config();
        // Remove us from validators
        cfg.validator_keys.remove(0);
        cfg
    };

    let config_proposal =
        build_consensus_config_propose(new_consensus_config.clone(), cfg_change_height);

    testkit.create_block_with_transaction(sign_config_propose_transaction(
        &testkit,
        config_proposal,
        initiator_id,
    ));
    testkit.create_blocks_until(cfg_change_height);
    testkit.create_block();
    assert_eq!(config_propose_entry(&testkit), None);

    // Send config one more time and vote for it
    let cfg_change_height = Height(10);
    let new_consensus_config = {
        let mut cfg = testkit.consensus_config();
        // Remove us from validators
        cfg.validator_keys.remove(1);
        cfg
    };

    let config_proposal =
        build_consensus_config_propose(new_consensus_config.clone(), cfg_change_height);
    let proposal_hash = config_proposal.object_hash();
    testkit.create_block_with_transaction(sign_config_propose_transaction(
        &testkit,
        config_proposal,
        initiator_id,
    ));

    let signed_txs = build_confirmation_transactions(&testkit, proposal_hash, initiator_id);
    testkit.create_block_with_transactions(signed_txs);
    testkit.create_blocks_until(cfg_change_height);

    assert_eq!(config_propose_entry(&testkit), None);
    assert_eq!(testkit.consensus_config(), new_consensus_config);
}

#[derive(Debug, ServiceFactory)]
#[exonum(
    artifact_name = "inc",
    artifact_version = "1.0.0",
    proto_sources = "proto",
    implements("Configure<Params = String>")
)]
pub struct ConfigChangeService;

const CONFIG_SERVICE_ID: InstanceId = 119;
const CONFIG_SERVICE_NAME: &str = "config-change";

impl Service for ConfigChangeService {
    fn state_hash(&self, _: InstanceDescriptor, _: &dyn Snapshot) -> Vec<Hash> {
        Vec::new()
    }
}

impl From<ConfigChangeService> for InstanceCollection {
    fn from(instance: ConfigChangeService) -> Self {
        InstanceCollection::new(instance).with_instance(
            CONFIG_SERVICE_ID,
            CONFIG_SERVICE_NAME,
            Vec::default(),
        )
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

        Entry::new(format!("{}.params", context.instance.name), fork).set(params.clone());

        match params.as_ref() {
            "apply_error" => {
                Err(DispatcherError::malformed_arguments("Error!")).map_err(From::from)
            }
            "apply_panic" => panic!("Aaaa!"),
            _ => Ok(()),
        }
    }
}

struct ConfigProposeConfigurator {
    config_propose: ConfigPropose,
}

impl ConfigProposeConfigurator {
    fn new(cfg_change_height: Height) -> Self {
        ConfigProposeConfigurator {
            config_propose: ConfigPropose {
                actual_from: cfg_change_height,
                changes: vec![],
            },
        }
    }

    fn extend_consensus_config_propose(mut self, consensus_config: ConsensusConfig) -> Self {
        self.config_propose
            .changes
            .push(ConfigChange::Consensus(consensus_config));
        self
    }

    fn extend_service_config_propose(mut self, param: String) -> Self {
        self.config_propose
            .changes
            .push(ConfigChange::Service(ServiceConfig {
                instance_id: CONFIG_SERVICE_ID,
                params: param.into_bytes(),
            }));
        self
    }

    fn config_propose(&self) -> ConfigPropose {
        self.config_propose.clone()
    }
}

fn testkit_with_change_service_and_static_instance(validator_count: u16) -> TestKit {
    let service = ConfigChangeService;
    let collection =
        InstanceCollection::new(service).with_instance(CONFIG_SERVICE_ID, CONFIG_SERVICE_NAME, ());
    TestKitBuilder::validator()
        .with_validators(validator_count)
        .with_service(Supervisor)
        .with_service(collection)
        .create()
}

fn check_service_actual_param(testkit: &TestKit, param: Option<String>) {
    let actual_params: Option<String> = Entry::new(
        format!("{}.params", CONFIG_SERVICE_NAME),
        &testkit.snapshot(),
    )
    .get();

    assert_eq!(actual_params, param);
}

#[test]
fn service_config_change() {
    let mut testkit = testkit_with_change_service_and_static_instance(4);
    let initiator_id = testkit.network().us().validator_id().unwrap();

    let cfg_change_height = Height(5);
    let params = "I am a new parameter".to_owned();

    let propose = ConfigProposeConfigurator::new(cfg_change_height)
        .extend_service_config_propose(params.clone())
        .config_propose();
    let proposal_hash = propose.object_hash();

    testkit.create_block_with_transaction(sign_config_propose_transaction(
        &testkit,
        propose,
        initiator_id,
    ));
    let signed_txs = build_confirmation_transactions(&testkit, proposal_hash, initiator_id);
    testkit.create_block_with_transactions(signed_txs);

    testkit.create_blocks_until(cfg_change_height);

    assert_eq!(config_propose_entry(&testkit), None);
    check_service_actual_param(&testkit, Some(params));
}

#[test]
fn discard_errored_service_config_change() {
    let mut testkit = testkit_with_change_service_and_static_instance(4);
    let initiator_id = testkit.network().us().validator_id().unwrap();

    let cfg_change_height = Height(5);
    let params = "I am a discarded parameter".to_owned();
    let new_consensus_config = {
        let mut cfg = testkit.consensus_config();
        // Remove us from validators
        cfg.validator_keys.remove(0);
        cfg
    };

    let propose = ConfigProposeConfigurator::new(cfg_change_height)
        .extend_service_config_propose(params.clone())
        .extend_service_config_propose("error".to_string())
        .extend_consensus_config_propose(new_consensus_config)
        .config_propose();

    let proposal_hash = propose.object_hash();

    testkit.create_block_with_transaction(sign_config_propose_transaction(
        &testkit,
        propose,
        initiator_id,
    ));
    let signed_txs = build_confirmation_transactions(&testkit, proposal_hash, initiator_id);
    testkit.create_block_with_transactions(signed_txs);
    testkit.create_blocks_until(cfg_change_height);

    assert_eq!(config_propose_entry(&testkit), None);

    check_service_actual_param(&testkit, None);
    assert_eq!(testkit.network().us().validator_id(), Some(initiator_id));
}

#[test]
fn discard_panicked_service_config_change() {
    let mut testkit = testkit_with_change_service_and_static_instance(4);
    let initiator_id = testkit.network().us().validator_id().unwrap();

    let cfg_change_height = Height(5);
    let params = "I am a discarded parameter".to_owned();
    let new_consensus_config = {
        let mut cfg = testkit.consensus_config();
        // Remove us from validators
        cfg.validator_keys.remove(0);
        cfg
    };

    let propose = ConfigProposeConfigurator::new(cfg_change_height)
        .extend_service_config_propose(params.clone())
        .extend_service_config_propose("panic".to_string())
        .extend_consensus_config_propose(new_consensus_config)
        .config_propose();

    let proposal_hash = propose.object_hash();

    testkit.create_block_with_transaction(sign_config_propose_transaction(
        &testkit,
        propose,
        initiator_id,
    ));
    let signed_txs = build_confirmation_transactions(&testkit, proposal_hash, initiator_id);
    testkit.create_block_with_transactions(signed_txs);
    testkit.create_blocks_until(cfg_change_height);

    assert_eq!(config_propose_entry(&testkit), None);

    check_service_actual_param(&testkit, None);
    assert_eq!(testkit.network().us().validator_id(), Some(initiator_id));
}

#[test]
fn incorrect_actual_from_field() {
    let mut testkit = testkit_with_change_service_and_static_instance(4);
    let initiator_id = testkit.network().us().validator_id().unwrap();

    let cfg_change_height = Height(5);
    let params = "I am a new parameter".to_owned();

    let propose = ConfigProposeConfigurator::new(cfg_change_height)
        .extend_service_config_propose(params.clone())
        .config_propose();

    testkit.create_blocks_until(cfg_change_height);
    testkit
        .create_block_with_transaction(sign_config_propose_transaction(
            &testkit,
            propose,
            initiator_id,
        ))
        .transactions[0]
        .status()
        .unwrap_err();
}

#[test]
fn another_configuration_change_proposal() {
    let mut testkit = testkit_with_change_service_and_static_instance(4);
    let initiator_id = testkit.network().us().validator_id().unwrap();
    let cfg_change_height = Height(5);
    let params = "I am a new parameter".to_owned();

    let propose = ConfigProposeConfigurator::new(cfg_change_height)
        .extend_service_config_propose(params.clone())
        .config_propose();

    let proposal_hash = propose.object_hash();
    testkit.create_block_with_transaction(sign_config_propose_transaction(
        &testkit,
        propose,
        initiator_id,
    ));

    // Try to commit second config change propose.
    let second_propose = ConfigPropose {
        actual_from: cfg_change_height,
        changes: vec![ConfigChange::Service(ServiceConfig {
            instance_id: CONFIG_SERVICE_ID,
            params: "I am an overridden parameter".to_string().into_bytes(),
        })],
    };
    testkit
        .create_block_with_transaction(sign_config_propose_transaction(
            &testkit,
            second_propose,
            initiator_id,
        ))
        .transactions[0]
        .status()
        .unwrap_err();

    let signed_txs = build_confirmation_transactions(&testkit, proposal_hash, initiator_id);
    testkit.create_block_with_transactions(signed_txs);
    testkit.create_blocks_until(cfg_change_height);

    assert_eq!(config_propose_entry(&testkit), None);
    check_service_actual_param(&testkit, Some(params));
}

#[test]
fn service_config_discard_fake_supervisor() {
    const FAKE_SUPERVISOR_ID: InstanceId = 5;
    let keypair = crypto::gen_keypair();

    let mut testkit = TestKitBuilder::validator()
        .with_validators(4)
        .with_service(InstanceCollection::new(Supervisor).with_instance(
            FAKE_SUPERVISOR_ID,
            "fake-supervisor",
            Vec::default(),
        ))
        .with_service(ConfigChangeService)
        .create();

    let cfg_change_height = Height(5);
    let params = "I am a new parameter".to_owned();

    let propose = ConfigPropose {
        actual_from: cfg_change_height,
        changes: vec![ConfigChange::Service(ServiceConfig {
            instance_id: CONFIG_SERVICE_ID,
            params: params.clone().into_bytes(),
        })],
    };

    testkit
        .create_block_with_transaction(propose.sign(FAKE_SUPERVISOR_ID, keypair.0, &keypair.1))
        .transactions[0]
        .status()
        .unwrap_err();
}

#[test]
fn test_configuration_and_rollbacks() {
    let mut testkit = TestKitBuilder::auditor()
        .with_validators(1)
        .with_service(Supervisor)
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

    let config_proposal = build_consensus_config_propose(new_config.clone(), cfg_change_height);
    let proposal_hash = config_proposal.object_hash();
    testkit.create_block_with_transaction(sign_config_propose_transaction(
        &testkit,
        config_proposal,
        ValidatorId(0),
    ));
    let signed_txs = build_confirmation_transactions(&testkit, proposal_hash, ValidatorId(0));
    testkit.create_block_with_transactions(signed_txs);

    testkit.create_blocks_until(cfg_change_height);
    assert_eq!(config_propose_entry(&testkit), None);
    assert_eq!(testkit.consensus_config(), new_config);

    testkit.checkpoint();
    testkit.create_block();
    testkit.rollback();
    assert_eq!(testkit.consensus_config(), new_config);
    assert_eq!(config_propose_entry(&testkit), None);

    testkit.rollback();

    // As rollback is behind the time a proposal entered the blockchain,
    // the proposal is effectively forgotten.
    testkit.create_blocks_until(Height(10));
    assert_eq!(testkit.consensus_config(), old_config);
    assert_eq!(config_propose_entry(&testkit), None);
}

#[test]
fn service_config_rollback_apply_error() {
    let mut testkit = testkit_with_change_service_and_static_instance(4);

    let cfg_change_height = Height(5);
    let params = "apply_error".to_owned();

    let propose = ConfigProposeConfigurator::new(cfg_change_height)
        .extend_service_config_propose(params.clone())
        .config_propose();
    testkit.create_block_with_transaction(sign_config_propose_transaction(
        &testkit,
        propose,
        ValidatorId(0),
    ));

    testkit.create_blocks_until(cfg_change_height.next());
    assert_eq!(config_propose_entry(&testkit), None);

    check_service_actual_param(&testkit, None);
}

#[test]
fn service_config_rollback_apply_panic() {
    let mut testkit = testkit_with_change_service_and_static_instance(4);
    let initiator_id = testkit.network().us().validator_id().unwrap();

    let cfg_change_height = Height(5);
    let params = "apply_panic".to_owned();

    let propose = ConfigProposeConfigurator::new(cfg_change_height)
        .extend_service_config_propose(params.clone())
        .config_propose();

    testkit.create_block_with_transaction(sign_config_propose_transaction(
        &testkit,
        propose,
        initiator_id,
    ));
    testkit.create_blocks_until(cfg_change_height.next());

    assert_eq!(config_propose_entry(&testkit), None);
    check_service_actual_param(&testkit, None);
}

#[test]
fn service_config_apply_multiple_configs() {
    let mut testkit = testkit_with_change_service_and_static_instance(4);
    let initiator_id = testkit.network().us().validator_id().unwrap();

    let cfg_change_height = Height(5);
    let params = "I am a new parameter".to_owned();

    let propose = ConfigProposeConfigurator::new(cfg_change_height)
        .extend_service_config_propose(params.clone())
        .extend_service_config_propose("apply_panic".to_owned())
        .extend_service_config_propose("apply_error".to_owned())
        .config_propose();
    let proposal_hash = propose.object_hash();

    testkit.create_block_with_transaction(sign_config_propose_transaction(
        &testkit,
        propose,
        initiator_id,
    ));

    let signed_txs = build_confirmation_transactions(&testkit, proposal_hash, initiator_id);
    testkit.create_block_with_transactions(signed_txs);

    testkit.create_blocks_until(cfg_change_height);

    check_service_actual_param(&testkit, Some(params));
}

#[test]
fn several_service_config_changes() {
    let mut testkit = testkit_with_change_service_and_static_instance(4);
    let initiator_id = testkit.network().us().validator_id().unwrap();

    for i in 1..5 {
        let cfg_change_height = Height(5 * i);
        let params = format!("Change {}", i);

        let propose = ConfigProposeConfigurator::new(cfg_change_height)
            .extend_service_config_propose(params.clone())
            .config_propose();
        let proposal_hash = propose.object_hash();

        testkit.create_block_with_transaction(sign_config_propose_transaction(
            &testkit,
            propose,
            initiator_id,
        ))[0]
            .status()
            .unwrap();

        let signed_txs = build_confirmation_transactions(&testkit, proposal_hash, initiator_id);
        testkit.create_block_with_transactions(signed_txs)[0]
            .status()
            .unwrap();

        testkit.create_blocks_until(cfg_change_height);
        assert_eq!(config_propose_entry(&testkit), None);
    }

    check_service_actual_param(&testkit, Some("Change 4".to_string()));
}
