// Copyright 2018 The Exonum Team
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
    blockchain::{Schema, StoredConfiguration, Transaction},
    crypto::{hash, CryptoHash, Hash, HASH_SIZE}, helpers::{Height, ValidatorId},
    storage::StorageValue,
};
use exonum_testkit::{TestKit, TestKitBuilder, TestNode};

use std::str;

use {
    Propose, Schema as ConfigurationSchema, Service as ConfigurationService, Vote, VoteAgainst,
    VotingDecision,
};

mod api;

pub fn to_boxed<T: Transaction>(tx: T) -> Box<Transaction> {
    Box::new(tx) as Box<Transaction>
}

pub fn new_tx_config_propose(node: &TestNode, cfg_proposal: StoredConfiguration) -> Propose {
    let keypair = node.service_keypair();
    Propose::new(
        keypair.0,
        str::from_utf8(cfg_proposal.into_bytes().as_slice()).unwrap(),
        keypair.1,
    )
}

pub fn new_tx_config_vote(node: &TestNode, cfg_proposal_hash: Hash) -> Vote {
    let keypair = node.service_keypair();
    Vote::new(keypair.0, &cfg_proposal_hash, keypair.1)
}

pub fn new_tx_config_vote_against(node: &TestNode, cfg_proposal_hash: Hash) -> VoteAgainst {
    let keypair = node.service_keypair();
    VoteAgainst::new(keypair.0, &cfg_proposal_hash, keypair.1)
}

pub trait ConfigurationTestKit {
    fn configuration_default() -> Self;

    fn apply_configuration(&mut self, proposer: ValidatorId, cfg_proposal: StoredConfiguration);

    fn votes_for_propose(&self, config_hash: Hash) -> Vec<Option<VotingDecision>>;

    fn find_propose(&self, config_hash: Hash) -> Option<Propose>;
}

impl ConfigurationTestKit for TestKit {
    fn configuration_default() -> Self {
        TestKitBuilder::validator()
            .with_validators(4)
            .with_service(ConfigurationService {})
            .create()
    }

    fn apply_configuration(&mut self, proposer: ValidatorId, cfg_proposal: StoredConfiguration) {
        let cfg_change_height = cfg_proposal.actual_from;
        // Push cfg change propose.
        let tx_propose = new_tx_config_propose(
            &self.network().validators()[proposer.0 as usize],
            cfg_proposal.clone(),
        );
        self.create_block_with_transactions(txvec![tx_propose]);
        // Push votes
        let cfg_proposal_hash = cfg_proposal.hash();
        let tx_votes = self.network()
            .validators()
            .iter()
            .map(|validator| new_tx_config_vote(validator, cfg_proposal_hash))
            .map(to_boxed)
            .collect::<Vec<_>>();
        self.create_block_with_transactions(tx_votes);
        // Fast forward to cfg_change_height
        self.create_blocks_until(cfg_change_height);
        // Check that configuration applied.
        assert_eq!(
            Schema::new(&self.snapshot()).actual_configuration(),
            cfg_proposal
        );
    }

    fn votes_for_propose(&self, config_hash: Hash) -> Vec<Option<VotingDecision>> {
        let snapshot = self.snapshot();
        let schema = ConfigurationSchema::new(&snapshot);
        schema.votes(&config_hash)
    }

    fn find_propose(&self, config_hash: Hash) -> Option<Propose> {
        let snapshot = self.snapshot();
        let schema = ConfigurationSchema::new(&snapshot);
        schema.propose(&config_hash)
    }
}

#[test]
fn test_full_node_to_validator() {
    let mut testkit = TestKitBuilder::auditor()
        .with_validators(3)
        .with_service(ConfigurationService {})
        .create();

    let cfg_change_height = Height(5);
    let new_cfg = {
        let mut cfg = testkit.configuration_change_proposal();
        let mut validators = cfg.validators().to_vec();
        validators.push(testkit.network().us().clone());
        cfg.set_actual_from(cfg_change_height);
        cfg.set_validators(validators);
        cfg.stored_configuration().clone()
    };
    testkit.apply_configuration(ValidatorId(1), new_cfg);
}

