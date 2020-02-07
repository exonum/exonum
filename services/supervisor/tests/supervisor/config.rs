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

use exonum::{
    blockchain::CallInBlock,
    crypto::{self, KeyPair},
    helpers::{Height, ValidatorId},
    merkledb::ObjectHash,
    runtime::{CommonError, ErrorMatch, InstanceId, SnapshotExt, SUPERVISOR_INSTANCE_ID},
};
use exonum_rust_runtime::ServiceFactory;
use exonum_testkit::TestKitBuilder;

use crate::{utils::*, IncService as ConfigChangeService};
use exonum_supervisor::{
    CommonError as SupervisorCommonError, ConfigVote, ConfigurationError, Supervisor,
    SupervisorInterface,
};

#[test]
fn test_multiple_consensus_change_proposes() {
    let mut testkit = testkit_with_supervisor(1);

    let config_proposal = ConfigProposeBuilder::new(CFG_CHANGE_HEIGHT)
        .extend_consensus_config_propose(consensus_config_propose_first_variant(&testkit))
        .extend_consensus_config_propose(consensus_config_propose_second_variant(&testkit))
        .build();

    let signed_proposal =
        sign_config_propose_transaction(&testkit, config_proposal, ValidatorId(0));
    let block = testkit.create_block_with_transaction(signed_proposal);
    let err = block.transactions[0].status().unwrap_err();
    assert_eq!(
        *err,
        ErrorMatch::from_fail(&ConfigurationError::MalformedConfigPropose)
            .for_service(SUPERVISOR_INSTANCE_ID)
            .with_any_description()
    );
    assert_eq!(config_propose_entry(&testkit), None);
}

#[test]
fn test_deadline_config_exceeded() {
    let mut testkit = testkit_with_supervisor(4);
    let initiator_id = testkit.network().us().validator_id().unwrap();
    let consensus_config = testkit.consensus_config();
    let new_consensus_config = consensus_config_propose_first_variant(&testkit);

    let config_proposal = ConfigProposeBuilder::new(CFG_CHANGE_HEIGHT)
        .extend_consensus_config_propose(new_consensus_config.clone())
        .build();
    testkit
        .create_block_with_transaction(sign_config_propose_transaction(
            &testkit,
            config_proposal,
            initiator_id,
        ))
        .transactions[0]
        .status()
        .expect("Transaction with change propose discarded.");
    testkit.create_blocks_until(CFG_CHANGE_HEIGHT.next());

    assert_eq!(config_propose_entry(&testkit), None);
    assert_eq!(testkit.consensus_config(), consensus_config);
}

#[test]
fn test_sent_new_config_after_expired_one() {
    let mut testkit = testkit_with_supervisor(4);
    let initiator_id = testkit.network().us().validator_id().unwrap();

    let first_consensus_config = consensus_config_propose_first_variant(&testkit);

    let config_proposal = ConfigProposeBuilder::new(CFG_CHANGE_HEIGHT)
        .configuration_number(0)
        .extend_consensus_config_propose(first_consensus_config.clone())
        .build();

    testkit
        .create_block_with_transaction(sign_config_propose_transaction(
            &testkit,
            config_proposal,
            initiator_id,
        ))
        .transactions[0]
        .status()
        .expect("Transaction with change propose discarded.");
    testkit.create_blocks_until(CFG_CHANGE_HEIGHT.next());
    assert_eq!(config_propose_entry(&testkit), None);

    // Send config one more time and vote for it
    let cfg_change_height = Height(7);
    let second_consensus_config = consensus_config_propose_second_variant(&testkit);

    let config_proposal = ConfigProposeBuilder::new(cfg_change_height)
        .configuration_number(1)
        .extend_consensus_config_propose(second_consensus_config.clone())
        .build();
    let proposal_hash = config_proposal.object_hash();
    testkit
        .create_block_with_transaction(sign_config_propose_transaction(
            &testkit,
            config_proposal,
            initiator_id,
        ))
        .transactions[0]
        .status()
        .expect("Transaction with change propose discarded.");

    let signed_txs = build_confirmation_transactions(&testkit, proposal_hash, initiator_id);
    testkit
        .create_block_with_transactions(signed_txs)
        .transactions[0]
        .status()
        .expect("Transaction with confirmations discarded.");
    testkit.create_blocks_until(cfg_change_height);

    assert_eq!(config_propose_entry(&testkit), None);
    assert_eq!(testkit.consensus_config(), second_consensus_config);
}

