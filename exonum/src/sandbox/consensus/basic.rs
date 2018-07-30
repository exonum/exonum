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

//! Tests in this module are designed to test basic functionality
//! related to consensus protocol handling, such as ability of the node
//! to add block after receiving correct consensus messages.

use rand::{thread_rng, Rng};

use std::collections::BTreeMap;

use blockchain::{Blockchain, Schema};
use crypto::{gen_keypair_from_seed, CryptoHash, Hash, Seed, HASH_SIZE, SEED_LENGTH};
use helpers::{Height, Round};
use messages::{Message, Precommit, Prevote, Propose, RawMessage, CONSENSUS};
use sandbox::{
    sandbox::timestamping_sandbox, sandbox_tests_helper::*,
    timestamping::{TimestampingTxGenerator, TIMESTAMPING_SERVICE},
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

    // TODO: Would be nice to check also for RequestPeers message which will appear
    // after 10 time units (at 11th round). (ECR-1627)
    let n_rounds_without_request_peers = Round(
        (sandbox.cfg().consensus.peers_timeout / sandbox.cfg().consensus.round_timeout) as u32,
    );

    for round in Round::first().iter_to(n_rounds_without_request_peers) {
        sandbox.assert_state(HEIGHT_ONE, round);
        add_round_with_transactions(&sandbox, &sandbox_state, &[tx.hash()]);
        sandbox.assert_state(HEIGHT_ONE, round.next());
    }
}

/// idea of the test is to reach one height
#[test]
fn test_reach_one_height() {
    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();

    add_one_height(&sandbox, &sandbox_state);
    sandbox.assert_state(HEIGHT_TWO, Round(1));
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
        .with_proposer_id(VALIDATOR_3)
        .with_tx_hash(&tx.hash())
        .build();

    let future_propose = Propose::new(
        VALIDATOR_3,
        HEIGHT_ONE,
        Round(4),
        &block_at_first_height.clone().hash(),
        &[], // there are no transactions in future propose
        sandbox.s(VALIDATOR_3),
    );

    sandbox.assert_state(HEIGHT_ONE, Round(1));
    sandbox.recv(&future_propose);
    sandbox.assert_state(HEIGHT_ONE, Round(1));
    sandbox.recv(&Prevote::new(
        VALIDATOR_2,
        HEIGHT_ONE,
        Round(4),
        &block_at_first_height.clone().hash(),
        Round::zero(),
        sandbox.s(VALIDATOR_2),
    ));

    sandbox.assert_state(HEIGHT_ONE, Round(4));
}

/// idea of the test is to reach one height two times and compare block hash
#[test]
fn test_reach_one_height_repeatable() {
    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();

    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
    sandbox.assert_state(HEIGHT_TWO, Round(1));
    let hash_1 = sandbox.last_block().hash();

    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();

    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
    sandbox.assert_state(HEIGHT_TWO, Round(1));
    let hash_2 = sandbox.last_block().hash();

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

    for height in 2..target_height + 1 {
        add_one_height(&sandbox, &sandbox_state);
        sandbox.assert_state(Height(height), Round(1));
    }
}

#[test]
fn test_query_state_hash() {
    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();
    //we do not change the state hash in between blocks for TimestampingService for now
    for _ in 0..2 {
        let state_hash = sandbox.last_state_hash();
        let configs_rh = sandbox.get_configs_merkle_root();
        let configs_key = Blockchain::service_table_unique_key(CONSENSUS, 0);
        let timestamp_t1_key = Blockchain::service_table_unique_key(TIMESTAMPING_SERVICE, 0);
        let timestamp_t2_key = Blockchain::service_table_unique_key(TIMESTAMPING_SERVICE, 1);

        let proof_configs = sandbox.get_proof_to_service_table(CONSENSUS, 0);
        let proof = proof_configs.check().unwrap();
        assert_eq!(proof.merkle_root(), state_hash);
        assert_ne!(configs_rh, Hash::zero());
        assert_eq!(proof.entries(), vec![(&configs_key, &configs_rh)]);

        let proof_configs = sandbox.get_proof_to_service_table(TIMESTAMPING_SERVICE, 0);
        let proof = proof_configs.check().unwrap();
        assert_eq!(proof.merkle_root(), state_hash);
        assert_eq!(
            proof.entries(),
            vec![(&timestamp_t1_key, &Hash::new([127; HASH_SIZE]))]
        );

        let proof_configs = sandbox.get_proof_to_service_table(TIMESTAMPING_SERVICE, 1);
        let proof = proof_configs.check().unwrap();
        assert_eq!(proof.merkle_root(), state_hash);
        assert_eq!(
            proof.entries(),
            vec![(&timestamp_t2_key, &Hash::new([128; HASH_SIZE]))]
        );

        add_one_height(&sandbox, &sandbox_state)
    }
}

#[test]
fn test_retrieve_block_and_precommits() {
    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();

    let target_height = Height(6);

    for _ in 2..target_height.0 + 1 {
        add_one_height(&sandbox, &sandbox_state)
    }
    sandbox.assert_state(target_height, Round(1));

    let bl_proof_option = sandbox.block_and_precommits(target_height.previous());
    // use serde_json;
    assert!(bl_proof_option.is_some());
    let block_proof = bl_proof_option.unwrap();
    let block = block_proof.block;
    let precommits: Vec<Precommit> = block_proof.precommits;
    let expected_height = target_height.previous();
    let expected_block_hash = block.hash();

    assert_eq!(expected_height, block.height());
    for precommit in precommits {
        assert_eq!(expected_height, precommit.height());
        assert_eq!(expected_block_hash, *precommit.block_hash());
        assert!(
            precommit
                .raw()
                .verify_signature(&sandbox.p(precommit.validator()),)
        );
    }
    let bl_proof_option = sandbox.block_and_precommits(target_height);
    assert!(bl_proof_option.is_none());
}

#[test]
fn test_store_txs_positions() {
    let mut rng = thread_rng();
    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();

    let data_size = 20;
    let generator = TimestampingTxGenerator::with_keypair(
        data_size,
        gen_keypair_from_seed(&Seed::new([11; SEED_LENGTH])),
    );

    let committed_height = Height(rng.gen_range(2, 30_u64));
    {
        for _ in 1..committed_height.0 {
            add_one_height(&sandbox, &sandbox_state)
        }
    }
    let num_txs = rng.gen_range(3, 100);
    let committed_block1 = generator
        .take(num_txs)
        .map(|tx| (tx.hash(), tx.raw().clone()))
        .collect::<BTreeMap<Hash, RawMessage>>();

    let hashes =
        add_one_height_with_transactions(&sandbox, &sandbox_state, committed_block1.values());
    sandbox.assert_state(committed_height.next(), Round(1));

    let snapshot = sandbox.blockchain_ref().snapshot();
    let schema = Schema::new(&snapshot);
    let locations = schema.transactions_locations();
    for (expected_idx, hash) in hashes.iter().enumerate() {
        let location = locations.get(hash).unwrap();
        assert_eq!(expected_idx as u64, location.position_in_block());
        assert_eq!(committed_height, location.block_height());
    }
}