#[test]
fn test_add_validators_to_config() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(3)
        .with_service(ConfigurationService {})
        .create();

    let cfg_change_height = Height(5);
    let new_cfg = {
        let mut cfg = testkit.configuration_change_proposal();
        let mut validators = cfg.validators().to_vec();
        validators.push(TestNode::new_validator(ValidatorId(3)));
        cfg.set_actual_from(cfg_change_height);
        cfg.set_validators(validators);
        cfg.stored_configuration().clone()
    };
    testkit.apply_configuration(ValidatorId(0), new_cfg);
}

#[test]
fn test_exclude_sandbox_node_from_config() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(4)
        .with_service(ConfigurationService {})
        .create();

    let cfg_change_height = Height(5);
    let new_cfg = {
        let mut cfg = testkit.configuration_change_proposal();
        let mut validators = cfg.validators().to_vec();
        validators.pop();
        cfg.set_actual_from(cfg_change_height);
        cfg.set_validators(validators);
        cfg.stored_configuration().clone()
    };
    testkit.apply_configuration(ValidatorId(0), new_cfg);
}

#[test]
fn test_apply_second_configuration() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(3)
        .with_service(ConfigurationService {})
        .create();
    // First configuration.
    let cfg_change_height = Height(5);
    let new_cfg = {
        let mut cfg = testkit.configuration_change_proposal();
        let mut validators = cfg.validators().to_vec();
        validators.push(TestNode::new_validator(ValidatorId(3)));
        cfg.set_actual_from(cfg_change_height);
        cfg.set_validators(validators);
        cfg.stored_configuration().clone()
    };
    testkit.apply_configuration(ValidatorId(0), new_cfg);
    // Second configuration.
    let cfg_change_height = Height(10);
    let new_cfg = {
        let mut cfg = testkit.configuration_change_proposal();
        let mut validators = cfg.validators().to_vec();
        validators.pop();
        cfg.set_actual_from(cfg_change_height);
        cfg.set_validators(validators);
        cfg.stored_configuration().clone()
    };
    testkit.apply_configuration(ValidatorId(0), new_cfg);
}

#[test]
fn test_apply_with_increased_majority() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(6)
        .with_service(ConfigurationService {})
        .create();

    // Applying the first configuration with custom majority count.
    let cfg_change_height = Height(5);
    let new_cfg = {
        let mut cfg = testkit.configuration_change_proposal();
        cfg.set_service_config("dummy", "First cfg");
        cfg.set_majority_count(Some(6));
        cfg.set_actual_from(cfg_change_height);
        cfg.stored_configuration().clone()
    };
    testkit.apply_configuration(ValidatorId(0), new_cfg);

    // Applying the second configuration.
    // Number of votes equals to the number of validators.
    let cfg_change_height = Height(10);
    let new_cfg = {
        let mut cfg = testkit.configuration_change_proposal();
        cfg.set_service_config("dummy", "Second cfg");
        cfg.set_actual_from(cfg_change_height);
        cfg.stored_configuration().clone()
    };
    testkit.apply_configuration(ValidatorId(0), new_cfg);

    // Trying to apply the third configuration.
    // Number is greater than byzantine_majority_count but less than configured majority count.
    let cfg_change_height = Height(15);
    let new_cfg = {
        let mut cfg = testkit.configuration_change_proposal();
        cfg.set_service_config("dummy", "Second cfg");
        cfg.set_actual_from(cfg_change_height);
        cfg.stored_configuration().clone()
    };

    let validators = testkit.network().validators().to_vec();
    let tx_propose = new_tx_config_propose(&validators[1], new_cfg.clone());
    testkit.create_block_with_transactions(txvec![tx_propose]);

    let cfg_proposal_hash = new_cfg.hash();

    let tx_votes = validators[0..5] // not enough validators
        .iter()
        .map(|validator| new_tx_config_vote(validator, cfg_proposal_hash))
        .map(to_boxed)
        .collect::<Vec<_>>();

    testkit.create_block_with_transactions(tx_votes);
    testkit.create_blocks_until(cfg_change_height);

    assert_ne!(
        Schema::new(&testkit.snapshot()).actual_configuration(),
        new_cfg
    );
}