#[test]
fn test_discard_config_with_not_enough_confirms() {
    let mut testkit = testkit_with_supervisor(4);
    let initiator_id = testkit.network().us().validator_id().unwrap();

    testkit.create_block();
    let base_consensus_config = testkit.consensus_config();

    let cfg_change_height = Height(3);
    let consensus_config = consensus_config_propose_first_variant(&testkit);
    let config_proposal = ConfigProposeBuilder::new(cfg_change_height)
        .extend_consensus_config_propose(consensus_config.clone())
        .build();
    let proposal_hash = config_proposal.object_hash();

    testkit
        .create_block_with_transaction(sign_config_propose_transaction(
            &testkit,
            config_proposal,
            initiator_id,
        ))
        .transactions[0]
        .status()
        .expect("Transaction with change propose discarded.");

    // Sign confirmation transaction by second validator
    let keypair = testkit.network().validators()[1].service_keypair();
    let signed_confirm = keypair.confirm_config_change(
        SUPERVISOR_INSTANCE_ID,
        ConfigVote {
            propose_hash: proposal_hash,
        },
    );
    testkit
        .create_block_with_transaction(signed_confirm)
        .transactions[0]
        .status()
        .expect("Transaction with confirmations discarded.");

    testkit.create_blocks_until(cfg_change_height.next());
    assert_eq!(config_propose_entry(&testkit), None);
    assert_eq!(testkit.consensus_config(), base_consensus_config);
}

#[test]
fn test_apply_config_by_min_required_majority() {
    let mut testkit = testkit_with_supervisor(4);
    let initiator_id = testkit.network().us().validator_id().unwrap();

    let cfg_change_height = Height(3);
    let consensus_config = consensus_config_propose_first_variant(&testkit);
    let config_proposal = ConfigProposeBuilder::new(cfg_change_height)
        .extend_consensus_config_propose(consensus_config.clone())
        .build();
    let proposal_hash = config_proposal.object_hash();

    testkit
        .create_block_with_transaction(sign_config_propose_transaction(
            &testkit,
            config_proposal,
            initiator_id,
        ))
        .transactions[0]
        .status()
        .expect("Transaction with change propose discarded.");

    let confirm = ConfigVote {
        propose_hash: proposal_hash,
    };
    // Sign and send confirmation transaction by second validator
    let keys = testkit.network().validators()[1].service_keypair();
    let tx = keys.confirm_config_change(SUPERVISOR_INSTANCE_ID, confirm.clone());
    testkit.create_block_with_transaction(tx).transactions[0]
        .status()
        .expect("Transaction with confirmations discarded.");

    // Sign confirmation transaction by third validator
    let keys = testkit.network().validators()[2].service_keypair();
    let tx = keys.confirm_config_change(SUPERVISOR_INSTANCE_ID, confirm);
    testkit.create_block_with_transaction(tx).transactions[0]
        .status()
        .expect("Transaction with confirmation discarded.");

    assert_eq!(config_propose_entry(&testkit), None);
    assert_eq!(testkit.consensus_config(), consensus_config);
}

