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
    blockchain::ConsensusConfig, crypto::Hash, helpers::ValidatorId,
    runtime::SUPERVISOR_INSTANCE_ID,
};
use exonum_merkledb::ObjectHash;
use exonum_testkit::{ApiKind, TestKit, TestKitApi};

use crate::utils::*;
use exonum_supervisor::{ConfigProposalWithHash, ConfigPropose, ConfigVote, SupervisorInterface};

fn actual_consensus_config(api: &TestKitApi) -> ConsensusConfig {
    api.public(ApiKind::Service("supervisor"))
        .get("consensus-config")
        .unwrap()
}

fn current_config_proposal(api: &TestKitApi) -> Option<ConfigProposalWithHash> {
    api.public(ApiKind::Service("supervisor"))
        .get("config-proposal")
        .unwrap()
}

pub fn create_proposal(api: &TestKitApi, proposal: ConfigPropose) -> Hash {
    let hash: Hash = api
        .private(ApiKind::Service("supervisor"))
        .query(&proposal)
        .post("propose-config")
        .unwrap();
    hash
}

fn confirm_config(api: &TestKitApi, confirm: ConfigVote) -> Hash {
    let hash: Hash = api
        .private(ApiKind::Service("supervisor"))
        .query(&confirm)
        .post("confirm-config")
        .unwrap();
    hash
}

fn configuration_number(api: &TestKitApi) -> u64 {
    api.private(ApiKind::Service("supervisor"))
        .get("configuration-number")
        .unwrap()
}

#[test]
fn test_consensus_config_api() {
    let mut testkit = testkit_with_supervisor(1);
    let consensus_config = actual_consensus_config(&testkit.api());
    assert_eq!(testkit.consensus_config(), consensus_config);
}

#[test]
fn test_config_proposal_api() {
    let mut testkit = testkit_with_supervisor(1);
    assert_eq!(current_config_proposal(&testkit.api()), None);
}

#[test]
fn test_confirm_proposal_with_api() {
    let mut testkit = testkit_with_supervisor(2);
    let consensus_proposal = consensus_config_propose_first_variant(&testkit);
    let config_proposal = ConfigProposeBuilder::new(CFG_CHANGE_HEIGHT)
        .extend_consensus_config_propose(consensus_proposal.clone())
        .build();

    // Create proposal
    testkit
        .create_block_with_transaction(sign_config_propose_transaction(
            &testkit,
            config_proposal.clone(),
            ValidatorId(1),
        ))
        .transactions[0]
        .status()
        .expect("Transaction with change propose discarded.");

    // Get proposal info
    let pending_config =
        current_config_proposal(&testkit.api()).expect("Config proposal was not registered.");
    let proposal_hash = config_proposal.object_hash();
    assert_eq!(proposal_hash, pending_config.propose_hash);
    assert_eq!(config_proposal, pending_config.config_propose);

    // Confirm proposal
    let tx_hash = confirm_config(
        &testkit.api(),
        ConfigVote {
            propose_hash: pending_config.propose_hash,
        },
    );
    let block = testkit.create_block();
    block[tx_hash].status().unwrap();
    testkit.create_blocks_until(CFG_CHANGE_HEIGHT.next());

    let consensus_config = actual_consensus_config(&testkit.api());
    assert_eq!(consensus_proposal, consensus_config);
}

#[test]
fn test_send_proposal_with_api() {
    let mut testkit = testkit_with_supervisor(2);
    let consensus_proposal = consensus_config_propose_first_variant(&testkit);
    let config_proposal = ConfigProposeBuilder::new(CFG_CHANGE_HEIGHT)
        .extend_consensus_config_propose(consensus_proposal.clone())
        .build();

    // Create proposal
    let hash = create_proposal(&testkit.api(), config_proposal.clone());
    let block = testkit.create_block();
    block[hash].status().unwrap();

    // Get proposal info
    let pending_config =
        current_config_proposal(&testkit.api()).expect("Config proposal was not registered.");
    let proposal_hash = config_proposal.object_hash();
    assert_eq!(proposal_hash, pending_config.propose_hash);
    assert_eq!(config_proposal, pending_config.config_propose);

    // Sign confirmation transaction by second validator
    let keypair = testkit.network().validators()[1].service_keypair();
    let signed_confirm = keypair.confirm_config_change(
        SUPERVISOR_INSTANCE_ID,
        ConfigVote {
            propose_hash: pending_config.propose_hash,
        },
    );
    // Confirm proposal
    testkit
        .create_block_with_transaction(signed_confirm)
        .transactions[0]
        .status()
        .expect("Transaction with confirmations discarded.");

    testkit.create_blocks_until(CFG_CHANGE_HEIGHT.next());
    let consensus_config = actual_consensus_config(&testkit.api());
    assert_eq!(consensus_proposal, consensus_config);
}

/// Applies some config via API.
/// This function can be used when we need to apply any config and don't care about the process.
fn apply_config(testkit: &mut TestKit) {
    let consensus_proposal = consensus_config_propose_first_variant(testkit);
    let config_proposal = ConfigProposeBuilder::new(CFG_CHANGE_HEIGHT)
        .extend_consensus_config_propose(consensus_proposal.clone())
        .build();

    // Create proposal.
    create_proposal(&testkit.api(), config_proposal.clone());
    testkit.create_block();

    // Get proposal info.
    let pending_config =
        current_config_proposal(&testkit.api()).expect("Config proposal was not registered.");

    // Sign confirmation transaction by second validator.
    let keypair = testkit.network().validators()[1].service_keypair();
    let signed_confirm = keypair.confirm_config_change(
        SUPERVISOR_INSTANCE_ID,
        ConfigVote {
            propose_hash: pending_config.propose_hash,
        },
    );

    // Confirm proposal.
    testkit
        .create_block_with_transaction(signed_confirm)
        .transactions[0]
        .status()
        .expect("Transaction with confirmations discarded.");

    testkit.create_blocks_until(CFG_CHANGE_HEIGHT.next());
}

/// Checks that configuration number obtained via API is correct.
#[test]
fn test_configuration_number() {
    let mut testkit = testkit_with_supervisor(2);

    // Check that at the start configuration number is 0.
    let initial_configuration_number = configuration_number(&testkit.api());
    assert_eq!(initial_configuration_number, 0);

    // Apply some config.
    apply_config(&mut testkit);

    // Check that configuration number is increased.
    let new_configuration_number = configuration_number(&testkit.api());
    assert_eq!(new_configuration_number, 1);
}