#[test]
fn test_discard_proposes_with_too_big_majority_count() {
    let mut testkit: TestKit = TestKit::configuration_default();

    let cfg_change_height = Height(5);
    let new_cfg = {
        let mut cfg = testkit.configuration_change_proposal();
        let excessive_majority_count = (&testkit.network().validators().len() + 100) as u16;
        cfg.set_service_config("dummy", "First cfg");
        cfg.set_majority_count(Some(excessive_majority_count));
        cfg.set_actual_from(cfg_change_height);
        cfg.stored_configuration().clone()
    };

    let propose_tx = new_tx_config_propose(&testkit.network().validators()[1], new_cfg.clone());
    testkit.create_block_with_transactions(txvec![propose_tx]);
    assert!(testkit.find_propose(new_cfg.hash()).is_none());
}

#[test]
fn test_discard_proposes_with_too_small_majority_count() {
    let mut testkit: TestKit = TestKit::configuration_default();

    let cfg_change_height = Height(5);
    let new_cfg = {
        let mut cfg = testkit.configuration_change_proposal();
        let insufficient_majority_count = (&testkit.network().validators().len() / 2) as u16;
        cfg.set_service_config("dummy", "First cfg");
        cfg.set_majority_count(Some(insufficient_majority_count));
        cfg.set_actual_from(cfg_change_height);
        cfg.stored_configuration().clone()
    };

    let propose_tx = new_tx_config_propose(&testkit.network().validators()[1], new_cfg.clone());
    testkit.create_block_with_transactions(txvec![propose_tx]);
    assert!(testkit.find_propose(new_cfg.hash()).is_none());
}

#[test]
fn test_discard_propose_for_same_cfg() {
    let mut testkit: TestKit = TestKit::configuration_default();

    let cfg_change_height = Height(5);
    let new_cfg = {
        let mut cfg = testkit.configuration_change_proposal();
        cfg.set_actual_from(cfg_change_height);
        cfg.set_service_config("dummy", "First cfg change");
        cfg.stored_configuration().clone()
    };
    let (propose_tx, duplicated_propose_tx) = {
        let validators = testkit.network().validators();
        let propose_tx = new_tx_config_propose(&validators[1], new_cfg.clone());
        let duplicated_propose_tx = new_tx_config_propose(&validators[0], new_cfg.clone());
        (propose_tx, duplicated_propose_tx)
    };

    testkit.create_block_with_transactions(txvec![propose_tx.clone(), duplicated_propose_tx]);
    assert_eq!(Some(propose_tx), testkit.find_propose(new_cfg.hash()));
}

#[test]
fn test_discard_vote_for_absent_propose() {
    let mut testkit: TestKit = TestKit::configuration_default();

    let cfg_change_height = Height(5);
    let new_cfg = {
        let mut cfg = testkit.configuration_change_proposal();
        cfg.set_service_config("dummy", "First cfg");
        cfg.set_actual_from(cfg_change_height);
        cfg.stored_configuration().clone()
    };
    let absent_cfg = {
        let mut cfg = testkit.configuration_change_proposal();
        cfg.set_service_config("dummy", "Absent propose");
        cfg.set_actual_from(cfg_change_height);
        cfg.stored_configuration().clone()
    };

    let propose_tx = new_tx_config_propose(&testkit.network().validators()[1], new_cfg.clone());
    testkit.create_block_with_transactions(txvec![propose_tx]);

    let legal_vote = new_tx_config_vote(&testkit.network().validators()[3], new_cfg.hash());
    let illegal_vote = new_tx_config_vote(&testkit.network().validators()[3], absent_cfg.hash());
    testkit.create_block_with_transactions(txvec![legal_vote.clone(), illegal_vote.clone()]);

    let votes = testkit.votes_for_propose(new_cfg.hash());
    assert!(votes.contains(&Some(VotingDecision::Yea(legal_vote))));
    assert!(!votes.contains(&Some(VotingDecision::Yea(illegal_vote))));
}

#[test]
fn test_vote_against_for_propose() {
    let mut testkit: TestKit = TestKit::configuration_default();

    let new_cfg = {
        let mut cfg = testkit.configuration_change_proposal();
        cfg.set_service_config("dummy", "First cfg");
        cfg.set_actual_from(Height(5));
        cfg.stored_configuration().clone()
    };

    let propose_tx = new_tx_config_propose(&testkit.network().validators()[1], new_cfg.clone());
    testkit.create_block_with_transactions(txvec![propose_tx]);

    let legal_vote = new_tx_config_vote_against(&testkit.network().validators()[3], new_cfg.hash());
    let illegal_vote = new_tx_config_vote(&testkit.network().validators()[3], new_cfg.hash());
    testkit.create_block_with_transactions(txvec![legal_vote.clone(), illegal_vote.clone()]);

    let votes = testkit.votes_for_propose(new_cfg.hash());
    assert!(votes.contains(&Some(VotingDecision::Nay(legal_vote))));
    assert!(!votes.contains(&Some(VotingDecision::Yea(illegal_vote))));
}