#[test]
fn test_send_confirmation_by_initiator() {
    let mut testkit = testkit_with_supervisor(4);
    let initiator_id = testkit.network().us().validator_id().unwrap();

    let consensus_config = consensus_config_propose_first_variant(&testkit);
    let config_proposal = ConfigProposeBuilder::new(CFG_CHANGE_HEIGHT)
        .extend_consensus_config_propose(consensus_config.clone())
        .build();
    let proposal_hash = config_proposal.object_hash();

    testkit
        .create_block_with_transaction(sign_config_propose_transaction(
            &testkit,
            config_proposal,
            initiator_id,
        ))
        .transactions[0]
        .status()
        .expect("Transaction with change propose discarded.");

    // Try to send confirmation transaction by the initiator
    let keys = testkit.network().us().service_keypair();
    let signed_confirm = keys.confirm_config_change(
        SUPERVISOR_INSTANCE_ID,
        ConfigVote {
            propose_hash: proposal_hash,
        },
    );

    let block = testkit.create_block_with_transaction(signed_confirm);
    let err = block.transactions[0].status().unwrap_err();
    assert_eq!(
        *err,
        ErrorMatch::from_fail(&ConfigurationError::AttemptToVoteTwice)
            .for_service(SUPERVISOR_INSTANCE_ID)
    );
}

#[test]
fn test_propose_config_change_by_incorrect_validator() {
    let mut testkit = testkit_with_supervisor(1);

    let consensus_config = consensus_config_propose_first_variant(&testkit);
    let change = ConfigProposeBuilder::new(CFG_CHANGE_HEIGHT)
        .extend_consensus_config_propose(consensus_config.clone())
        .build();
    let keys = KeyPair::random();
    let signed_confirm = keys.propose_config_change(SUPERVISOR_INSTANCE_ID, change);

    let block = testkit.create_block_with_transaction(signed_confirm);
    let err = block.transactions[0].status().unwrap_err();
    assert_eq!(
        *err,
        ErrorMatch::from_fail(&CommonError::UnauthorizedCaller).for_service(SUPERVISOR_INSTANCE_ID)
    );
}

#[test]
fn test_confirm_config_by_incorrect_validator() {
    let mut testkit = testkit_with_supervisor(1);
    let initiator_id = testkit.network().us().validator_id().unwrap();

    let consensus_config = consensus_config_propose_first_variant(&testkit);
    let config_proposal = ConfigProposeBuilder::new(CFG_CHANGE_HEIGHT)
        .extend_consensus_config_propose(consensus_config.clone())
        .build();
    let proposal_hash = config_proposal.object_hash();

    testkit
        .create_block_with_transaction(sign_config_propose_transaction(
            &testkit,
            config_proposal,
            initiator_id,
        ))
        .transactions[0]
        .status()
        .expect("Transaction with change propose discarded.");

    let keys = KeyPair::random();
    let signed_confirm = keys.confirm_config_change(
        SUPERVISOR_INSTANCE_ID,
        ConfigVote {
            propose_hash: proposal_hash,
        },
    );

    let block = testkit.create_block_with_transaction(signed_confirm);
    let err = block.transactions[0].status().unwrap_err();
    assert_eq!(
        *err,
        ErrorMatch::from_fail(&CommonError::UnauthorizedCaller).for_service(SUPERVISOR_INSTANCE_ID)
    );
}

#[test]
fn test_try_confirm_non_existent_proposal() {
    let mut testkit = testkit_with_supervisor(4);
    let initiator_id = testkit.network().us().validator_id().unwrap();

    let consensus_config = consensus_config_propose_first_variant(&testkit);
    let config_proposal = ConfigProposeBuilder::new(CFG_CHANGE_HEIGHT)
        .extend_consensus_config_propose(consensus_config.clone())
        .build();

    testkit
        .create_block_with_transaction(sign_config_propose_transaction(
            &testkit,
            config_proposal,
            initiator_id,
        ))
        .transactions[0]
        .status()
        .expect("Transaction with change propose discarded.");

    let wrong_hash = crypto::hash(&[0]);
    let signed_confirm = build_confirmation_transactions(&testkit, wrong_hash, initiator_id);

    let block = testkit.create_block_with_transactions(signed_confirm);
    let err = block.transactions[0].status().unwrap_err();
    assert_eq!(
        *err,
        ErrorMatch::from_fail(&ConfigurationError::ConfigProposeNotRegistered)
            .for_service(SUPERVISOR_INSTANCE_ID)
    );
}

