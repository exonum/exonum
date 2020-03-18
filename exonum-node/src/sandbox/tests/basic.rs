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

//! Tests in this module are designed to test basic functionality
//! related to consensus protocol handling, such as ability of the node
//! to add block after receiving correct consensus messages.

use exonum::{
    crypto::{Hash, KeyPair, Seed, HASH_SIZE, SEED_LENGTH},
    helpers::{Height, Round, ValidatorId},
    merkledb::ObjectHash,
    messages::{Precommit, Verified},
    runtime::SnapshotExt,
};
use rand::{thread_rng, Rng};

use std::collections::BTreeMap;

use crate::sandbox::{
    sandbox_tests_helper::*,
    timestamping::{TimestampingTxGenerator, DATA_SIZE},
    timestamping_sandbox, timestamping_sandbox_builder, Sandbox,
};

/// idea of the test is to verify that at certain periodic rounds we (`validator_0`) become a leader
/// assumption: in some loops current node becomes a leader
#[test]
fn test_check_leader() {
    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();

    // option: with transaction
    let tx = gen_timestamping_tx();
    sandbox.recv(&tx);

    let n_rounds_without_request_peers = {
        let mut rounds = 0;
        let mut time_left = sandbox.cfg().peers_timeout;
        while time_left > 0 {
            time_left = time_left.saturating_sub(
                sandbox.first_round_timeout() + rounds * sandbox.round_timeout_increase(),
            );
            rounds += 1;
        }
        Round(rounds as u32)
    };

    let mut was_leader = false;
    for round in Round::first().iter_to(n_rounds_without_request_peers) {
        sandbox.assert_state(Height(1), round);
        add_round_with_transactions(&sandbox, &sandbox_state, &[tx.object_hash()]);
        sandbox.assert_state(Height(1), round.next());
        was_leader = was_leader || sandbox.is_leader();
    }
    assert!(was_leader);

    add_round_with_transactions(&sandbox, &sandbox_state, &[tx.object_hash()]);

    // Status timeout is equal to peers timeout in sandbox' ConsensusConfig.
    sandbox.broadcast(&Sandbox::create_status(
        sandbox.public_key(ValidatorId(0)),
        Height(1),
        sandbox.last_block().object_hash(),
        1,
        sandbox.secret_key(ValidatorId(0)),
    ));

    sandbox.send_peers_request();
}

/// idea of the test is to reach one height
#[test]
fn test_reach_one_height() {
    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();

    add_one_height(&sandbox, &sandbox_state);
    sandbox.assert_state(Height(2), Round(1));
}

/// The idea of the test is to reach one height in the network with single validator.
#[test]
fn test_one_validator() {
    let sandbox = timestamping_sandbox_builder().with_validators(1).build();
    let sandbox_state = SandboxState::new();

    add_one_height(&sandbox, &sandbox_state);
    sandbox.assert_state(Height(2), Round(1));
}

/// Validator2,3,4 starts in 5th round
/// Validator1 starts with delay
/// Validator1 receive consensus messages, and reach actual round
#[test]
fn test_reach_actual_round() {
    let sandbox = timestamping_sandbox();

    // get some tx
    let tx = gen_timestamping_tx();

    let block_at_first_height = BlockBuilder::new(&sandbox)
        .with_proposer_id(ValidatorId(3))
        .with_tx_hash(&tx.object_hash())
        .build();

    let future_propose = sandbox.create_propose(
        ValidatorId(3),
        Height(1),
        Round(4),
        block_at_first_height.clone().object_hash(),
        vec![], // there are no transactions in future propose
        sandbox.secret_key(ValidatorId(3)),
    );

    sandbox.assert_state(Height(1), Round(1));
    sandbox.recv(&future_propose);
    sandbox.assert_state(Height(1), Round(1));
    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(2),
        Height(1),
        Round(4),
        block_at_first_height.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(2)),
    ));

    sandbox.assert_state(Height(1), Round(4));
}

/// idea of the test is to reach one height two times and compare block hash
#[test]
fn test_reach_one_height_repeatable() {
    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();

    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
    sandbox.assert_state(Height(2), Round(1));
    let hash_1 = sandbox.last_block().object_hash();

    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();

    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
    sandbox.assert_state(Height(2), Round(1));
    let hash_2 = sandbox.last_block().object_hash();

    assert_eq!(hash_2, hash_1);
}