#[test]
fn test_discard_proposes_with_expired_actual_from() {
    let mut testkit: TestKit = TestKit::configuration_default();

    testkit.create_blocks_until(Height(10));
    let cfg_change_height = Height(5);
    let new_cfg = {
        let mut cfg = testkit.configuration_change_proposal();
        cfg.set_service_config("dummy", "First cfg");
        cfg.set_actual_from(cfg_change_height);
        cfg.stored_configuration().clone()
    };

    let propose_tx = new_tx_config_propose(&testkit.network().validators()[1], new_cfg.clone());
    testkit.create_block_with_transactions(txvec![propose_tx]);
    assert_eq!(None, testkit.find_propose(new_cfg.hash()));
}

#[test]
fn test_discard_votes_with_expired_actual_from() {
    let mut testkit: TestKit = TestKit::configuration_default();

    let cfg_change_height = Height(5);
    let new_cfg = {
        let mut cfg = testkit.configuration_change_proposal();
        cfg.set_service_config("dummy", "First cfg");
        cfg.set_actual_from(cfg_change_height);
        cfg.stored_configuration().clone()
    };

    let propose_tx = new_tx_config_propose(&testkit.network().validators()[1], new_cfg.clone());
    testkit.create_block_with_transactions(txvec![propose_tx]);
    let legal_votes = {
        let validators = testkit.network().validators();
        txvec![
            new_tx_config_vote(&validators[1], new_cfg.hash()),
            new_tx_config_vote(&validators[3], new_cfg.hash()),
        ]
    };
    testkit.create_block_with_transactions(legal_votes);
    testkit.create_blocks_until(Height(10));
    let illegal_vote = new_tx_config_vote(&testkit.network().validators()[0], new_cfg.hash());
    testkit.create_block_with_transactions(txvec![illegal_vote.clone()]);
    assert!(!testkit
        .votes_for_propose(new_cfg.hash())
        .contains(&Some(VotingDecision::Yea(illegal_vote))));
}

#[test]
fn test_discard_invalid_config_json() {
    let mut testkit: TestKit = TestKit::configuration_default();

    let cfg_bytes = [70; 74];
    let new_cfg = str::from_utf8(&cfg_bytes).unwrap(); // invalid json bytes

    let propose_tx = {
        let keypair = testkit.network().validators()[1].service_keypair();
        Propose::new(&keypair.0, new_cfg, &keypair.1)
    };
    testkit.create_block_with_transactions(txvec![propose_tx]);
    assert_eq!(None, testkit.find_propose(hash(new_cfg.as_bytes())));
}

#[test]
fn test_config_txs_discarded_when_following_config_present() {
    let mut testkit: TestKit = TestKit::configuration_default();

    let first_cfg = {
        let mut cfg = testkit.configuration_change_proposal();
        cfg.set_actual_from(Height(5));
        cfg.set_service_config("dummy", "First cfg change");
        cfg.stored_configuration().clone()
    };

    let tx_propose = new_tx_config_propose(&testkit.network().validators()[1], first_cfg.clone());
    testkit.create_block_with_transactions(txvec![tx_propose]);

    let cfg_proposal_hash = first_cfg.hash();
    let tx_votes = testkit
        .network()
        .validators()
        .iter()
        .map(|validator| new_tx_config_vote(validator, cfg_proposal_hash))
        .map(to_boxed)
        .collect::<Vec<_>>();
    testkit.create_block_with_transactions(tx_votes);

    let second_cfg = {
        let mut cfg = testkit.configuration_change_proposal();
        cfg.set_actual_from(Height(15));
        cfg.set_service_config("dummy", "Second cfg change");
        cfg.stored_configuration().clone()
    };
    let tx_propose = new_tx_config_propose(&testkit.network().validators()[0], second_cfg.clone());
    testkit.create_block_with_transactions(txvec![tx_propose]);
    assert!(testkit.find_propose(second_cfg.hash()).is_none());
    assert!(testkit.find_propose(first_cfg.hash()).is_some());
}