#[test]
fn test_service_config_change() {
    let mut testkit = testkit_with_supervisor_and_service(4);
    let initiator_id = testkit.network().us().validator_id().unwrap();

    let params = "I am a new parameter".to_owned();

    let propose = ConfigProposeBuilder::new(CFG_CHANGE_HEIGHT)
        .extend_service_config_propose(params.clone())
        .build();
    let proposal_hash = propose.object_hash();

    testkit
        .create_block_with_transaction(sign_config_propose_transaction(
            &testkit,
            propose,
            initiator_id,
        ))
        .transactions[0]
        .status()
        .expect("Transaction with change propose discarded.");
    let signed_txs = build_confirmation_transactions(&testkit, proposal_hash, initiator_id);
    testkit
        .create_block_with_transactions(signed_txs)
        .transactions[0]
        .status()
        .expect("Transaction with confirmations discarded.");

    testkit.create_blocks_until(CFG_CHANGE_HEIGHT);

    assert_eq!(config_propose_entry(&testkit), None);
    check_service_actual_param(&testkit, Some(params));
}

#[test]
fn test_discard_errored_service_config_change() {
    let mut testkit = testkit_with_supervisor_and_service(4);
    let new_consensus_config = consensus_config_propose_first_variant(&testkit);
    let propose = ConfigProposeBuilder::new(CFG_CHANGE_HEIGHT)
        .extend_service_config_propose("error".to_string())
        .extend_consensus_config_propose(new_consensus_config)
        .build();

    let signed_proposal = sign_config_propose_transaction(&testkit, propose, ValidatorId(0));
    let block = testkit.create_block_with_transaction(signed_proposal);
    let err = block.transactions[0].status().unwrap_err();
    assert!(err
        .description()
        .contains("IncService: Configure error request"));
    assert_eq!(config_propose_entry(&testkit), None);
}

#[test]
fn test_discard_panicked_service_config_change() {
    let mut testkit = testkit_with_supervisor_and_service(4);
    let params = "I am a discarded parameter".to_owned();
    let new_consensus_config = consensus_config_propose_first_variant(&testkit);

    let propose = ConfigProposeBuilder::new(CFG_CHANGE_HEIGHT)
        .extend_service_config_propose(params.clone())
        .extend_service_config_propose("panic".to_string())
        .extend_consensus_config_propose(new_consensus_config)
        .build();

    let signed_proposal = sign_config_propose_transaction(&testkit, propose, ValidatorId(0));
    let block = testkit.create_block_with_transaction(signed_proposal);
    let err = block.transactions[0].status().unwrap_err();
    assert_eq!(
        *err,
        ErrorMatch::from_fail(&ConfigurationError::MalformedConfigPropose)
            .for_service(SUPERVISOR_INSTANCE_ID)
            .with_any_description()
    );
    assert_eq!(config_propose_entry(&testkit), None);
}

#[test]
fn test_incorrect_actual_from_field() {
    let mut testkit = testkit_with_supervisor_and_service(1);
    let params = "I am a new parameter".to_owned();
    let propose = ConfigProposeBuilder::new(CFG_CHANGE_HEIGHT)
        .extend_service_config_propose(params.clone())
        .build();

    testkit.create_blocks_until(CFG_CHANGE_HEIGHT);
    let signed_proposal = sign_config_propose_transaction(&testkit, propose, ValidatorId(0));
    let block = testkit.create_block_with_transaction(signed_proposal);
    let err = block.transactions[0].status().unwrap_err();
    assert_eq!(
        *err,
        ErrorMatch::from_fail(&SupervisorCommonError::ActualFromIsPast)
            .for_service(SUPERVISOR_INSTANCE_ID)
    );
}

