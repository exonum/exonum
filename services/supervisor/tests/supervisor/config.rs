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
use exonum_testkit::{TestKit, TestKitBuilder};

use exonum::{
    crypto::Hash,
    helpers::{Height, ValidatorId},
    runtime::{
        rust::{
            Transaction,
        },
        SUPERVISOR_INSTANCE_ID, ConfigChange,
    },
    blockchain::ConsensusConfig,
    messages::{AnyTx, Verified},
};

use exonum_supervisor::{Supervisor, Schema, ConfigPropose, ConfigVote};

trait TxSign{
    fn sign_config_propose(&self, config: ConfigPropose) -> Verified<AnyTx>;
    fn nodes_vote_for_proposed_config(&mut self, proposal_hash: Hash, initiator_id: ValidatorId);
}

impl TxSign for TestKit{
    fn sign_config_propose(&self, config: ConfigPropose) -> Verified<AnyTx> {
        let keys = &self.validator(ValidatorId(0)).service_keypair();
        config.sign(SUPERVISOR_INSTANCE_ID, keys.0, &keys.1)
    }

    fn nodes_vote_for_proposed_config(&mut self, proposal_hash: Hash, initiator_id: ValidatorId) {
        println!("Validators in network: {}", self.network().validators().len());
        for node in self.network().validators() {
            if node.validator_id() != Some(initiator_id) {
                println!("Validator node key: {}", node.service_keypair().0);
                let keys = node.service_keypair();
                println!("Create block for {}", keys.0);
                self.create_block_with_transaction(
                    ConfigVote{propose_hash: proposal_hash}
                        .sign(SUPERVISOR_INSTANCE_ID, keys.0, &keys.1),
                );
            }
        }
    }
}

fn assert_config_change_entry_is_empty(testkit: &TestKit) {
    let snapshot = testkit.snapshot();
    assert!(!Schema::new("supervisor", &snapshot).config_propose_entry().exists());
}


fn prepare_config_proposal(new_consensus_config: &ConsensusConfig, change_height: Height) -> (ConfigPropose, Hash) {
    let config_proposal = ConfigPropose{
        actual_from: change_height,
        changes: vec!(ConfigChange::Consensus(new_consensus_config.clone())),
    };
    let proposal_hash = config_proposal.object_hash();

    (config_proposal, proposal_hash)
}

#[test]
fn test_add_nodes_to_validators() {
    let mut testkit = TestKitBuilder::auditor()
        .with_validators(1)
        .with_service(Supervisor)
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

    let (new_config_propose, _) = prepare_config_proposal(&new_consensus_config, cfg_change_height);

    testkit.create_block_with_transaction(
        testkit.sign_config_propose(new_config_propose)
    );

    testkit.create_blocks_until(cfg_change_height);

    assert_config_change_entry_is_empty(&testkit);
    assert_eq!(testkit.network().us().validator_id(), Some(ValidatorId(1)));
    assert_eq!(&testkit.network().validators()[1], testkit.network().us());
    assert_eq!(testkit.consensus_config(), new_consensus_config);
}

#[test]
fn exclude_us_from_validators() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(4)
        .with_service(Supervisor)
        .create();

    let cfg_change_height = Height(5);
    let new_consensus_config = {
        let mut cfg = testkit.consensus_config();
        // Remove us from validators
        cfg.validator_keys.remove(0);
        cfg
    };

    let old_validators = testkit.network().validators();

    let (config_proposal, proposal_hash) = prepare_config_proposal(&new_consensus_config, cfg_change_height);

    testkit.create_block_with_transaction(
        testkit.sign_config_propose(config_proposal)
    );

    testkit.nodes_vote_for_proposed_config(proposal_hash, ValidatorId(0));

    testkit.create_blocks_until(cfg_change_height.next());

    let new_validators = testkit.network().validators();

    assert_config_change_entry_is_empty(&testkit);
    assert_eq!(testkit.consensus_config(), new_consensus_config);
    assert_eq!(testkit.network().us().validator_id(), None);
    assert_ne!(old_validators, new_validators);
}

