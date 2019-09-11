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

// TODO Implement new configuration change logic [ECR-3285]

#[macro_use]
extern crate pretty_assertions;

use exonum::helpers::{Height, ValidatorId};
use exonum_testkit::TestKitBuilder;

#[test]
#[ignore = "Implement new configuration change logic [ECR-3285]"]
fn test_configuration_and_rollbacks() {
    let mut testkit = TestKitBuilder::validator().create();
    testkit.create_blocks_until(Height(5));

    let cfg_change_height = Height(10);
    let proposal = {
        let mut cfg = testkit.configuration_change_proposal();
        cfg.set_actual_from(cfg_change_height);
        cfg.set_service_config("service", "config");
        cfg
    };
    let old_config = testkit.consensus_config();
    let new_config = proposal.consensus_config().clone();

    testkit.checkpoint();

    testkit.commit_configuration_change(proposal);
    testkit.create_blocks_until(Height(10));
    assert_eq!(testkit.consensus_config(), new_config);

    testkit.checkpoint();
    testkit.create_block();
    testkit.rollback();
    assert_eq!(testkit.consensus_config(), new_config);

    testkit.rollback();

    // As rollback is behind the time a proposal entered the blockchain,
    // the proposal is effectively forgotten.
    testkit.create_blocks_until(Height(10));
    assert_eq!(testkit.consensus_config(), old_config);
}

#[test]
fn test_add_to_validators() {
    let mut testkit = TestKitBuilder::auditor().with_validators(1).create();

    let cfg_change_height = Height(5);
    let proposal = {
        let mut cfg = testkit.configuration_change_proposal();
        let mut validators = cfg.validators().to_vec();
        validators.push(testkit.network().us().clone());
        cfg.set_actual_from(cfg_change_height);
        cfg.set_validators(validators);
        cfg
    };
    let stored = proposal.consensus_config().clone();
    testkit.commit_configuration_change(proposal);

    testkit.create_blocks_until(cfg_change_height.previous());

    assert_eq!(testkit.network().us().validator_id(), Some(ValidatorId(1)));
    assert_eq!(&testkit.network().validators()[1], testkit.network().us());
    assert_eq!(testkit.consensus_config(), stored);
}

#[test]
fn test_exclude_from_validators() {
    let mut testkit = TestKitBuilder::validator().with_validators(2).create();

    let cfg_change_height = Height(5);
    let proposal = {
        let mut cfg = testkit.configuration_change_proposal();
        let validator = cfg.validators()[1].clone();
        cfg.set_actual_from(cfg_change_height);
        cfg.set_validators(vec![validator]);
        cfg
    };
    let stored = proposal.consensus_config().clone();
    testkit.commit_configuration_change(proposal);

    testkit.create_blocks_until(cfg_change_height.previous());

    assert_eq!(testkit.network().us().validator_id(), None);
    assert_eq!(testkit.network().validators().len(), 1);
    assert_eq!(testkit.consensus_config(), stored);
}

#[test]
#[ignore = "Implement new configuration change logic [ECR-3285]"]
fn test_change_service_config() {}

#[test]
#[should_panic(expected = "The `actual_from` height should be greater than the current")]
fn test_incorrect_actual_from_field() {
    let mut testkit = TestKitBuilder::auditor().with_validators(1).create();
    testkit.create_blocks_until(Height(2));
    let proposal = {
        let mut cfg = testkit.configuration_change_proposal();
        cfg.set_actual_from(Height(2));
        cfg
    };
    testkit.commit_configuration_change(proposal);
}

#[test]
#[should_panic(expected = "There is an active configuration change proposal")]
fn test_another_configuration_change_proposal() {
    let mut testkit = TestKitBuilder::auditor().with_validators(1).create();
    let first_proposal = {
        let mut cfg = testkit.configuration_change_proposal();
        cfg.set_actual_from(Height(10));
        cfg
    };
    testkit.commit_configuration_change(first_proposal);
    let second_proposal = {
        let mut cfg = testkit.configuration_change_proposal();
        cfg.set_actual_from(Height(11));
        cfg
    };
    testkit.commit_configuration_change(second_proposal);
}