#[test]
fn test_another_configuration_change_proposal() {
    let mut testkit = testkit_with_supervisor_and_service(4);
    let initiator_id = testkit.network().us().validator_id().unwrap();
    let params = "I am a new parameter".to_owned();

    let cfg_change_height = Height(4);
    let propose = ConfigProposeBuilder::new(cfg_change_height)
        .configuration_number(0)
        .extend_service_config_propose(params.clone())
        .build();

    let proposal_hash = propose.object_hash();
    testkit
        .create_block_with_transaction(sign_config_propose_transaction(
            &testkit,
            propose,
            initiator_id,
        ))
        .transactions[0]
        .status()
        .expect("Transaction with change propose discarded.");

    // Try to commit second config change propose.
    let second_propose = ConfigProposeBuilder::new(cfg_change_height)
        .configuration_number(1)
        .extend_service_config_propose("I am an overridden parameter".to_string())
        .build();

    let signed_proposal = sign_config_propose_transaction(&testkit, second_propose, initiator_id);
    let block = testkit.create_block_with_transaction(signed_proposal);
    let err = block.transactions[0].status().unwrap_err();
    assert_eq!(
        *err,
        ErrorMatch::from_fail(&ConfigurationError::ConfigProposeExists)
            .for_service(SUPERVISOR_INSTANCE_ID)
    );

    let signed_txs = build_confirmation_transactions(&testkit, proposal_hash, initiator_id);
    testkit
        .create_block_with_transactions(signed_txs)
        .transactions[0]
        .status()
        .expect("Transaction with confirmations discarded.");
    testkit.create_blocks_until(cfg_change_height);

    assert_eq!(config_propose_entry(&testkit), None);
    check_service_actual_param(&testkit, Some(params));
}

#[test]
fn test_service_config_discard_fake_supervisor() {
    const FAKE_SUPERVISOR_ID: InstanceId = 5;
    let keypair = KeyPair::random();
    let fake_supervisor_artifact = Supervisor.artifact_id();

    let fake_supervisor_instance = fake_supervisor_artifact
        .clone()
        .into_default_instance(FAKE_SUPERVISOR_ID, "fake-supervisor")
        .with_constructor(Supervisor::decentralized_config());

    let mut testkit = TestKitBuilder::validator()
        .with_validators(1)
        .with_rust_service(Supervisor)
        .with_artifact(fake_supervisor_artifact)
        .with_instance(fake_supervisor_instance)
        .with_default_rust_service(ConfigChangeService)
        .build();

    let params = "I am a new parameter".to_owned();
    let propose = ConfigProposeBuilder::new(CFG_CHANGE_HEIGHT)
        .extend_service_config_propose(params)
        .build();

    let tx = keypair.propose_config_change(FAKE_SUPERVISOR_ID, propose);
    let block = testkit.create_block_with_transaction(tx);
    let err = block.transactions[0].status().unwrap_err();
    assert_eq!(
        *err,
        ErrorMatch::from_fail(&CommonError::UnauthorizedCaller).for_service(FAKE_SUPERVISOR_ID)
    );
}

#[test]
fn test_test_configuration_and_rollbacks() {
    let mut testkit = testkit_with_supervisor(4);
    testkit.create_blocks_until(CFG_CHANGE_HEIGHT);

    let cfg_change_height = Height(5);
    let old_config = testkit.consensus_config();
    testkit.checkpoint();

    let new_config = consensus_config_propose_first_variant(&testkit);
    let propose = ConfigProposeBuilder::new(cfg_change_height)
        .extend_consensus_config_propose(new_config.clone())
        .build();

    let proposal_hash = propose.object_hash();
    testkit
        .create_block_with_transaction(sign_config_propose_transaction(
            &testkit,
            propose,
            ValidatorId(0),
        ))
        .transactions[0]
        .status()
        .expect("Transaction with change propose discarded.");
    let signed_txs = build_confirmation_transactions(&testkit, proposal_hash, ValidatorId(0));
    testkit
        .create_block_with_transactions(signed_txs)
        .transactions[0]
        .status()
        .expect("Transaction with confirmations discarded.");

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
    testkit.create_blocks_until(cfg_change_height);
    assert_eq!(testkit.consensus_config(), old_config);
    assert_eq!(config_propose_entry(&testkit), None);
}

