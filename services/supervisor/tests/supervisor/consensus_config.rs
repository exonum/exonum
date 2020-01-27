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

use exonum_merkledb::ObjectHash;
use exonum_testkit::TestKitBuilder;

use exonum::helpers::ValidatorId;
use exonum_rust_runtime::ServiceFactory;

use crate::utils::*;
use exonum_supervisor::Supervisor;

#[test]
fn test_add_nodes_to_validators() {
    let mut testkit = TestKitBuilder::auditor()
        .with_validators(1)
        .with_rust_service(Supervisor)
        .with_artifact(Supervisor.artifact_id())
        .with_instance(Supervisor::decentralized())
        .build();

    let new_node_keys = testkit.network_mut().add_node().public_keys();
    let new_consensus_config = {
        let mut cfg = testkit.consensus_config();
        // Add our node.
        cfg.validator_keys.push(testkit.us().public_keys());
        // Add new node.
        cfg.validator_keys.push(new_node_keys);
        cfg
    };

    let new_config_propose = ConfigProposeBuilder::new(CFG_CHANGE_HEIGHT)
        .extend_consensus_config_propose(new_consensus_config.clone())
        .build();
    let signed_proposal =
        sign_config_propose_transaction(&testkit, new_config_propose, ValidatorId(0));
    testkit.create_block_with_transaction(signed_proposal);

    testkit.create_blocks_until(CFG_CHANGE_HEIGHT);

    assert_eq!(config_propose_entry(&testkit), None);
    assert_eq!(testkit.network().us().validator_id(), Some(ValidatorId(1)));
    assert_eq!(testkit.consensus_config(), new_consensus_config);
}

#[test]
fn test_exclude_us_from_validators() {
    let mut testkit = testkit_with_supervisor(4);
    let initiator_id = testkit.network().us().validator_id().unwrap();

    let new_consensus_config = {
        let mut cfg = testkit.consensus_config();
        // Remove us from validators
        cfg.validator_keys.remove(0);
        cfg
    };

    let old_validators = testkit.network().validators();

    let config_proposal = ConfigProposeBuilder::new(CFG_CHANGE_HEIGHT)
        .extend_consensus_config_propose(new_consensus_config.clone())
        .build();
    let proposal_hash = config_proposal.object_hash();
    testkit.create_block_with_transaction(sign_config_propose_transaction(
        &testkit,
        config_proposal,
        initiator_id,
    ));

    let signed_txs = build_confirmation_transactions(&testkit, proposal_hash, initiator_id);
    testkit.create_block_with_transactions(signed_txs);

    testkit.create_blocks_until(CFG_CHANGE_HEIGHT);

    let new_validators = testkit.network().validators();

    assert_eq!(config_propose_entry(&testkit), None);
    assert_eq!(testkit.consensus_config(), new_consensus_config);
    assert_eq!(testkit.network().us().validator_id(), None);
    assert_ne!(old_validators, new_validators);
}

#[test]
fn test_exclude_other_from_validators() {
    let mut testkit = testkit_with_supervisor(4);
    let initiator_id = testkit.network().us().validator_id().unwrap();

    let new_consensus_config = {
        let mut cfg = testkit.consensus_config();
        // Remove one of node from validators
        cfg.validator_keys.remove(1);
        cfg
    };

    let config_proposal = ConfigProposeBuilder::new(CFG_CHANGE_HEIGHT)
        .extend_consensus_config_propose(new_consensus_config.clone())
        .build();
    let proposal_hash = config_proposal.object_hash();

    testkit.create_block_with_transaction(sign_config_propose_transaction(
        &testkit,
        config_proposal,
        initiator_id,
    ));

    let signed_txs = build_confirmation_transactions(&testkit, proposal_hash, initiator_id);
    testkit.create_block_with_transactions(signed_txs);

    testkit.create_blocks_until(CFG_CHANGE_HEIGHT);

    assert_eq!(config_propose_entry(&testkit), None);
    assert_eq!(testkit.consensus_config(), new_consensus_config);
    assert_eq!(testkit.network().us().validator_id(), Some(initiator_id));
}

#[test]
fn test_change_our_validator_id() {
    let mut testkit = testkit_with_supervisor(4);
    let initiator_id = testkit.network().us().validator_id().unwrap();

    let new_consensus_config = {
        let mut cfg = testkit.consensus_config();
        // Change our validator id with another
        cfg.validator_keys.swap(0, 1);
        cfg
    };

    let config_proposal = ConfigProposeBuilder::new(CFG_CHANGE_HEIGHT)
        .extend_consensus_config_propose(new_consensus_config.clone())
        .build();
    let proposal_hash = config_proposal.object_hash();

    testkit.create_block_with_transaction(sign_config_propose_transaction(
        &testkit,
        config_proposal,
        initiator_id,
    ));

    let signed_txs = build_confirmation_transactions(&testkit, proposal_hash, initiator_id);
    testkit.create_block_with_transactions(signed_txs);

    testkit.create_blocks_until(CFG_CHANGE_HEIGHT);

    assert_eq!(config_propose_entry(&testkit), None);
    assert_eq!(testkit.network().us().validator_id(), Some(ValidatorId(1)));
    assert_eq!(&testkit.network().validators()[1], testkit.network().us());
    assert_eq!(testkit.consensus_config(), new_consensus_config);
}
