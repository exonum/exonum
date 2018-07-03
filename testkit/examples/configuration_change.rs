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

extern crate exonum;
extern crate exonum_testkit;
extern crate serde;
extern crate serde_json;

use exonum::{
    blockchain::Schema, crypto::CryptoHash, helpers::{Height, ValidatorId},
};
use exonum_testkit::TestKitBuilder;

fn main() {
    let mut testkit = TestKitBuilder::auditor().with_validators(3).create();

    let cfg_change_height = Height(5);
    let proposal = {
        let mut cfg = testkit.configuration_change_proposal();
        // Add us to validators.
        let mut validators = cfg.validators().to_vec();
        validators.push(testkit.network().us().clone());
        cfg.set_validators(validators);
        // Change configuration of our service.
        cfg.set_service_config("my_service", "My config");
        // Set the height with which the configuration takes effect.
        cfg.set_actual_from(cfg_change_height);
        cfg
    };
    // Save proposed configuration.
    let stored = proposal.stored_configuration().clone();
    // Commit configuration change proposal to the testkit.
    testkit.commit_configuration_change(proposal);
    // Create blocks up to the height preceding the `actual_from` height.
    testkit.create_blocks_until(cfg_change_height.previous());
    // Check that the proposal has become actual.
    assert_eq!(testkit.network().us().validator_id(), Some(ValidatorId(3)));
    assert_eq!(testkit.validator(ValidatorId(3)), testkit.network().us());
    assert_eq!(testkit.actual_configuration(), stored);
    assert_eq!(
        Schema::new(&testkit.snapshot())
            .previous_configuration()
            .unwrap()
            .hash(),
        stored.previous_cfg_hash
    );
}