#[test]
fn test_service_config_discard_single_apply_error() {
    let mut testkit = testkit_with_supervisor_and_service(1);
    let params = "apply_error".to_owned();

    let propose = ConfigProposeBuilder::new(CFG_CHANGE_HEIGHT)
        .extend_service_config_propose(params)
        .build();
    testkit
        .create_block_with_transaction(sign_config_propose_transaction(
            &testkit,
            propose,
            ValidatorId(0),
        ))
        .transactions[0]
        .status()
        .expect("Transaction with change propose discarded.");

    testkit.create_blocks_until(CFG_CHANGE_HEIGHT);
    let snapshot = testkit.snapshot();
    let err = snapshot
        .for_core()
        .call_errors(testkit.height())
        .get(&CallInBlock::after_transactions(SUPERVISOR_INSTANCE_ID))
        .unwrap();
    assert!(err.description().contains("IncService: Configure error"));

    // Create one more block for supervisor to remove failed config.
    testkit.create_block();

    assert_eq!(config_propose_entry(&testkit), None);
    check_service_actual_param(&testkit, None);
}

#[test]
fn test_service_config_discard_single_apply_panic() {
    let mut testkit = testkit_with_supervisor_and_service(1);
    let initiator_id = testkit.network().us().validator_id().unwrap();
    let params = "apply_panic".to_owned();

    let propose = ConfigProposeBuilder::new(CFG_CHANGE_HEIGHT)
        .extend_service_config_propose(params)
        .build();

    testkit
        .create_block_with_transaction(sign_config_propose_transaction(
            &testkit,
            propose,
            initiator_id,
        ))
        .transactions[0]
        .status()
        .expect("Transaction with change propose discarded.");
    testkit.create_blocks_until(CFG_CHANGE_HEIGHT);

    let snapshot = testkit.snapshot();
    let err = snapshot
        .for_core()
        .call_errors(testkit.height())
        .get(&CallInBlock::after_transactions(SUPERVISOR_INSTANCE_ID))
        .unwrap();
    assert!(err.description().contains("Configure panic"));

    // Create one more block for supervisor to remove failed config.
    testkit.create_block();

    assert_eq!(config_propose_entry(&testkit), None);
    check_service_actual_param(&testkit, None);
}

// This test checks that we can send a new config proposal right after
// the failure of the previous config applying.
#[test]
fn test_send_config_right_after_error() {
    let mut testkit = testkit_with_supervisor_and_service(1);
    let initiator_id = testkit.network().us().validator_id().unwrap();

    let propose = ConfigProposeBuilder::new(CFG_CHANGE_HEIGHT)
        .extend_service_config_propose("apply_panic".into())
        .build();

    testkit
        .create_block_with_transaction(sign_config_propose_transaction(
            &testkit,
            propose,
            initiator_id,
        ))
        .transactions[0]
        .status()
        .expect("Transaction with change propose discarded.");
    testkit.create_blocks_until(CFG_CHANGE_HEIGHT);

    // Send a new config right after the failure.
    let new_height = Height(100); // We don't really care about height, we're checking the tx approval only.
    let propose = ConfigProposeBuilder::new(new_height)
        .configuration_number(1)
        .extend_service_config_propose("good_config".into())
        .build();

    testkit
        .create_block_with_transaction(sign_config_propose_transaction(
            &testkit,
            propose,
            initiator_id,
        ))
        .transactions[0]
        .status()
        .expect("Transaction with change propose discarded.");
}

