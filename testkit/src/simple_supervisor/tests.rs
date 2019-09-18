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

use exonum::helpers::{Height, ValidatorId};

use crate::{
    simple_supervisor::{ConfigPropose, SimpleSupervisor},
    TestKitBuilder,
};

#[test]
fn add_nodes_to_validators() {
    let mut testkit = TestKitBuilder::auditor()
        .with_validators(1)
        .with_service(SimpleSupervisor)
        .with_logger()
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

    testkit.create_block_with_transaction(
        ConfigPropose::actual_from(cfg_change_height)
            .consensus_config(new_consensus_config.clone())
            .into_tx(),
    );
    testkit.create_blocks_until(cfg_change_height.previous());
    assert_eq!(testkit.network().us().validator_id(), None);
    testkit.create_block();

    assert_eq!(testkit.network().us().validator_id(), Some(ValidatorId(1)));
    assert_eq!(&testkit.network().validators()[1], testkit.network().us());
    assert_eq!(testkit.consensus_config(), new_consensus_config);
}

#[test]
fn exclude_us_from_validators() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(2)
        .with_service(SimpleSupervisor)
        .with_logger()
        .create();

    let cfg_change_height = Height(5);
    let new_consensus_config = {
        let mut cfg = testkit.consensus_config();
        // Remove us from validators
        cfg.validator_keys.remove(0);
        cfg
    };

    testkit.create_block_with_transaction(
        ConfigPropose::actual_from(cfg_change_height)
            .consensus_config(new_consensus_config.clone())
            .into_tx(),
    );
    testkit.create_blocks_until(cfg_change_height);

    assert_eq!(testkit.consensus_config(), new_consensus_config);
    assert_eq!(testkit.network().us().validator_id(), None);
}

#[test]
fn exclude_other_from_validators() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(2)
        .with_service(SimpleSupervisor)
        .with_logger()
        .create();

    let cfg_change_height = Height(5);
    let new_consensus_config = {
        let mut cfg = testkit.consensus_config();
        // Remove one of node from validators
        cfg.validator_keys.remove(1);
        cfg
    };

    testkit.create_block_with_transaction(
        ConfigPropose::actual_from(cfg_change_height)
            .consensus_config(new_consensus_config.clone())
            .into_tx(),
    );
    testkit.create_blocks_until(cfg_change_height);

    assert_eq!(testkit.consensus_config(), new_consensus_config);
    assert_eq!(testkit.network().us().validator_id(), Some(ValidatorId(0)));
}

#[test]
fn change_us_validator_id() {
    let mut testkit = TestKitBuilder::validator()
        .with_validators(2)
        .with_service(SimpleSupervisor)
        .with_logger()
        .create();

    let cfg_change_height = Height(5);
    let new_consensus_config = {
        let mut cfg = testkit.consensus_config();
        // Remove one of node from validators
        cfg.validator_keys.swap(0, 1);
        cfg
    };

    testkit.create_block_with_transaction(
        ConfigPropose::actual_from(cfg_change_height)
            .consensus_config(new_consensus_config.clone())
            .into_tx(),
    );
    testkit.create_blocks_until(cfg_change_height);

    assert_eq!(testkit.network().us().validator_id(), Some(ValidatorId(1)));
    assert_eq!(&testkit.network().validators()[1], testkit.network().us());
    assert_eq!(testkit.consensus_config(), new_consensus_config);
}