/// idea of the test is to reach some height
/// assumptions: status timeout and `request_peers` timeout are not handled in this test,
/// so, according timeouts should be big enough not to occur
#[test]
fn test_reach_thirteen_height() {
    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();

    let target_height = 13;

    for height in 2..=target_height {
        add_one_height(&sandbox, &sandbox_state);
        sandbox.assert_state(Height(height), Round(1));
    }
}

#[test]
fn test_query_state_hash() {
    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();
    // We do not change the state hash in between blocks for TimestampingService for now
    for _ in 0..2 {
        let state_hash = sandbox.last_state_hash();
        let configs_rh = sandbox.get_configs_merkle_root();

        let proof = sandbox.get_proof_to_index("core.consensus_config");
        let proof = proof.check_against_hash(state_hash).unwrap();
        assert_ne!(configs_rh, Hash::zero());
        assert_eq!(
            proof
                .entries()
                .map(|(k, v)| (k.as_str(), v))
                .collect::<Vec<_>>(),
            vec![("core.consensus_config", &configs_rh)]
        );

        let proof = sandbox.get_proof_to_index("timestamping.first");
        let proof = proof.check_against_hash(state_hash).unwrap();
        assert_eq!(
            proof
                .entries()
                .map(|(k, v)| (k.as_str(), v))
                .collect::<Vec<_>>(),
            vec![("timestamping.first", &Hash::new([127; HASH_SIZE]))]
        );

        let proof = sandbox.get_proof_to_index("timestamping.second");
        let proof = proof.check_against_hash(state_hash).unwrap();
        assert_eq!(
            proof
                .entries()
                .map(|(k, v)| (k.as_str(), v))
                .collect::<Vec<_>>(),
            vec![("timestamping.second", &Hash::new([128; HASH_SIZE]))]
        );

        add_one_height(&sandbox, &sandbox_state)
    }
}

#[test]
fn test_retrieve_block_and_precommits() {
    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();

    let target_height = Height(6);

    for _ in 2..=target_height.0 {
        add_one_height(&sandbox, &sandbox_state);
    }
    sandbox.assert_state(target_height, Round(1));

    let block_proof = sandbox
        .block_and_precommits(target_height.previous())
        .unwrap();
    let block = block_proof.block;
    let precommits: Vec<Verified<Precommit>> = block_proof.precommits;
    let expected_height = target_height.previous();
    let expected_block_hash = block.object_hash();

    assert_eq!(expected_height, block.height);
    for precommit in precommits {
        assert_eq!(expected_height, precommit.payload().epoch);
        assert_eq!(expected_block_hash, precommit.payload().block_hash);
    }
    assert!(sandbox.block_and_precommits(target_height).is_none());
}

#[test]
fn test_store_txs_positions() {
    let mut rng = thread_rng();
    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();

    let generator = TimestampingTxGenerator::with_keypair(
        DATA_SIZE,
        KeyPair::from_seed(&Seed::new([11; SEED_LENGTH])),
    );

    let committed_height = Height(rng.gen_range(2, 30_u64));
    for _ in 1..committed_height.0 {
        add_one_height(&sandbox, &sandbox_state);
    }
    let num_txs = rng.gen_range(3, 100);
    let committed_block1 = generator
        .take(num_txs)
        .map(|tx| (tx.object_hash(), tx))
        .collect::<BTreeMap<_, _>>();

    let hashes =
        add_one_height_with_transactions(&sandbox, &sandbox_state, committed_block1.values());
    sandbox.assert_state(committed_height.next(), Round(1));

    let snapshot = sandbox.blockchain().snapshot();
    let schema = snapshot.for_core();
    let locations = schema.transactions_locations();
    for (expected_idx, hash) in (0_u32..).zip(&hashes) {
        let location = locations.get(hash).unwrap();
        assert_eq!(expected_idx, location.position_in_block());
        assert_eq!(committed_height, location.block_height());
    }
}

#[test]
fn tx_cache_with_tx_block_limit() {
    let sandbox = timestamping_sandbox_builder()
        .with_consensus(|config| {
            config.txs_block_limit = 5;
        })
        .build();

    let generator = TimestampingTxGenerator::with_keypair(
        DATA_SIZE,
        KeyPair::from_seed(&Seed::new([10; SEED_LENGTH])),
    );

    let num_txs = 10;
    let txs = generator
        .take(num_txs)
        .map(|tx| (tx.object_hash(), tx))
        .collect::<BTreeMap<_, _>>();

    for tx in txs.values() {
        sandbox.recv(tx)
    }

    sandbox.assert_tx_cache_len(10);

    //TODO: check pool after commit.
}