#[test]
fn test_config_txs_discarded_when_not_referencing_actual_config_or_sent_by_illegal_validator() {
    let mut testkit: TestKit = TestKit::configuration_default();
    let initial_cfg = Schema::new(&testkit.snapshot()).actual_configuration();

    let new_cfg_bad_previous_cfg = {
        let mut cfg = testkit.configuration_change_proposal();
        cfg.set_actual_from(Height(6));
        let mut stored = cfg.stored_configuration().clone();
        stored.previous_cfg_hash = Hash::new([11; HASH_SIZE]);
        stored
    };
    let new_cfg = {
        let mut cfg = testkit.configuration_change_proposal();
        cfg.set_actual_from(Height(6));
        cfg.set_service_config("dummy", "Following cfg");
        cfg.stored_configuration().clone()
    };
    let discarded_votes_cfg = {
        let mut cfg = testkit.configuration_change_proposal();
        cfg.set_actual_from(Height(8));
        cfg.set_service_config("dummy", "Following cfg");
        cfg.stored_configuration().clone()
    };

    {
        let illegal_node = TestNode::new_validator(ValidatorId(0));
        let illegal_propose1 = new_tx_config_propose(
            &testkit.network().validators()[1],
            new_cfg_bad_previous_cfg.clone(),
        );
        let illegal_propose2 = new_tx_config_propose(&illegal_node, new_cfg.clone());
        testkit.create_block_with_transactions(txvec![illegal_propose1, illegal_propose2]);
        assert!(
            testkit
                .find_propose(new_cfg_bad_previous_cfg.hash())
                .is_none()
        );
        assert!(testkit.find_propose(new_cfg.hash()).is_none());
    }
    {
        let legal_propose1 =
            new_tx_config_propose(&testkit.network().validators()[1], new_cfg.clone());
        let legal_propose2 = new_tx_config_propose(
            &testkit.network().validators()[1],
            discarded_votes_cfg.clone(),
        );
        testkit.create_block_with_transactions(txvec![legal_propose1, legal_propose2]);
        assert!(testkit.find_propose(discarded_votes_cfg.hash()).is_some());
        assert!(testkit.find_propose(new_cfg.hash()).is_some());
    }
    {
        let illegal_node = TestNode::new_auditor();
        let illegal_validator_vote = new_tx_config_vote(&illegal_node, discarded_votes_cfg.hash());
        testkit.create_block_with_transactions(txvec![illegal_validator_vote.clone()]);
        assert!(!testkit
            .votes_for_propose(discarded_votes_cfg.hash())
            .contains(&Some(VotingDecision::Yea(illegal_validator_vote))))
    }
    {
        let votes = (0..3)
            .map(|id| new_tx_config_vote(&testkit.network().validators()[id], new_cfg.hash()))
            .map(to_boxed)
            .collect::<Vec<_>>();
        testkit.create_block_with_transactions(votes);
        assert_eq!(
            initial_cfg,
            Schema::new(&testkit.snapshot()).actual_configuration()
        );
        assert_eq!(
            Some(new_cfg.clone()),
            Schema::new(&testkit.snapshot()).following_configuration()
        );
    }
    {
        testkit.create_block();
        assert_eq!(
            new_cfg,
            Schema::new(&testkit.snapshot()).actual_configuration()
        );
        assert!(
            Schema::new(&testkit.snapshot())
                .following_configuration()
                .is_none()
        );
    }
    {
        let expected_votes = (0..3)
            .map(|id| {
                new_tx_config_vote(
                    &testkit.network().validators()[id],
                    discarded_votes_cfg.hash(),
                )
            })
            .collect::<Vec<_>>();
        testkit.create_block_with_transactions(expected_votes.clone().into_iter().map(to_boxed));

        let actual_votes = testkit.votes_for_propose(discarded_votes_cfg.hash());
        for expected_vote in expected_votes {
            assert!(!actual_votes.contains(&Some(VotingDecision::Yea(expected_vote))));
        }
    }
}

