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

use serde_derive::{Deserialize};

use exonum_merkledb::ObjectHash;
use exonum_testkit::{ApiKind, TestKitApi};

use exonum::blockchain::ConsensusConfig;
use exonum::{
    crypto::Hash,
    helpers::ValidatorId,
    runtime::api::Error,
};

use crate::utils::*;
use exonum_supervisor::{ConfigPropose, ConfigVote};

/// Pending config change proposal entry
#[derive(Debug, Deserialize)]
pub struct ConfigProposalWithHash {
    pub config_propose: ConfigPropose,
    pub propose_hash: Hash,
}

fn actual_consensus_config(api: &TestKitApi) -> ConsensusConfig {
    api.public(ApiKind::Service("supervisor"))
        .get("consensus-config")
        .unwrap()
}

fn current_config_proposal(api: &TestKitApi) -> Result<ConfigProposalWithHash, Error> {
    api.public(ApiKind::Service("supervisor"))
        .get("config-proposal")
}

fn confirm_config(api: &TestKitApi, request: ConfigVote) -> Hash {
    let hash: Hash = api
        .private(ApiKind::Service("supervisor"))
        .query(&request)
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
#[should_panic(expected = "NotFound")]
fn test_config_proposal_api() {
    let testkit = testkit_with_supervisor(1);

    current_config_proposal(&testkit.api()).unwrap();
}

#[test]
fn test_change_config_base() {
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
    testkit.create_block();
    testkit.api().exonum_api().assert_tx_success(tx_hash);

    testkit.create_blocks_until(CFG_CHANGE_HEIGHT.next());

    let consensus_config = actual_consensus_config(&testkit.api());
    assert_eq!(consensus_proposal, consensus_config);
}
