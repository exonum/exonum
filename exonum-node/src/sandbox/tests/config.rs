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

//! Tests in this module are designed to test configuration change protocol.

use exonum::{
    helpers::{Height, ValidatorId},
    merkledb::BinaryValue,
};

use crate::sandbox::{sandbox_tests_helper::*, supervisor::TxConfig, timestamping_sandbox};

/// - exclude validator from consensus
/// - idea of test is to exclude sandbox validator from consensus
/// - node continues as `full node`
#[test]
fn test_exclude_validator_from_consensus() {
    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();

    add_one_height(&sandbox, &sandbox_state);

    let tx_cfg = {
        let mut consensus_cfg = sandbox.cfg();
        consensus_cfg.validator_keys.swap_remove(0);

        TxConfig::create_signed(
            sandbox.public_key(ValidatorId(0)),
            &consensus_cfg.into_bytes(),
            Height(0),
            sandbox.secret_key(ValidatorId(0)),
        )
    };

    add_one_height_with_transactions(&sandbox, &sandbox_state, &[tx_cfg]);
    // node loses validator status
    add_one_height_with_transactions_from_other_validator(&sandbox, &sandbox_state, &[]);
}

// - include validator to consensus
// TODO [ECR-3222]