#[test]
fn exclude_other_from_validators() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(2)
        .with_service(Supervisor)
        .create();

    let cfg_change_height = Height(5);
    let new_consensus_config = {
        let mut cfg = testkit.consensus_config();
        // Remove one of node from validators
        cfg.validator_keys.remove(1);
        cfg
    };

    let (config_proposal, proposal_hash) = prepare_config_proposal(&new_consensus_config, cfg_change_height);

    testkit.create_block_with_transaction(
        testkit.sign_config_propose(config_proposal)
    );

    testkit.nodes_vote_for_proposed_config(proposal_hash, ValidatorId(0));

    testkit.create_blocks_until(cfg_change_height.next());

    assert_config_change_entry_is_empty(&testkit);
    assert_eq!(testkit.consensus_config(), new_consensus_config);
    assert_eq!(testkit.network().us().validator_id(), Some(ValidatorId(0)));
}

#[test]
fn change_us_validator_id() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(2)
        .with_service(Supervisor)
        .create();

    let cfg_change_height = Height(5);
    let new_consensus_config = {
        let mut cfg = testkit.consensus_config();
        // Remove one of node from validators
        cfg.validator_keys.swap(0, 1);
        cfg
    };

    let (config_proposal, proposal_hash) = prepare_config_proposal(&new_consensus_config, cfg_change_height);

    testkit.create_block_with_transaction(
        testkit.sign_config_propose(config_proposal)
    );

    testkit.nodes_vote_for_proposed_config(proposal_hash, ValidatorId(0));

    testkit.create_blocks_until(cfg_change_height.next());

    assert_config_change_entry_is_empty(&testkit);
    assert_eq!(testkit.network().us().validator_id(), Some(ValidatorId(1)));
    assert_eq!(&testkit.network().validators()[1], testkit.network().us());
    assert_eq!(testkit.consensus_config(), new_consensus_config);
}

#[test]
#[should_panic]
fn deadline_config_exceeded() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(4)
        .with_service(Supervisor)
        .create();

    let cfg_change_height = Height(5);
    let new_consensus_config = {
        let mut cfg = testkit.consensus_config();
        // Remove us from validators
        cfg.validator_keys.remove(0);
        cfg
    };

    let (config_proposal, _) = prepare_config_proposal(&new_consensus_config, cfg_change_height);

    testkit.create_block_with_transaction(
        testkit.sign_config_propose(config_proposal)
    );
    testkit.create_blocks_until(cfg_change_height.next());

    assert_config_change_entry_is_empty(&testkit);
}

#[test]
fn repeatedly_sent_expired_config() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(4)
        .with_service(Supervisor)
        .create();

    let cfg_change_height = Height(5);
    let new_consensus_config = {
        let mut cfg = testkit.consensus_config();
        // Remove us from validators
        cfg.validator_keys.remove(0);
        cfg
    };

    let (config_proposal, _) = prepare_config_proposal(&new_consensus_config, cfg_change_height);

    testkit.create_block_with_transaction(
        testkit.sign_config_propose(config_proposal)
    );
    testkit.create_blocks_until(cfg_change_height.next());
    testkit.create_block();
    assert_config_change_entry_is_empty(&testkit);

    //Try to send config one more time and vote for it
    let cfg_change_height = Height(15);
    let (config_proposal, proposal_hash) = prepare_config_proposal(&new_consensus_config, cfg_change_height);
    testkit.create_block_with_transaction(
        testkit.sign_config_propose(config_proposal)
    );

    testkit.nodes_vote_for_proposed_config(proposal_hash, ValidatorId(0));
    testkit.create_blocks_until(cfg_change_height.next());

    assert_config_change_entry_is_empty(&testkit);
    assert_eq!(testkit.network().us().validator_id(), None);
    assert_eq!(testkit.consensus_config(), new_consensus_config);
}
