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
    api::Error as ApiError,
    crypto::Hash,
    helpers::ValidatorId,
    runtime::{rust::Transaction, SUPERVISOR_INSTANCE_ID},
};

use crate::utils::*;
use exonum_supervisor::{ConfigPropose, ConfigVote};

fn actual_consensus_config(api: &TestKitApi) -> ConsensusConfig {
    api.public(ApiKind::Service("supervisor"))
        .get("consensus-config")
        .unwrap()
}

fn current_config_proposals(api: &TestKitApi) -> Vec<(Hash, ConfigPropose)> {
    api.public(ApiKind::Service("supervisor"))
        .get("config-proposals")
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

    assert!(current_config_proposals(&testkit.api()).is_empty());
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
    let pending_configs = current_config_proposals(&testkit.api());
    assert_eq!(
        pending_configs.len(),
        1,
        "Config proposal was not registered."
    );
    let (pending_config_hash, pending_config_propose) = pending_configs.get(0).unwrap();
    let propose_hash = config_proposal.object_hash();
    assert_eq!(&propose_hash, pending_config_hash);
    assert_eq!(&config_proposal, pending_config_propose);

    // Confirm proposal
    let tx_hash = confirm_config(&testkit.api(), ConfigVote { propose_hash });
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
    let pending_configs = current_config_proposals(&testkit.api());
    assert_eq!(
        pending_configs.len(),
        1,
        "Config proposal was not registered."
    );
    let (pending_config_hash, pending_config_propose) = pending_configs.get(0).unwrap();
    let propose_hash = config_proposal.object_hash();
    assert_eq!(&propose_hash, pending_config_hash);
    assert_eq!(&config_proposal, pending_config_propose);

    // Sign confirmation transaction by second validator
    let keys = testkit.network().validators()[1].service_keypair();
    let signed_confirm = ConfigVote { propose_hash }.sign(SUPERVISOR_INSTANCE_ID, keys.0, &keys.1);
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

#[test]
fn test_send_two_proposals_with_api() {
    let mut testkit = testkit_with_supervisor(2);

    let first_proposal = consensus_config_propose_first_variant(&testkit);
    let first_config_proposal = ConfigProposeBuilder::new(CFG_CHANGE_HEIGHT)
        .extend_consensus_config_propose(first_proposal)
        .config_propose();

    // Create first proposal through testkit tx mechanism
    testkit
        .create_block_with_transaction(sign_config_propose_transaction(
            &testkit,
            first_config_proposal.clone(),
            ValidatorId(1),
        ))
        .transactions[0]
        .status()
        .expect("Transaction with change propose discarded.");

    let second_proposal = consensus_config_propose_second_variant(&testkit);
    let second_config_proposal = ConfigProposeBuilder::new(CFG_CHANGE_HEIGHT)
        .extend_consensus_config_propose(second_proposal)
        .config_propose();
    // Try to create second proposal with api
    let error = testkit
        .api()
        .private(ApiKind::Service("supervisor"))
        .query(&second_config_proposal)
        .post::<Hash>("propose-config")
        .unwrap_err();

    assert_matches!(
        error,
        ApiError::InternalError(ref body) if body.to_string() ==
                "Config proposal with the same height has already been registered"
    );
}

#[test]
fn test_pending_proposals_api() {
    let mut testkit = testkit_with_supervisor(2);

    let first_proposal = consensus_config_propose_first_variant(&testkit);
    let first_config_proposal = ConfigProposeBuilder::new(CFG_CHANGE_HEIGHT)
        .extend_consensus_config_propose(first_proposal)
        .config_propose();
    testkit
        .create_block_with_transaction(sign_config_propose_transaction(
            &testkit,
            first_config_proposal.clone(),
            ValidatorId(0),
        ))
        .transactions[0]
        .status()
        .expect("Transaction with change propose discarded.");

    let second_proposal = consensus_config_propose_second_variant(&testkit);
    let second_config_proposal = ConfigProposeBuilder::new(CFG_CHANGE_HEIGHT.next())
        .extend_consensus_config_propose(second_proposal)
        .config_propose();
    testkit
        .create_block_with_transaction(sign_config_propose_transaction(
            &testkit,
            second_config_proposal.clone(),
            ValidatorId(1),
        ))
        .transactions[0]
        .status()
        .expect("Transaction with change propose discarded.");

    // Get proposals info
    let pending_configs = current_config_proposals(&testkit.api());
    assert_eq!(
        pending_configs.len(),
        2,
        "Config proposal was not registered."
    );

    let first_proposal_hash = first_config_proposal.object_hash();
    let second_proposal_hash = second_config_proposal.object_hash();
    assert!(pending_configs.contains(&(first_proposal_hash, first_config_proposal)));
    assert!(pending_configs.contains(&(second_proposal_hash, second_config_proposal)));
}