/// regression: votes' were summed for all proposes simultaneously, and not for the same propose
#[test]
fn test_regression_majority_votes_for_different_proposes() {
    let mut testkit: TestKit = TestKit::configuration_default();
    let initial_cfg = Schema::new(&testkit.snapshot()).actual_configuration();

    let actual_from = Height(5);
    let new_cfg1 = {
        let mut cfg = testkit.configuration_change_proposal();
        cfg.set_actual_from(actual_from);
        cfg.set_service_config("dummy", "First cfg");
        cfg.stored_configuration().clone()
    };
    let new_cfg2 = {
        let mut cfg = testkit.configuration_change_proposal();
        cfg.set_actual_from(actual_from);
        cfg.set_service_config("dummy", "Second cfg");
        cfg.stored_configuration().clone()
    };
    {
        let proposes = [new_cfg1.clone(), new_cfg2.clone()]
            .into_iter()
            .map(|cfg| {
                let validator = &testkit.network().validators()[1];
                new_tx_config_propose(&validator, cfg.clone())
            })
            .map(to_boxed)
            .collect::<Vec<_>>();
        testkit.create_block_with_transactions(proposes);
    }
    {
        let votes = (0..2)
            .map(|validator| {
                let validator = &testkit.network().validators()[validator];
                new_tx_config_vote(validator, new_cfg1.hash())
            })
            .map(to_boxed)
            .collect::<Vec<_>>();
        testkit.create_block_with_transactions(votes);
        assert_eq!(
            initial_cfg,
            Schema::new(&testkit.snapshot()).actual_configuration()
        );
    }
    {
        let prop2_validator2 =
            new_tx_config_vote(&testkit.network().validators()[2], new_cfg2.hash());
        testkit.create_block_with_transactions(txvec![prop2_validator2]);
        assert_eq!(
            initial_cfg,
            Schema::new(&testkit.snapshot()).actual_configuration()
        );
    }
    {
        let prop1_validator2 =
            new_tx_config_vote(&testkit.network().validators()[2], new_cfg1.hash());
        testkit.create_block_with_transactions(txvec![prop1_validator2]);
        assert_eq!(
            new_cfg1,
            Schema::new(&testkit.snapshot()).actual_configuration()
        );
    }
}

#[test]
fn test_regression_new_vote_for_older_config_applies_old_config() {
    let mut testkit: TestKit = TestKit::configuration_default();

    let new_cfg1 = {
        let mut cfg = testkit.configuration_change_proposal();
        cfg.set_actual_from(Height(3));
        cfg.set_service_config("dummy", "First cfg");
        cfg.stored_configuration().clone()
    };
    let new_cfg2 = {
        let mut cfg = testkit.configuration_change_proposal();
        cfg.set_actual_from(Height(5));
        cfg.set_service_config("dummy", "Second cfg");
        let mut stored = cfg.stored_configuration().clone();
        stored.previous_cfg_hash = new_cfg1.hash();
        stored
    };
    {
        let propose_tx1 =
            new_tx_config_propose(&testkit.network().validators()[1], new_cfg1.clone());
        testkit.create_block_with_transactions(txvec![propose_tx1]);
        let votes = (0..3)
            .map(|validator| {
                let validator = &testkit.network().validators()[validator];
                new_tx_config_vote(validator, new_cfg1.hash())
            })
            .map(to_boxed)
            .collect::<Vec<_>>();
        testkit.create_block_with_transactions(votes);
        assert_eq!(
            new_cfg1,
            Schema::new(&testkit.snapshot()).actual_configuration()
        );
    }
    {
        let propose_tx2 =
            new_tx_config_propose(&testkit.network().validators()[1], new_cfg2.clone());
        testkit.create_block_with_transactions(txvec![propose_tx2]);
        let votes = (0..3)
            .map(|validator| {
                let validator = &testkit.network().validators()[validator];
                new_tx_config_vote(validator, new_cfg2.hash())
            })
            .map(to_boxed)
            .collect::<Vec<_>>();
        testkit.create_block_with_transactions(votes);
        assert_eq!(Height(4), testkit.height());
        assert_eq!(
            new_cfg2,
            Schema::new(&testkit.snapshot()).actual_configuration()
        );
    }
    {
        let prop1_validator3 =
            new_tx_config_propose(&testkit.network().validators()[3], new_cfg1.clone());
        testkit.create_block_with_transactions(txvec![prop1_validator3]);
        assert_eq!(
            new_cfg2,
            Schema::new(&testkit.snapshot()).actual_configuration()
        );
    }
}