#[test]
fn test_services_config_apply_multiple_configs() {
    let mut testkit = testkit_with_supervisor_and_2_services(4);
    let initiator_id = testkit.network().us().validator_id().unwrap();
    let params = "I am a new parameter".to_owned();

    let propose = ConfigProposeBuilder::new(CFG_CHANGE_HEIGHT)
        .extend_service_config_propose(params.clone())
        .extend_second_service_config_propose(params.clone())
        .build();
    let proposal_hash = propose.object_hash();

    testkit
        .create_block_with_transaction(sign_config_propose_transaction(
            &testkit,
            propose,
            initiator_id,
        ))
        .transactions[0]
        .status()
        .expect("Transaction with change propose discarded.");

    let signed_txs = build_confirmation_transactions(&testkit, proposal_hash, initiator_id);
    testkit
        .create_block_with_transactions(signed_txs)
        .transactions[0]
        .status()
        .expect("Transaction with confirmations discarded.");
    testkit.create_blocks_until(CFG_CHANGE_HEIGHT);

    check_service_actual_param(&testkit, Some(params.clone()));
    check_second_service_actual_param(&testkit, Some(params));
}

#[test]
fn test_services_config_discard_multiple_configs() {
    let mut testkit = testkit_with_supervisor_and_2_services(1);
    let initiator_id = testkit.network().us().validator_id().unwrap();
    let params = "I am a new parameter".to_owned();

    let propose = ConfigProposeBuilder::new(CFG_CHANGE_HEIGHT)
        .extend_service_config_propose(params.clone())
        .extend_second_service_config_propose(params.clone())
        .extend_second_service_config_propose("I am a extra proposal".to_owned())
        .build();

    let signed_proposal = sign_config_propose_transaction(&testkit, propose, initiator_id);

    let block = testkit.create_block_with_transaction(signed_proposal);
    let err = block.transactions[0].status().unwrap_err();
    assert_eq!(
        *err,
        ErrorMatch::from_fail(&ConfigurationError::MalformedConfigPropose)
            .for_service(SUPERVISOR_INSTANCE_ID)
            .with_any_description()
    );
    assert_eq!(config_propose_entry(&testkit), None);
}

#[test]
fn test_several_service_config_changes() {
    let mut testkit = testkit_with_supervisor_and_service(4);
    let initiator_id = testkit.network().us().validator_id().unwrap();

    for i in 1..5 {
        let cfg_change_height = Height(2 * i);
        let params = format!("Change {}", i);

        let propose = ConfigProposeBuilder::new(cfg_change_height)
            .configuration_number(i - 1)
            .extend_service_config_propose(params.clone())
            .build();
        let proposal_hash = propose.object_hash();

        testkit
            .create_block_with_transaction(sign_config_propose_transaction(
                &testkit,
                propose,
                initiator_id,
            ))
            .transactions[0]
            .status()
            .expect("Transaction with change propose discarded.");

        let signed_txs = build_confirmation_transactions(&testkit, proposal_hash, initiator_id);
        testkit.create_block_with_transactions(signed_txs)[0]
            .status()
            .unwrap();

        testkit.create_blocks_until(cfg_change_height);
        assert_eq!(config_propose_entry(&testkit), None);
    }

    check_service_actual_param(&testkit, Some("Change 4".to_string()));
}

