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

use exonum_merkledb::ObjectHash;
use exonum_testkit::{ApiKind, TestKitApi};

use exonum::blockchain::ConsensusConfig;
use exonum::{
    crypto::Hash,
    helpers::ValidatorId,
    runtime::{rust::Transaction, SUPERVISOR_INSTANCE_ID},
};

use crate::utils::*;
use exonum_supervisor::{ConfigProposalWithHash, ConfigPropose, ConfigVote};

fn actual_consensus_config(api: &TestKitApi) -> ConsensusConfig {
    api.public(ApiKind::Service("supervisor"))
        .get("consensus-config")
        .unwrap()
}

fn current_config_proposal(api: &TestKitApi) -> Vec<ConfigProposalWithHash> {
    api.public(ApiKind::Service("supervisor"))
        .get("config-proposal")
        .unwrap()
}

fn create_proposal(api: &TestKitApi, proposal: ConfigPropose) -> Hash {
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

#[test]
fn test_consensus_config_api() {
    let testkit = testkit_with_supervisor(1);

    let consensus_config = actual_consensus_config(&testkit.api());
    assert_eq!(testkit.consensus_config(), consensus_config);
}

#[test]
fn test_config_proposal_api() {
    let testkit = testkit_with_supervisor(1);

    assert!(current_config_proposal(&testkit.api()).is_empty());
}

#[test]
fn test_confirm_proposal_with_api() {
    let mut testkit = testkit_with_supervisor(2);

    let consensus_proposal = consensus_config_propose_first_variant(&testkit);

    let config_proposal = ConfigProposeBuilder::new(CFG_CHANGE_HEIGHT)
        .extend_consensus_config_propose(consensus_proposal.clone())
        .config_propose();

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
    let pending_configs =
        current_config_proposal(&testkit.api());
    assert_eq!(pending_configs.len(), 1,  "Config proposal was not registered.");
    let pending_config = pending_configs.get(0).unwrap();
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
    testkit.create_block();
    testkit.api().exonum_api().assert_tx_success(tx_hash);

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
        .config_propose();

    // Create proposal
    let hash = create_proposal(&testkit.api(), config_proposal.clone());
    testkit.create_block();
    testkit.api().exonum_api().assert_tx_success(hash);

    // Get proposal info
    let pending_configs =
        current_config_proposal(&testkit.api());
    assert_eq!(pending_configs.len(), 1, "Config proposal was not registered.");
    let pending_config = pending_configs.get(0).unwrap();
    let proposal_hash = config_proposal.object_hash();
    assert_eq!(proposal_hash, pending_config.propose_hash);
    assert_eq!(config_proposal, pending_config.config_propose);

    // Sign confirmation transaction by second validator
    let keys = testkit.network().validators()[1].service_keypair();
    let signed_confirm = ConfigVote {
        propose_hash: pending_config.propose_hash,
    }
    .sign(SUPERVISOR_INSTANCE_ID, keys.0, &keys.1);
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

// 2 proposals to same height simultaneously
