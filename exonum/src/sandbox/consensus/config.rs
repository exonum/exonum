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

//! Tests in this module are designed to test configuration change protocol.

use crate::blockchain::Schema;
use crate::crypto::CryptoHash;
use crate::helpers::{Height, ValidatorId};
use crate::sandbox::{config_updater::TxConfig, sandbox_tests_helper::*, timestamping_sandbox};
use exonum_merkledb::BinaryValue;

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
        consensus_cfg.actual_from = sandbox.current_height().next().next();
        consensus_cfg.previous_cfg_hash = sandbox.cfg().hash();

        TxConfig::create_signed(
            &sandbox.public_key(ValidatorId(0)),
            &consensus_cfg.clone().into_bytes(),
            consensus_cfg.actual_from,
            sandbox.secret_key(ValidatorId(0)),
        )
    };

    add_one_height_with_transactions(&sandbox, &sandbox_state, &[tx_cfg.clone()]);
    add_one_height(&sandbox, &sandbox_state);
    // node loses validator status
    add_one_height_with_transactions_from_other_validator(&sandbox, &sandbox_state, &[]);
}

/// - Check `following_configuration` method in schema
/// - idea of the test is check configurations method from schema
#[test]
fn test_schema_config_changes() {
    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();

    add_one_height(&sandbox, &sandbox_state);

    let (tx_cfg, following_cfg) = {
        let mut consensus_cfg = sandbox.cfg();
        consensus_cfg.consensus.txs_block_limit = 2000;
        consensus_cfg.actual_from = sandbox.current_height().next().next();
        consensus_cfg.previous_cfg_hash = sandbox.cfg().hash();

        let tx = TxConfig::create_signed(
            &sandbox.public_key(ValidatorId(0)),
            &consensus_cfg.clone().into_bytes(),
            consensus_cfg.actual_from,
            sandbox.secret_key(ValidatorId(0)),
        );
        (tx, consensus_cfg)
    };
    let prev_cfg = sandbox.cfg();

    // Check configuration from genesis block
    assert_eq!(
        Schema::new(&sandbox.blockchain_ref().snapshot()).actual_configuration(),
        prev_cfg
    );
    // Try to get configuration from non exists height
    assert_eq!(
        Schema::new(&sandbox.blockchain_ref().snapshot()).configuration_by_height(Height(4)),
        prev_cfg
    );
    // Commit a new configuration
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[tx_cfg.clone()]);
    // Check that following configuration is visible
    assert_eq!(
        Schema::new(&sandbox.blockchain_ref().snapshot()).following_configuration(),
        Some(following_cfg.clone())
    );
    // Make following configuration actual
    add_one_height(&sandbox, &sandbox_state);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
    // Check that following configuration becomes actual
    assert_eq!(
        Schema::new(&sandbox.blockchain_ref().snapshot()).actual_configuration(),
        following_cfg
    );
    // Check previous configuration
    assert_eq!(
        Schema::new(&sandbox.blockchain_ref().snapshot())
            .previous_configuration()
            .unwrap(),
        prev_cfg
    );

    // Finally check configuration for some heights
    assert_eq!(
        Schema::new(&sandbox.blockchain_ref().snapshot()).configuration_by_height(Height(0)),
        prev_cfg
    );
    assert_eq!(
        Schema::new(&sandbox.blockchain_ref().snapshot())
            .configuration_by_height(sandbox.current_height()),
        following_cfg
    );
}