/// Checks that config with incorrect configuration number is discarded.
#[test]
fn test_discard_incorrect_configuration_number() {
    let mut testkit = testkit_with_supervisor(4);

    // Attempt to send config with incorrect configuration number (expected 0, actual 100).
    let incorrect_configuration_number = 100;
    let first_config_height = Height(2);

    let config_proposal = ConfigProposeBuilder::new(first_config_height)
        .configuration_number(incorrect_configuration_number)
        .extend_consensus_config_propose(consensus_config_propose_first_variant(&testkit))
        .build();

    let signed_proposal =
        sign_config_propose_transaction(&testkit, config_proposal, ValidatorId(0));
    let block = testkit.create_block_with_transaction(signed_proposal);
    let err = block.transactions[0].status().unwrap_err();
    assert_eq!(
        *err,
        ErrorMatch::from_fail(&ConfigurationError::IncorrectConfigurationNumber)
            .for_service(SUPERVISOR_INSTANCE_ID)
    );
    assert_eq!(config_propose_entry(&testkit), None);

    // Apply some correct config (expected 0, actual 0).
    let second_config_height = Height(4);
    let initiator_id = testkit.network().us().validator_id().unwrap();

    let first_consensus_config = consensus_config_propose_first_variant(&testkit);

    let config_proposal = ConfigProposeBuilder::new(second_config_height)
        .configuration_number(0)
        .extend_consensus_config_propose(first_consensus_config.clone())
        .build();
    let proposal_hash = config_proposal.object_hash();

    testkit
        .create_block_with_transaction(sign_config_propose_transaction(
            &testkit,
            config_proposal,
            initiator_id,
        ))
        .transactions[0]
        .status()
        .expect("Transaction with change propose discarded.");

    let signed_txs = build_confirmation_transactions(&testkit, proposal_hash, initiator_id);
    testkit
        .create_block_with_transactions(signed_txs)
        .transactions[0]
        .status()
        .expect("Transaction with confirmations discarded.");
    testkit.create_blocks_until(second_config_height);

    assert_eq!(config_propose_entry(&testkit), None);
    assert_eq!(testkit.consensus_config(), first_consensus_config);

    // Attempt to send config with outdated configuration number (expected 1, actual 0).
    let incorrect_configuration_number = 0;
    let third_config_height = Height(6);

    let config_proposal = ConfigProposeBuilder::new(third_config_height)
        .configuration_number(incorrect_configuration_number)
        .extend_consensus_config_propose(consensus_config_propose_first_variant(&testkit))
        .build();

    let signed_proposal =
        sign_config_propose_transaction(&testkit, config_proposal, ValidatorId(0));
    let block = testkit.create_block_with_transaction(signed_proposal);
    let err = block.transactions[0].status().unwrap_err();
    assert_eq!(
        *err,
        ErrorMatch::from_fail(&ConfigurationError::IncorrectConfigurationNumber)
            .for_service(SUPERVISOR_INSTANCE_ID)
    );
    assert_eq!(config_propose_entry(&testkit), None);
}

/// Checks that if config applying error, none of changes from the proposal are applied.
#[test]
fn test_all_changes_are_discarded_on_panic() {
    let mut testkit = testkit_with_supervisor_and_service(4);
    let initiator_id = testkit.network().us().validator_id().unwrap();

    let erroneous_config_params = ["apply_error", "apply_panic"];

    for (i, params) in erroneous_config_params.iter().enumerate() {
        let cfg_change_height = Height(3 * (i + 1) as u64);
        let mut propose =
            ConfigProposeBuilder::new(cfg_change_height).configuration_number(i as u64);

        // Add a valid config entry.
        let old_consensus_config = testkit.consensus_config();
        let consensus_config = consensus_config_propose_first_variant(&testkit);
        propose = propose.extend_consensus_config_propose(consensus_config.clone());

        // Add an erroneous config entry.
        propose = propose.extend_service_config_propose((*params).to_owned());

        // Send config proposal.
        let propose = propose.build();

        let proposal_hash = propose.object_hash();

        testkit
            .create_block_with_transaction(sign_config_propose_transaction(
                &testkit,
                propose,
                initiator_id,
            ))
            .transactions[0]
            .status()
            .expect("Transaction with change propose discarded.");

        let signed_txs = build_confirmation_transactions(&testkit, proposal_hash, initiator_id);
        testkit.create_block_with_transactions(signed_txs)[0]
            .status()
            .unwrap();

        testkit.create_blocks_until(cfg_change_height);
        testkit.create_block();

        // Check that config didn't change.
        assert_eq!(config_propose_entry(&testkit), None);
        assert_eq!(testkit.consensus_config(), old_consensus_config);
    }
}
