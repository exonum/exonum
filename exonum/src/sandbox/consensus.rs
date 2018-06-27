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

// spell-checker:ignore precommiters, uncommented, uncomment, commited

use bit_vec::BitVec;
use rand::{thread_rng, Rng};

use std::{collections::BTreeMap, time::Duration};

use super::{
    config_updater::TxConfig, sandbox::{sandbox_with_services_uninitialized, timestamping_sandbox},
    sandbox_tests_helper::*,
    timestamping::{TimestampTx, TimestampingTxGenerator, TIMESTAMPING_SERVICE},
};
use blockchain::{Blockchain, Schema};
use crypto::{gen_keypair, gen_keypair_from_seed, CryptoHash, Hash, Seed};
use helpers::{user_agent, Height, Round};
use messages::{
    BlockRequest, BlockResponse, Connect, Message, PeersRequest, Precommit, Prevote,
    PrevotesRequest, Propose, ProposeRequest, RawMessage, Status, TransactionsRequest,
    TransactionsResponse, CONSENSUS,
};
use node::{
    self,
    state::{
        BLOCK_REQUEST_TIMEOUT, PREVOTES_REQUEST_TIMEOUT, PROPOSE_REQUEST_TIMEOUT,
        TRANSACTIONS_REQUEST_TIMEOUT,
    },
};

// HANDLE CONSENSUS BASIC

#[test]
fn test_queue_message_from_future_round() {
    let sandbox = timestamping_sandbox();

    let propose = Propose::new(
        VALIDATOR_3,
        HEIGHT_ONE,
        ROUND_TWO,
        &sandbox.last_hash(),
        &[],
        sandbox.s(VALIDATOR_3),
    );

    sandbox.recv(&propose);
    sandbox.add_time(Duration::from_millis(sandbox.round_timeout() - 1));
    sandbox.assert_state(HEIGHT_ONE, ROUND_ONE);
    sandbox.add_time(Duration::from_millis(1));
    sandbox.assert_state(HEIGHT_ONE, ROUND_TWO);
    sandbox.broadcast(&Prevote::new(
        VALIDATOR_0,
        HEIGHT_ONE,
        ROUND_TWO,
        &propose.hash(),
        LOCK_ZERO,
        sandbox.s(VALIDATOR_0),
    ));
}

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
    sandbox.assert_state(HEIGHT_TWO, ROUND_ONE);
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
        ROUND_FOUR,
        &block_at_first_height.clone().hash(),
        &[], // there are no transactions in future propose
        sandbox.s(VALIDATOR_3),
    );

    sandbox.assert_state(HEIGHT_ONE, ROUND_ONE);
    sandbox.recv(&future_propose);
    sandbox.assert_state(HEIGHT_ONE, ROUND_ONE);
    sandbox.recv(&Prevote::new(
        VALIDATOR_2,
        HEIGHT_ONE,
        ROUND_FOUR,
        &block_at_first_height.clone().hash(),
        Round::zero(),
        sandbox.s(VALIDATOR_2),
    ));

    sandbox.assert_state(HEIGHT_ONE, ROUND_FOUR);
}

/// idea of the test is to reach one height two times and compare block hash
#[test]
fn test_reach_one_height_repeatable() {
    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();

    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
    sandbox.assert_state(HEIGHT_TWO, ROUND_ONE);
    let hash_1 = sandbox.last_block().hash();

    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();

    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
    sandbox.assert_state(HEIGHT_TWO, ROUND_ONE);
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
        sandbox.assert_state(Height(height), ROUND_ONE);
    }
}

#[test]
fn test_disable_and_enable() {
    let mut sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();

    sandbox.assert_state(HEIGHT_ONE, ROUND_ONE);
    try_add_one_height(&sandbox, &sandbox_state).unwrap();
    sandbox.assert_state(HEIGHT_TWO, ROUND_ONE);

    // Disable the node.
    let message = node::ExternalMessage::Enable(false);
    sandbox
        .node_handler_mut()
        .channel
        .api_requests
        .send(message)
        .unwrap();
    sandbox.process_events();

    // Save the current time to "rewind" sandbox to it later.
    let time_saved = sandbox.time();

    // A fail is expected here as the node is disabled.
    sandbox.assert_state(HEIGHT_TWO, ROUND_ONE);
    let result = try_add_one_height(&sandbox, &sandbox_state);
    assert!(result.is_err());

    // Re-enable the node.
    let message = node::ExternalMessage::Enable(true);
    sandbox
        .node_handler_mut()
        .channel
        .api_requests
        .send(message)
        .unwrap();
    sandbox.process_events();

    // Check if the node is still at the same height and round.
    sandbox.assert_state(HEIGHT_TWO, ROUND_ONE);

    // Reset the time.
    sandbox.set_time(time_saved);

    // The node should work fine now
    try_add_one_height(&sandbox, &sandbox_state).unwrap();
    sandbox.assert_state(HEIGHT_THREE, ROUND_ONE);
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
            vec![(&timestamp_t1_key, &Hash::new([127; 32]))]
        );

        let proof_configs = sandbox.get_proof_to_service_table(TIMESTAMPING_SERVICE, 1);
        let proof = proof_configs.check().unwrap();
        assert_eq!(proof.merkle_root(), state_hash);
        assert_eq!(
            proof.entries(),
            vec![(&timestamp_t2_key, &Hash::new([128; 32]))]
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
    sandbox.assert_state(target_height, ROUND_ONE);

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

/// Scenario:
/// - Node sends `Propose` and `Prevote`.
/// - Node restarts.
/// - Node should recover to previous state: jump into the round before stop and does not send
///   `Propose` again.
#[test]
fn should_not_send_propose_and_prevote_after_node_restart() {
    let sandbox = timestamping_sandbox();

    // round happens
    sandbox.add_time(Duration::from_millis(sandbox.round_timeout()));
    sandbox.add_time(Duration::from_millis(
        sandbox.round_timeout() + PROPOSE_TIMEOUT,
    ));

    assert!(sandbox.is_leader());
    sandbox.assert_state(HEIGHT_ONE, ROUND_THREE);

    // ok, we are leader
    let propose = ProposeBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .build();

    let prevote = make_prevote_from_propose(&sandbox, &propose);
    sandbox.broadcast(&propose);
    sandbox.broadcast(&prevote);

    let current_height = sandbox.current_height();
    let current_round = sandbox.current_round();

    let sandbox_restarted = sandbox.restart();

    sandbox_restarted.broadcast(&prevote);
    sandbox_restarted.assert_lock(LOCK_ZERO, None);
    sandbox_restarted.assert_state(current_height, current_round);

    // Now we should be sure that node recovered its state but didn't send any messages.
    // Here sandbox_restarted goes out of scope and sandbox_restarted.drop() will cause panic
    // if there any sent messages
}

/// Idea:
/// - Node gets locked on some Propose from certain validator.
/// - Node restarts.
/// - Node receives other Propose in the same round.
/// - Make sure node doesn't vote for the new `Propose`.
#[test]
fn should_not_vote_after_node_restart() {
    let sandbox = timestamping_sandbox();

    let propose = ProposeBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .build();

    let prevote = make_prevote_from_propose(&sandbox, &propose);

    let block = BlockBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .build();

    sandbox.recv(&propose);

    sandbox.broadcast(&prevote);

    sandbox.recv(&Prevote::new(
        VALIDATOR_1,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        LOCK_ZERO,
        sandbox.s(VALIDATOR_1),
    ));
    sandbox.assert_lock(LOCK_ZERO, None); // Do not lock if <2/3 prevotes

    sandbox.recv(&Prevote::new(
        VALIDATOR_2,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        LOCK_ZERO,
        sandbox.s(VALIDATOR_2),
    ));
    sandbox.assert_lock(LOCK_ONE, Some(propose.hash()));

    let precommit = Precommit::new(
        VALIDATOR_0,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_0),
    );
    sandbox.broadcast(&precommit);
    sandbox.assert_lock(LOCK_ONE, Some(propose.hash()));
    let current_height = sandbox.current_height();
    let current_round = sandbox.current_round();

    // Simulate node restart.
    let sandbox_restarted = sandbox.restart();

    // Assert that consensus messages were recovered and we're in locked state now.
    sandbox_restarted.assert_lock(LOCK_ONE, Some(propose.hash()));
    sandbox_restarted.assert_state(current_height, current_round);
    sandbox_restarted.broadcast(&prevote);
    sandbox_restarted.broadcast(&precommit);

    // Receive another propose within the round
    let tx = gen_timestamping_tx();
    receive_valid_propose_with_transactions(&sandbox_restarted, &[tx.hash()]);

    // Here sandbox goes out of scope and sandbox.drop() will cause panic if there any sent messages
}

/// Idea: we should check whether outgoing `Precommit` gets saved and recovered from cache.
/// - Node locks on some Propose and broadcasts `Precommit`.
/// - Node restarts.
/// - Node receives Precommits from two other validators. With our recovered one it's already
///   +2/3 of Precommits, and node reaches new height.
#[test]
fn should_save_precommit_to_consensus_cache() {
    let sandbox = timestamping_sandbox();

    let propose = ProposeBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .build();

    let prevote = make_prevote_from_propose(&sandbox, &propose);

    let block = BlockBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .build();

    sandbox.recv(&propose);

    sandbox.broadcast(&prevote);

    sandbox.recv(&Prevote::new(
        VALIDATOR_1,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        LOCK_ZERO,
        sandbox.s(VALIDATOR_1),
    ));
    sandbox.assert_lock(LOCK_ZERO, None); //do not lock if <2/3 prevotes

    sandbox.recv(&Prevote::new(
        VALIDATOR_2,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        LOCK_ZERO,
        sandbox.s(VALIDATOR_2),
    ));
    sandbox.assert_lock(LOCK_ONE, Some(propose.hash()));

    let precommit = Precommit::new(
        VALIDATOR_0,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_0),
    );

    sandbox.broadcast(&precommit);

    let current_height = sandbox.current_height();
    let current_round = sandbox.current_round();

    // Simulate node restart
    let sandbox_restarted = sandbox.restart();

    // assert that consensus messages were recovered and we're in locked state now
    sandbox_restarted.assert_lock(LOCK_ONE, Some(propose.hash()));
    sandbox_restarted.assert_state(current_height, current_round);
    sandbox_restarted.broadcast(&prevote);
    sandbox_restarted.broadcast(&precommit);

    sandbox_restarted.recv(&Precommit::new(
        VALIDATOR_1,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox_restarted.time().into(),
        sandbox_restarted.s(VALIDATOR_1),
    ));

    sandbox_restarted.recv(&Precommit::new(
        VALIDATOR_2,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox_restarted.time().into(),
        sandbox_restarted.s(VALIDATOR_2),
    ));

    sandbox_restarted.assert_state(HEIGHT_TWO, ROUND_ONE);
    sandbox_restarted.check_broadcast_status(HEIGHT_TWO, &block.hash());
}

/// Idea:
/// - Node locks on some Propose.
/// - Node doesn't get any precommits within the round.
/// - Next round appears.
/// - Node receives new propose from new leader and locks on it.
/// - Node restarts.
/// - Node should recover in same round and locked on same `Propose`.
#[test]
fn test_recover_consensus_messages_in_other_round() {
    let sandbox = timestamping_sandbox();

    let first_propose = ProposeBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .build();
    let first_prevote = make_prevote_from_propose(&sandbox, &first_propose);
    let block = BlockBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .build();

    sandbox.recv(&first_propose);

    sandbox.broadcast(&first_prevote);

    sandbox.recv(&Prevote::new(
        VALIDATOR_1,
        HEIGHT_ONE,
        ROUND_ONE,
        &first_propose.hash(),
        LOCK_ZERO,
        sandbox.s(VALIDATOR_1),
    ));
    sandbox.assert_lock(LOCK_ZERO, None); //do not lock if <2/3 prevotes

    sandbox.recv(&Prevote::new(
        VALIDATOR_2,
        HEIGHT_ONE,
        ROUND_ONE,
        &first_propose.hash(),
        LOCK_ZERO,
        sandbox.s(VALIDATOR_2),
    ));
    sandbox.assert_lock(LOCK_ONE, Some(first_propose.hash()));

    let first_precommit = Precommit::new(
        VALIDATOR_0,
        HEIGHT_ONE,
        ROUND_ONE,
        &first_propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_0),
    );

    sandbox.broadcast(&first_precommit);

    sandbox.assert_state(HEIGHT_ONE, ROUND_ONE);
    sandbox.add_time(Duration::from_millis(sandbox.round_timeout()));
    sandbox.assert_state(HEIGHT_ONE, ROUND_TWO);

    // make sure we broadcasted same Prevote for second round
    let first_updated_prevote = Prevote::new(
        first_prevote.validator(),
        first_prevote.height(),
        ROUND_TWO,
        first_prevote.propose_hash(),
        LOCK_ONE,
        sandbox.s(VALIDATOR_0),
    );
    sandbox.broadcast(&first_updated_prevote);

    let second_propose = ProposeBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .build();
    let second_block = BlockBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .build();

    sandbox.recv(&second_propose);

    sandbox.recv(&Prevote::new(
        VALIDATOR_1,
        HEIGHT_ONE,
        ROUND_TWO,
        &second_propose.hash(),
        LOCK_ZERO,
        sandbox.s(VALIDATOR_1),
    ));

    sandbox.recv(&Prevote::new(
        VALIDATOR_2,
        HEIGHT_ONE,
        ROUND_TWO,
        &second_propose.hash(),
        LOCK_ZERO,
        sandbox.s(VALIDATOR_2),
    ));

    sandbox.assert_lock(LOCK_ONE, Some(first_propose.hash()));

    sandbox.recv(&Prevote::new(
        VALIDATOR_3,
        HEIGHT_ONE,
        ROUND_TWO,
        &second_propose.hash(),
        LOCK_ZERO,
        sandbox.s(VALIDATOR_3),
    ));

    sandbox.assert_lock(LOCK_TWO, Some(second_propose.hash()));

    let second_precommit = Precommit::new(
        VALIDATOR_0,
        HEIGHT_ONE,
        ROUND_TWO,
        &second_propose.hash(),
        &second_block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_0),
    );
    sandbox.broadcast(&second_precommit);

    // Restart node.
    let saved_time = sandbox.time();
    let sandbox_new = sandbox.restart_with_time(saved_time);

    sandbox_new.assert_lock(LOCK_TWO, Some(second_propose.hash()));
    sandbox_new.assert_state(HEIGHT_ONE, ROUND_TWO);
    sandbox_new.broadcast(&first_prevote);

    let first_precommit_new_time = Precommit::new(
        first_precommit.validator(),
        first_precommit.height(),
        first_precommit.round(),
        first_precommit.propose_hash(),
        first_precommit.block_hash(),
        sandbox_new.time().into(),
        sandbox_new.s(VALIDATOR_0),
    );
    sandbox_new.broadcast(&first_precommit_new_time);
    sandbox_new.broadcast(&first_updated_prevote);
    sandbox_new.broadcast(&second_precommit);
}

/// - Node 0 is not aware of Node 1
/// - Node 0 receives `PeersRequest` from Node 1 and responds nothing
/// - Node 0 receives `Connect` from Node 1, saves it and trying to connect
/// - Node 0 restarts
/// - Node 0 should connect to Node 1
/// - Node 0 should be aware of Node 1 and send received `Connect` in response to `PeersRequest`
#[test]
fn should_restore_peers_after_restart() {
    // create sandbox with nodes not aware about each other
    let sandbox = sandbox_with_services_uninitialized(vec![]);

    let (v0, v1) = (VALIDATOR_0, VALIDATOR_1);
    let (p0, s0, a0) = (sandbox.p(v0), sandbox.s(v0).clone(), sandbox.a(v0));
    let (p1, s1, a1) = (sandbox.p(v1), sandbox.s(v1).clone(), sandbox.a(v1));

    let time = sandbox.time();
    let connect_from_0 = Connect::new(&p0, a0, time.into(), &user_agent::get(), &s0);
    let connect_from_1 = Connect::new(&p1, a1, time.into(), &user_agent::get(), &s1);
    let peers_request = PeersRequest::new(&p1, &p0, &s1);

    // check that peers are absent
    sandbox.recv(&peers_request);

    // receive a `Connect` message and the respond on it
    sandbox.recv(&connect_from_1);
    sandbox.send(a1, &connect_from_0);

    // restart the node
    let sandbox_restarted = sandbox.restart_uninitialized();

    // check that the node is connecting with the peer
    sandbox_restarted.send(a1, &connect_from_0);

    // check that the peer is restored
    sandbox_restarted.recv(&peers_request);
    sandbox_restarted.send(a1, &connect_from_1);
}

#[test]
fn test_store_txs_positions() {
    let mut rng = thread_rng();
    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();

    let data_size = 20;
    let generator = TimestampingTxGenerator::with_keypair(
        data_size,
        gen_keypair_from_seed(&Seed::new([11; 32])),
    );

    let committed_height = Height(rng.gen_range(2, 30u64));
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
    sandbox.assert_state(committed_height.next(), ROUND_ONE);

    let snapshot = sandbox.blockchain_ref().snapshot();
    let schema = Schema::new(&snapshot);
    let locations = schema.transactions_locations();
    for (expected_idx, hash) in hashes.iter().enumerate() {
        let location = locations.get(hash).unwrap();
        assert_eq!(expected_idx as u64, location.position_in_block());
        assert_eq!(committed_height, location.block_height());
    }
}

/// idea of the scenario is to:
/// - receive correct Prevote for some next height (first one) at 0 time (and respectively 1 height)
/// - queue it
/// - reach that first height
/// - handle queued Prevote
/// - and observe `ProposeRequest` for queued `Prevote`
#[test]
#[should_panic(expected = "Send unexpected message Request(ProposeRequest")]
fn test_queue_prevote_message_from_next_height() {
    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();

    sandbox.recv(&Prevote::new(
        VALIDATOR_3,
        HEIGHT_TWO,
        ROUND_ONE,
        &empty_hash(),
        Round::zero(),
        sandbox.s(VALIDATOR_3),
    ));

    add_one_height(&sandbox, &sandbox_state);
    sandbox.add_time(Duration::from_millis(sandbox.round_timeout() - 1));
    sandbox.add_time(Duration::from_millis(0));
}

/// idea of the scenario is to:
/// - receive correct Propose for some next height (first one) at 0 time (and respectively 1 height)
/// - queue it
/// - reach that first height
/// - handle queued Propose
/// - and observe Prevote for queued Propose
/// check line from `NodeHandler.handle_consensus()`
/// case `msg.height() == self.state.height() + 1`
#[test]
#[should_panic(expected = "Send unexpected message Consensus(Prevote")]
fn test_queue_propose_message_from_next_height() {
    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();

    // get some tx
    let tx = gen_timestamping_tx();

    // TODO: This commented code is saved because it may be used later. (ECR-1627)
    //    let block_at_first_height = Block::new(HEIGHT_ZERO, ROUND_FOUR, future_propose_time,
    //          &sandbox.last_block().unwrap().map_or(hash(&[]), |block| block.hash()), &tx.hash(),
    //          &hash(&[]));
    let block_at_first_height = BlockBuilder::new(&sandbox)
        .with_proposer_id(VALIDATOR_0)
        .with_tx_hash(&tx.hash())
        .with_state_hash(&sandbox.compute_state_hash(&[tx.raw().clone()]))
        .build();

    let future_propose = Propose::new(
        VALIDATOR_0,
        HEIGHT_TWO,
        ROUND_TWO,
        &block_at_first_height.clone().hash(),
        &[], // there are no transactions in future propose
        sandbox.s(VALIDATOR_0),
    );

    sandbox.recv(&future_propose);

    add_one_height_with_transactions(&sandbox, &sandbox_state, &[tx.raw().clone()]);

    info!(
        "last_block={:#?}, hash={:?}",
        sandbox.last_block(),
        sandbox.last_block().hash()
    );
    info!(
        "proposed_block={:#?}, hash={:?}",
        block_at_first_height,
        block_at_first_height.hash()
    );

    sandbox.add_time(Duration::from_millis(sandbox.round_timeout()));
    sandbox.add_time(Duration::from_millis(0));
}

/// idea of scenario is to check line // Ignore messages from previous and future height
/// from `NodeHandler.handle_consensus()`
/// case `msg.height() > self.state.height() + 1`
#[test]
fn test_ignore_message_from_far_height() {
    let sandbox = timestamping_sandbox();

    let propose = ProposeBuilder::new(&sandbox)
        .with_height(HEIGHT_TWO)//without this line some Prevote will be sent
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .build();

    sandbox.recv(&propose);
}

/// idea of scenario is to check line // Ignore messages from previous and future height
/// from `NodeHandler.handle_consensus()`
/// case `msg.height() < self.state.height()`
#[test]
fn test_ignore_message_from_prev_height() {
    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();

    add_one_height(&sandbox, &sandbox_state);

    sandbox.assert_state(HEIGHT_TWO, ROUND_ONE);

    let propose = ProposeBuilder::new(&sandbox)
        .with_height(HEIGHT_ZERO)//without this line some Prevote will be sent
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .build();

    sandbox.recv(&propose);
}

/// check scenario:
/// HANDLE FULL PROPOSE
/// - send prevote when lock=0 for known propose
#[test]
fn positive_get_propose_send_prevote() {
    let sandbox = timestamping_sandbox();

    let propose = ProposeBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .build();
    sandbox.recv(&propose);

    // check scenario:
    // HANDLE FULL PROPOSE
    // - send prevote when lock=0 for known propose
    sandbox.assert_lock(LOCK_ZERO, None);
    sandbox.broadcast(&Prevote::new(
        VALIDATOR_0,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        LOCK_ZERO,
        sandbox.s(VALIDATOR_0),
    ));

    info!("time: {:?}", sandbox.time());
}

#[test]
fn test_ignore_message_with_incorrect_validator_id() {
    let sandbox = timestamping_sandbox();

    let propose = Propose::new(
        INCORRECT_VALIDATOR_ID,
        HEIGHT_ZERO,
        ROUND_ONE,
        &sandbox.last_hash(),
        &[],
        sandbox.s(VALIDATOR_1),
    );

    sandbox.recv(&propose);
}

#[test]
fn test_ignore_message_with_incorrect_signature() {
    let sandbox = timestamping_sandbox();

    let propose = Propose::new(
        VALIDATOR_0,
        HEIGHT_ZERO,
        ROUND_ONE,
        &sandbox.last_hash(),
        &[],
        sandbox.s(VALIDATOR_1),
    );

    sandbox.recv(&propose);
}

// HANDLE PROPOSE

#[test]
fn ignore_propose_with_incorrect_prev_hash() {
    let sandbox = timestamping_sandbox();

    let propose = ProposeBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .with_prev_hash(&empty_hash())
        .build();

    sandbox.recv(&propose);
}

#[test]
fn ignore_propose_from_non_leader() {
    let sandbox = timestamping_sandbox();

    let propose = ProposeBuilder::new(&sandbox)
        .with_validator(VALIDATOR_3)    //without this line Prevote would have been broadcast
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .build();

    sandbox.recv(&propose);
}

/// Propose with incorrect time should be handled as usual.
#[test]
fn handle_propose_with_incorrect_time() {
    let sandbox = timestamping_sandbox();

    let propose = ProposeBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(sandbox.round_timeout() + PROPOSE_TIMEOUT + 1)
        .build();

    sandbox.recv(&propose);

    sandbox.assert_lock(LOCK_ZERO, None);
    sandbox.broadcast(&Prevote::new(
        VALIDATOR_0,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        LOCK_ZERO,
        sandbox.s(VALIDATOR_0),
    ));
}

#[test]
fn ignore_propose_with_commited_transaction() {
    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();

    //    add_one_height(&sandbox);
    add_one_height(&sandbox, &sandbox_state);

    let propose = ProposeBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        // without this line Prevote would have been broadcast
        .with_tx_hashes(sandbox_state.committed_transaction_hashes.borrow().as_ref())
        .build();

    sandbox.recv(&propose);
    //    broadcast here is absent
}

// TODO: Ask Ivan how to test this scenario in terms of messages:
// - remove propose request when getting propose and request txs from known nodes;
//     - not only leader, but also prevotes;
//     - not only leader, but also precommiters.
// (ECR-1627)

#[test]
fn handle_propose_that_sends_before_than_propose_timeout_exceeded() {
    let sandbox = timestamping_sandbox();

    let propose = ProposeBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT - 1)
        .build();

    sandbox.recv(&propose);

    sandbox.assert_lock(LOCK_ZERO, None);
    sandbox.broadcast(&Prevote::new(
        VALIDATOR_0,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        LOCK_ZERO,
        sandbox.s(VALIDATOR_0),
    ));
}

// HANDLE FULL PROPOSE

// - send prevote when lock=0 for known propose//covered in positive_get_propose_send_prevote()
// - not send prevote if lock > 0

// - get lock when getting propose if we have +2/3 prevotes (consensus:88)
//     - only after lock
//     - only after propose_round
//     - only before current round

// - commit when getting propose if we have +2/3 precommits (consensus:96)
// - panic when getting propose if we have +2/3 precommits (consensus:96)

// HANDLE PREVOTE

// - request propose when get prevote
#[test]
fn request_propose_when_get_prevote() {
    let sandbox = timestamping_sandbox();

    sandbox.recv(&Prevote::new(
        VALIDATOR_2,
        HEIGHT_ONE,
        ROUND_ONE,
        &empty_hash(),
        LOCK_ZERO,
        sandbox.s(VALIDATOR_2),
    ));
    sandbox.add_time(Duration::from_millis(sandbox.round_timeout() - 1));
    sandbox.send(
        sandbox.a(VALIDATOR_2),
        &ProposeRequest::new(
            &sandbox.p(VALIDATOR_0),
            &sandbox.p(VALIDATOR_2),
            HEIGHT_ONE,
            &empty_hash(),
            sandbox.s(VALIDATOR_0),
        ),
    );
    sandbox.add_time(Duration::from_millis(0));
}

/// idea of the test is to verify request transaction scenario: other node requests
/// transaction from our node
// TODO: Add scenario with manually created and signed tx. (ECR-1627)
#[test]
fn response_to_request_txs() {
    let sandbox = timestamping_sandbox();

    let tx = gen_timestamping_tx();
    sandbox.recv(&tx);

    sandbox.recv(&TransactionsRequest::new(
        &sandbox.p(VALIDATOR_1),
        &sandbox.p(VALIDATOR_0),
        &[tx.hash()],
        sandbox.s(VALIDATOR_1),
    ));

    sandbox.send(
        sandbox.a(VALIDATOR_1),
        &TransactionsResponse::new(
            &sandbox.p(VALIDATOR_0),
            &sandbox.p(VALIDATOR_1),
            vec![tx.raw().clone()],
            sandbox.s(VALIDATOR_0),
        ),
    );
}

#[test]
fn empty_tx_request() {
    let sandbox = timestamping_sandbox();

    sandbox.recv(&TransactionsRequest::new(
        &sandbox.p(VALIDATOR_1),
        &sandbox.p(VALIDATOR_0),
        &[],
        sandbox.s(VALIDATOR_1),
    ));
}

#[test]
fn duplicate_tx_in_pool() {
    let sandbox = timestamping_sandbox();

    let mut tx_gen = TimestampingTxGenerator::new(64);
    let tx1 = tx_gen.next().unwrap();
    let tx2 = tx_gen.next().unwrap();

    let propose = ProposeBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .with_tx_hashes(&[tx1.hash()])
        .build();

    sandbox.recv(&propose);
    sandbox.add_time(Duration::from_millis(TRANSACTIONS_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.a(VALIDATOR_2),
        &TransactionsRequest::new(
            &sandbox.p(VALIDATOR_0),
            &sandbox.p(VALIDATOR_2),
            &[tx1.hash()],
            &sandbox.s(VALIDATOR_0),
        ),
    );
    sandbox.recv(&tx1);
    sandbox.broadcast(&make_prevote_from_propose(&sandbox, &propose));

    sandbox.recv(&tx2);

    sandbox.recv(&TransactionsResponse::new(
        &sandbox.p(VALIDATOR_2),
        &sandbox.p(VALIDATOR_0),
        vec![tx1.raw().clone()],
        sandbox.s(VALIDATOR_2),
    ));
}

#[test]
#[should_panic(expected = "Send unexpected message Request(TransactionsRequest")]
fn incorrect_tx_in_request() {
    let sandbox = timestamping_sandbox();

    let (pub_key, _) = gen_keypair();
    let (_, sec_key) = gen_keypair();
    let data = vec![0; 64];
    let tx0 = TimestampTx::new(&pub_key, &data, &sec_key);

    let propose = ProposeBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .with_tx_hashes(&[tx0.hash()])
        .build();

    // Receive propose with unknown transaction `tx0`.
    sandbox.recv(&propose);
    // After `TRANSACTIONS_REQUEST_TIMEOUT` node send request with `tx0`.
    sandbox.add_time(Duration::from_millis(TRANSACTIONS_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.a(VALIDATOR_2),
        &TransactionsRequest::new(
            &sandbox.p(VALIDATOR_0),
            &sandbox.p(VALIDATOR_2),
            &[tx0.hash()],
            &sandbox.s(VALIDATOR_0),
        ),
    );

    // Receive response with invalid `tx0`.
    sandbox.recv(&TransactionsResponse::new(
        &sandbox.p(VALIDATOR_2),
        &sandbox.p(VALIDATOR_0),
        vec![tx0.raw().clone()],
        sandbox.s(VALIDATOR_2),
    ));

    add_one_height(&sandbox, &SandboxState::new());

    let tx1 = gen_timestamping_tx();
    let propose = ProposeBuilder::new(&sandbox)
        .with_tx_hashes(&[tx0.hash(), tx1.hash()])
        .build();

    sandbox.recv(&tx1);
    // Receive new propose with `tx0` and `tx1`.
    // `tx1` - valid and after receiving go to the pool.
    // `tx0` - invalid and after receiving should be dismissed.
    sandbox.recv(&propose);

    sandbox.add_time(Duration::from_millis(TRANSACTIONS_REQUEST_TIMEOUT));
}

#[test]
fn response_size_larger_than_max_message_len() {
    use crypto::{PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH};
    use messages::HEADER_LENGTH;
    use storage::StorageValue;

    const EMPTY_RESPONSE_SIZE: usize =
        (HEADER_LENGTH + SIGNATURE_LENGTH + 2 * PUBLIC_KEY_LENGTH + 8);

    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();

    // Create 4 transactions.
    // The size of the fourth transactions is 1 more than size of the first three.
    let tx1 = gen_timestamping_tx();
    let tx2 = gen_timestamping_tx();
    let tx3 = gen_timestamping_tx();
    let (pub_key, sec_key) = gen_keypair();
    let data = vec![0; 65];
    let tx4 = TimestampTx::new(&pub_key, &data, &sec_key);

    assert_eq!(
        tx1.raw().len() + tx2.raw().len() + 1,
        tx3.raw().len() + tx4.raw().len()
    );

    // Create new config. Set the size of the message to a size
    // that is exactly equal to the message to send the first two transactions.
    let tx_cfg = {
        let mut consensus_cfg = sandbox.cfg();
        consensus_cfg.consensus.max_message_len =
            (EMPTY_RESPONSE_SIZE + tx1.raw().len() + tx2.raw().len()) as u32;
        consensus_cfg.actual_from = sandbox.current_height().next();
        consensus_cfg.previous_cfg_hash = sandbox.cfg().hash();

        TxConfig::new(
            &sandbox.p(VALIDATOR_0),
            &consensus_cfg.clone().into_bytes(),
            consensus_cfg.actual_from,
            sandbox.s(VALIDATOR_0),
        )
    };

    add_one_height_with_transactions(&sandbox, &sandbox_state, &[tx_cfg.raw().clone()]);

    sandbox.recv(&tx1);
    sandbox.recv(&tx2);

    // Send request with `tx1` and `tx2`.
    sandbox.recv(&TransactionsRequest::new(
        &sandbox.p(VALIDATOR_1),
        &sandbox.p(VALIDATOR_0),
        &[tx1.hash(), tx2.hash()],
        sandbox.s(VALIDATOR_1),
    ));

    // Receive response with `tx1` and `tx2`.
    sandbox.send(
        sandbox.a(VALIDATOR_1),
        &TransactionsResponse::new(
            &sandbox.p(VALIDATOR_0),
            &sandbox.p(VALIDATOR_1),
            vec![tx1.raw().clone(), tx2.raw().clone()],
            sandbox.s(VALIDATOR_0),
        ),
    );

    sandbox.recv(&tx3);
    sandbox.recv(&tx4);

    // Send request with `tx3` and `tx4`.
    sandbox.recv(&TransactionsRequest::new(
        &sandbox.p(VALIDATOR_1),
        &sandbox.p(VALIDATOR_0),
        &[tx3.hash(), tx4.hash()],
        sandbox.s(VALIDATOR_1),
    ));

    // Receive separate responses with `tx3` and `tx4`.
    sandbox.send(
        sandbox.a(VALIDATOR_1),
        &TransactionsResponse::new(
            &sandbox.p(VALIDATOR_0),
            &sandbox.p(VALIDATOR_1),
            vec![tx3.raw().clone()],
            sandbox.s(VALIDATOR_0),
        ),
    );

    sandbox.send(
        sandbox.a(VALIDATOR_1),
        &TransactionsResponse::new(
            &sandbox.p(VALIDATOR_0),
            &sandbox.p(VALIDATOR_1),
            vec![tx4.raw().clone()],
            sandbox.s(VALIDATOR_0),
        ),
    );
}

/// idea of the test is to
///  - become e leader
///  - receive tx
///  - broadcast propose & prevote
///  - receive 2 prevotes
///  - broadcast precommit
///  - receive requests for tx, propose, prevote & precommit
///   - send responses to above requests
#[test]
fn respond_to_request_tx_propose_prevotes_precommits() {
    let sandbox = timestamping_sandbox();

    {
        // round happens to make us a leader
        sandbox.add_time(Duration::from_millis(sandbox.round_timeout()));
        sandbox.add_time(Duration::from_millis(sandbox.round_timeout()));
        assert!(sandbox.is_leader());
        sandbox.assert_state(HEIGHT_ONE, ROUND_THREE);
    }

    // option: with transaction
    let tx = gen_timestamping_tx();
    sandbox.recv(&tx);

    sandbox.add_time(Duration::from_millis(PROPOSE_TIMEOUT));

    let propose = ProposeBuilder::new(&sandbox)
        .with_tx_hashes(&[tx.hash()]) //ordinary propose, but with this unreceived tx
        .build();

    let block = BlockBuilder::new(&sandbox)
        .with_state_hash(&sandbox.compute_state_hash(&[tx.raw().clone()]))
        .with_tx_hash(&tx.hash())
        .build();

    let precommit_1 = Precommit::new(
        VALIDATOR_1,
        HEIGHT_ONE,
        ROUND_THREE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_1),
    );
    let precommit_2 = Precommit::new(
        VALIDATOR_2,
        HEIGHT_ONE,
        ROUND_THREE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_2),
    );

    // ok, we are leader

    sandbox.broadcast(&propose);
    sandbox.broadcast(&make_prevote_from_propose(&sandbox, &propose));

    {
        // respond to RequestPropose
        sandbox.recv(&ProposeRequest::new(
            &sandbox.p(VALIDATOR_3),
            &sandbox.p(VALIDATOR_0),
            HEIGHT_ONE,
            &propose.hash(),
            sandbox.s(VALIDATOR_3),
        ));

        sandbox.send(sandbox.a(VALIDATOR_3), &propose);
    }

    {
        // respond to RequestPrevotes
        let mut validators = BitVec::from_elem(sandbox.n_validators(), false);
        validators.set(VALIDATOR_3.into(), true);

        sandbox.recv(&PrevotesRequest::new(
            &sandbox.p(VALIDATOR_3),
            &sandbox.p(VALIDATOR_0),
            HEIGHT_ONE,
            ROUND_THREE,
            &propose.hash(),
            validators,
            sandbox.s(VALIDATOR_3),
        ));

        sandbox.send(
            sandbox.a(VALIDATOR_3),
            &make_prevote_from_propose(&sandbox, &propose),
        );
    }

    sandbox.recv(&Prevote::new(
        VALIDATOR_1,
        HEIGHT_ONE,
        ROUND_THREE,
        &propose.hash(),
        Round::zero(),
        sandbox.s(VALIDATOR_1),
    ));
    sandbox.recv(&Prevote::new(
        VALIDATOR_2,
        HEIGHT_ONE,
        ROUND_THREE,
        &propose.hash(),
        Round::zero(),
        sandbox.s(VALIDATOR_2),
    ));

    sandbox.broadcast(&Precommit::new(
        VALIDATOR_0,
        HEIGHT_ONE,
        ROUND_THREE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_0),
    ));

    sandbox.recv(&precommit_1);
    sandbox.recv(&precommit_2);

    sandbox.assert_state(HEIGHT_TWO, ROUND_ONE);
    sandbox.check_broadcast_status(HEIGHT_TWO, &block.hash());

    {
        // respond to RequestTransactions
        sandbox.recv(&TransactionsRequest::new(
            &sandbox.p(VALIDATOR_1),
            &sandbox.p(VALIDATOR_0),
            &[tx.hash()],
            sandbox.s(VALIDATOR_1),
        ));

        sandbox.send(
            sandbox.a(VALIDATOR_1),
            &TransactionsResponse::new(
                &sandbox.p(VALIDATOR_0),
                &sandbox.p(VALIDATOR_1),
                vec![tx.raw().clone()],
                sandbox.s(VALIDATOR_0),
            ),
        );
    }

    {
        // respond to RequestPropose negative
        sandbox.recv(&ProposeRequest::new(
            &sandbox.p(VALIDATOR_3),
            &sandbox.p(VALIDATOR_0),
            HEIGHT_ONE,
            &propose.hash(),
            sandbox.s(VALIDATOR_3),
        ));

        // here state.height() is 1 already, so requestPropose with height zero is ignored
        //        sandbox.send(
        //            sandbox.a(VALIDATOR_3),
        //            propose.clone()
        //        );
    }

    {
        // respond to RequestPrevotes negative
        let mut validators = BitVec::from_elem(sandbox.n_validators(), false);
        validators.set(VALIDATOR_3.into(), true);

        sandbox.recv(&PrevotesRequest::new(
            &sandbox.p(VALIDATOR_3),
            &sandbox.p(VALIDATOR_0),
            HEIGHT_ONE,
            ROUND_THREE,
            &propose.hash(),
            validators,
            sandbox.s(VALIDATOR_3),
        ));

        // here state.height() is 1 already, so RequestPrevotes with height zero is ignored
        //        sandbox.send(
        //            sandbox.a(VALIDATOR_3),
        //            make_prevote_from_propose(&sandbox, &propose)
        //        );
    }

    sandbox.add_time(Duration::from_millis(0));
}

/// HANDLE TX
///     - remove tx request
/// scenario:
/// - receive propose and tx
/// - verify that tx is not requested (without tx received requestTx request would be sent)
#[test]
fn not_request_txs_when_get_tx_and_propose() {
    let sandbox = timestamping_sandbox();

    // option: with transaction
    let tx = gen_timestamping_tx();
    sandbox.recv(&tx);

    let propose = ProposeBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .with_tx_hashes(&[tx.hash()]) //ordinary propose, but with this unreceived tx
        .build();

    sandbox.recv(&propose);
    sandbox.broadcast(&make_prevote_from_propose(&sandbox, &propose));
    sandbox.add_time(Duration::from_millis(TRANSACTIONS_REQUEST_TIMEOUT));
}

/// HANDLE TX
/// - verify signature
/// - should panic because tx has wrong signature and is not considered
#[cfg_attr(rustfmt, rustfmt_skip)]
#[test]
#[should_panic(expected = "Send unexpected message Request(TransactionsRequest")]
fn handle_tx_verify_signature() {
    let sandbox = timestamping_sandbox();

    // generate incorrect tx
    let (public_key1, _) = gen_keypair();
    let (_, secret_key2) = gen_keypair();

    let data = vec![0; 64]; // TODO: Find the way how to get rid of hard-coded value. (ECR-1627)
    let tx = TimestampTx::new(&public_key1, &data, &secret_key2);
    sandbox.recv(&tx);

    let propose = ProposeBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .with_tx_hashes(&[tx.hash()]) //ordinary propose, but with this unreceived tx
        .build();

    sandbox.recv(&propose);
    sandbox.add_time(Duration::from_millis(TRANSACTIONS_REQUEST_TIMEOUT));
}

/// - request txs when get propose
/// - request tx from propose with unknown tx
/// - scenario 'request unknown txs when get prevote' is also covered in this test
/// - idea of the test:
///   - receive Propose with unknown transaction
///   - request unknown transaction
///   - receive Prevote for above Propose
///   - again request unknown transaction from above Propose
#[test]
fn request_txs_when_get_propose_or_prevote() {
    let sandbox = timestamping_sandbox();

    // option: with transaction
    let tx = gen_timestamping_tx();

    let propose = ProposeBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .with_tx_hashes(&[tx.hash()]) //ordinary propose, but with this unreceived tx
        .build();

    sandbox.recv(&propose);
    sandbox.add_time(Duration::from_millis(sandbox.round_timeout() - 1));

    sandbox.send(
        sandbox.a(VALIDATOR_2),
        &TransactionsRequest::new(
            &sandbox.p(VALIDATOR_0),
            &sandbox.p(VALIDATOR_2),
            &[tx.hash()],
            sandbox.s(VALIDATOR_0),
        ),
    );

    sandbox.add_time(Duration::from_millis(0));

    sandbox.recv(&Prevote::new(
        VALIDATOR_3,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        Round::zero(),
        sandbox.s(VALIDATOR_3),
    ));

    sandbox.add_time(Duration::from_millis(sandbox.round_timeout() - 1));

    sandbox.send(
        sandbox.a(VALIDATOR_3),
        &TransactionsRequest::new(
            &sandbox.p(VALIDATOR_0),
            &sandbox.p(VALIDATOR_3),
            &[tx.hash()],
            sandbox.s(VALIDATOR_0),
        ),
    );

    sandbox.add_time(Duration::from_millis(0));
}

/// - request prevotes when get prevote message
#[test]
fn request_prevotes_when_get_prevote_message() {
    let sandbox = timestamping_sandbox();

    sandbox.recv(&Prevote::new(
        VALIDATOR_2,
        HEIGHT_ONE,
        ROUND_ONE,
        &empty_hash(),
        LOCK_ONE,
        sandbox.s(VALIDATOR_2),
    ));
    sandbox.add_time(Duration::from_millis(sandbox.round_timeout() - 1));
    sandbox.send(
        sandbox.a(VALIDATOR_2),
        &ProposeRequest::new(
            &sandbox.p(VALIDATOR_0),
            &sandbox.p(VALIDATOR_2),
            HEIGHT_ONE,
            &empty_hash(),
            sandbox.s(VALIDATOR_0),
        ),
    );

    let mut validators = BitVec::from_elem(sandbox.n_validators(), false);
    validators.set(VALIDATOR_2.into(), true);

    sandbox.send(
        sandbox.a(VALIDATOR_2),
        &PrevotesRequest::new(
            &sandbox.p(VALIDATOR_0),
            &sandbox.p(VALIDATOR_2),
            HEIGHT_ONE,
            ROUND_ONE,
            &empty_hash(),
            validators,
            sandbox.s(VALIDATOR_0),
        ),
    );
    sandbox.add_time(Duration::from_millis(0));
}

/// - lock to propose when get +2/3 prevote
///     - only if round > locked round
/// &
/// - Send prevote
//     - round > locked + 1
/// scenario:
/// - at 0 time:
///  - receive Propose
///  - broadcast Prevote for received Propose
///  - receive 2 more Prevotes for received Propose
///  - verify that lock is obtained
///  - add few rounds and verify that prevote is broadcast
#[test]
fn lock_to_propose_when_get_2_3_prevote_positive() {
    let sandbox = timestamping_sandbox();

    let propose = ProposeBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .build();

    let block = BlockBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .build();

    sandbox.recv(&propose);
    sandbox.broadcast(&Prevote::new(
        VALIDATOR_0,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        LOCK_ZERO,
        sandbox.s(VALIDATOR_0),
    ));

    sandbox.recv(&Prevote::new(
        VALIDATOR_1,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        LOCK_ZERO,
        sandbox.s(VALIDATOR_1),
    ));
    sandbox.assert_lock(LOCK_ZERO, None); //do not lock if <2/3 prevotes

    sandbox.recv(&Prevote::new(
        VALIDATOR_2,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        LOCK_ZERO,
        sandbox.s(VALIDATOR_2),
    ));
    sandbox.assert_lock(LOCK_ONE, Some(propose.hash())); //only if round > locked round

    sandbox.broadcast(&Precommit::new(
        VALIDATOR_0,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_0),
    ));
    sandbox.assert_lock(LOCK_ONE, Some(propose.hash()));
    sandbox.add_time(Duration::from_millis(0));

    {
        // Send prevote even if current round > locked + 1
        // add round
        sandbox.add_time(Duration::from_millis(sandbox.round_timeout()));
        sandbox.broadcast(&Prevote::new(
            VALIDATOR_0,
            HEIGHT_ONE,
            ROUND_TWO,
            &propose.hash(),
            LOCK_ONE,
            sandbox.s(VALIDATOR_0),
        ));

        // add round
        sandbox.add_time(Duration::from_millis(sandbox.round_timeout()));
        sandbox.broadcast(&Prevote::new(
            VALIDATOR_0,
            HEIGHT_ONE,
            ROUND_THREE,
            &propose.hash(),
            LOCK_ONE,
            sandbox.s(VALIDATOR_0),
        ));
    }
    sandbox.add_time(Duration::from_millis(0));
}

/// idea: lock to propose from past round and observe broadcast Prevote
/// LOCK
/// - Send prevote
///     - round < `current_round`
#[test]
fn lock_to_past_round_broadcast_prevote() {
    let sandbox = timestamping_sandbox();

    sandbox.add_time(Duration::from_millis(PROPOSE_TIMEOUT));

    let propose = ProposeBuilder::new(&sandbox).build();

    let block = BlockBuilder::new(&sandbox).build();

    sandbox.recv(&propose);
    sandbox.broadcast(&make_prevote_from_propose(&sandbox, &propose));

    sandbox.add_time(Duration::from_millis(
        sandbox.round_timeout() - PROPOSE_TIMEOUT,
    ));
    sandbox.assert_state(HEIGHT_ONE, ROUND_TWO);

    sandbox.recv(&Prevote::new(
        VALIDATOR_1,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        LOCK_ZERO,
        sandbox.s(VALIDATOR_1),
    ));
    sandbox.assert_lock(LOCK_ZERO, None); //do not lock if <2/3 prevotes

    sandbox.recv(&Prevote::new(
        VALIDATOR_2,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        LOCK_ZERO,
        sandbox.s(VALIDATOR_2),
    ));
    sandbox.assert_lock(LOCK_ONE, Some(propose.hash())); //only if round > locked round

    sandbox.broadcast(&Precommit::new(
        VALIDATOR_0,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_0),
    ));
    sandbox.assert_lock(LOCK_ONE, Some(propose.hash()));
    // ! here broadcast of
    sandbox.broadcast(&Prevote::new(
        VALIDATOR_0,
        HEIGHT_ONE,
        ROUND_TWO,
        &propose.hash(),
        LOCK_ONE,
        sandbox.s(VALIDATOR_0),
    ));
    sandbox.add_time(Duration::from_millis(0));

    {
        // Send prevote even if current round > locked + 1
        // add round
        sandbox.add_time(Duration::from_millis(sandbox.round_timeout()));
        sandbox.broadcast(&Prevote::new(
            VALIDATOR_0,
            HEIGHT_ONE,
            ROUND_THREE,
            &propose.hash(),
            LOCK_ONE,
            sandbox.s(VALIDATOR_0),
        ));

        // add round
        sandbox.add_time(Duration::from_millis(sandbox.round_timeout()));
        sandbox.broadcast(&Prevote::new(
            VALIDATOR_0,
            HEIGHT_ONE,
            ROUND_FOUR,
            &propose.hash(),
            LOCK_ONE,
            sandbox.s(VALIDATOR_0),
        ));
    }
    sandbox.add_time(Duration::from_millis(0));
}

/// HANDLE PRECOMMIT //all are done
/// - Request prevotes
///     - if `msg.round` > locked round    // covered in `handle_precommit_remove_request_prevotes`
/// idea of the scenario:
///   - obtain lock
///   - receive precommit in same round
///   - verify that `PrevotesRequest` are absent
#[test]
fn handle_precommit_remove_request_prevotes() {
    let sandbox = timestamping_sandbox();

    let propose = ProposeBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .build();

    let block = BlockBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .build();

    sandbox.recv(&propose);
    sandbox.broadcast(&Prevote::new(
        VALIDATOR_0,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        LOCK_ZERO,
        sandbox.s(VALIDATOR_0),
    ));

    sandbox.recv(&Prevote::new(
        VALIDATOR_1,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        LOCK_ZERO,
        sandbox.s(VALIDATOR_1),
    ));
    sandbox.assert_lock(LOCK_ZERO, None); //do not lock if <2/3 prevotes

    {
        // in this block lock is obtained; without this lock requestPrevotes would have been sent
        sandbox.recv(&Prevote::new(
            VALIDATOR_2,
            HEIGHT_ONE,
            ROUND_ONE,
            &propose.hash(),
            LOCK_ZERO,
            sandbox.s(VALIDATOR_2),
        ));
        sandbox.assert_lock(LOCK_ONE, Some(propose.hash())); //only if round > locked round

        sandbox.broadcast(&Precommit::new(
            VALIDATOR_0,
            HEIGHT_ONE,
            ROUND_ONE,
            &propose.hash(),
            &block.hash(),
            sandbox.time().into(),
            sandbox.s(VALIDATOR_0),
        ));
        sandbox.assert_lock(LOCK_ONE, Some(propose.hash()));
        sandbox.add_time(Duration::from_millis(0));
    }

    sandbox.recv(&Precommit::new(
        VALIDATOR_1,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_1),
    ));
    sandbox.add_time(Duration::from_millis(PREVOTES_REQUEST_TIMEOUT));
}

/// get lock and send prevote
/// LOCK
/// - Send prevote
/// idea of the scenario:
/// - in 1st round receive
///   - empty propose in order to send some Prevote
///    - broadcast Prevote for this empty Propose
///   - and real Propose from 2nd round with some tx
///   - and this tx
///   - main consequence: Prevote for real Propose is not broadcasted
/// - then in 2nd round
///   - no Prevotes are sent because neither Proposes in this round nor lock in
///     previous rounds are present
///   - receive prevotes
///   - and make lock
///   - and broadcast Prevote from lock
#[test]
fn lock_to_propose_and_send_prevote() {
    let sandbox = timestamping_sandbox();

    let empty_propose = ProposeBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .build();

    sandbox.recv(&empty_propose);
    sandbox.broadcast(&make_prevote_from_propose(&sandbox, &empty_propose.clone()));

    let tx = gen_timestamping_tx();
    sandbox.recv(&tx);

    let propose = ProposeBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(sandbox.round_timeout() + PROPOSE_TIMEOUT)
        .with_tx_hashes(&[tx.hash()])
        .build();
    let block = BlockBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(sandbox.round_timeout() + PROPOSE_TIMEOUT)
        .with_tx_hash(&tx.hash())
        .with_state_hash(&sandbox.compute_state_hash(&[tx.raw().clone()]))
        .build();

    sandbox.recv(&propose);

    // inc round
    sandbox.add_time(Duration::from_millis(sandbox.round_timeout()));

    sandbox.recv(&Prevote::new(
        VALIDATOR_1,
        HEIGHT_ONE,
        ROUND_TWO,
        &propose.hash(),
        LOCK_ZERO,
        sandbox.s(VALIDATOR_1),
    ));
    sandbox.assert_lock(LOCK_ZERO, None); //do not lock if <2/3 prevotes

    sandbox.recv(&Prevote::new(
        VALIDATOR_2,
        HEIGHT_ONE,
        ROUND_TWO,
        &propose.hash(),
        LOCK_ZERO,
        sandbox.s(VALIDATOR_2),
    ));
    sandbox.assert_lock(LOCK_ZERO, None);

    sandbox.recv(&Prevote::new(
        VALIDATOR_3,
        HEIGHT_ONE,
        ROUND_TWO,
        &propose.hash(),
        LOCK_ZERO,
        sandbox.s(VALIDATOR_3),
    ));
    sandbox.assert_lock(LOCK_TWO, Some(propose.hash())); //only if round > locked round

    // !! here broadcast, of prevote from lock() function, occurs
    sandbox.broadcast(&Prevote::new(
        VALIDATOR_0,
        HEIGHT_ONE,
        ROUND_TWO,
        &propose.hash(),
        LOCK_ZERO,
        sandbox.s(VALIDATOR_0),
    ));

    sandbox.broadcast(&Precommit::new(
        VALIDATOR_0,
        HEIGHT_ONE,
        ROUND_TWO,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_0),
    ));
    sandbox.assert_lock(LOCK_TWO, Some(propose.hash()));
    sandbox.add_time(Duration::from_millis(0));
}

/// LOCK
///     - if get +2/3 prevotes
///         - remove prevote request
/// idea of the scenario:
///  - just obtain lock
///  - wait `PREVOTES_REQUEST_TIMEOUT`
///  - verify that `PrevotesRequest` request is absent (it would have been observed without
///    last block with appropriate comment)
#[test]
fn lock_remove_request_prevotes() {
    let sandbox = timestamping_sandbox();

    // add round
    sandbox.add_time(Duration::from_millis(sandbox.round_timeout()));

    let propose = ProposeBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .build();

    let block = BlockBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .build();

    sandbox.recv(&propose);
    sandbox.broadcast(&make_prevote_from_propose(&sandbox, &propose));

    sandbox.recv(&Prevote::new(
        VALIDATOR_2,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        LOCK_ONE,
        sandbox.s(VALIDATOR_2),
    ));
    sandbox.recv(&Prevote::new(
        VALIDATOR_3,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        LOCK_ONE,
        sandbox.s(VALIDATOR_3),
    ));

    {
        // without this block RequestPrevotes would have been broadcast
        sandbox.recv(&Prevote::new(
            VALIDATOR_1,
            HEIGHT_ONE,
            ROUND_ONE,
            &propose.hash(),
            LOCK_ONE,
            sandbox.s(VALIDATOR_1),
        ));
        sandbox.broadcast(&Prevote::new(
            VALIDATOR_0,
            HEIGHT_ONE,
            ROUND_ONE,
            &propose.hash(),
            LOCK_ZERO,
            sandbox.s(VALIDATOR_0),
        ));
        sandbox.broadcast(&Precommit::new(
            VALIDATOR_0,
            HEIGHT_ONE,
            ROUND_ONE,
            &propose.hash(),
            &block.hash(),
            sandbox.time().into(),
            sandbox.s(VALIDATOR_0),
        ));
    }
    sandbox.add_time(Duration::from_millis(PREVOTES_REQUEST_TIMEOUT));
}

/// scenario: // HANDLE PRECOMMIT positive scenario
///         - Our `block_hash` is different from the precommits one.
#[test]
#[should_panic(expected = "Our block_hash different from precommits one.")]
fn handle_precommit_different_block_hash() {
    let sandbox = timestamping_sandbox();

    // option: with transaction
    let tx = gen_timestamping_tx();

    let propose = ProposeBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .with_tx_hashes(&[tx.hash()]) //ordinary propose, but with this unreceived tx
        .build();

    // precommits with this block will be received
    // without tx
    let block = BlockBuilder::new(&sandbox).build();

    let precommit_1 = Precommit::new(
        VALIDATOR_1,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_1),
    );
    let precommit_2 = Precommit::new(
        VALIDATOR_2,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_2),
    );
    let precommit_3 = Precommit::new(
        VALIDATOR_3,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_3),
    );

    sandbox.recv(&precommit_1);
    sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.a(VALIDATOR_1),
        &make_request_propose_from_precommit(&sandbox, &precommit_1),
    );
    sandbox.send(
        sandbox.a(VALIDATOR_1),
        &make_request_prevote_from_precommit(&sandbox, &precommit_1),
    );
    sandbox.recv(&propose);
    sandbox.recv(&tx);
    sandbox.broadcast(&make_prevote_from_propose(&sandbox, &propose));

    sandbox.recv(&precommit_2);
    // Here consensus.rs->handle_majority_precommits()->//Commit is achieved
    sandbox.recv(&precommit_3);
}

/// scenario: // HANDLE PRECOMMIT positive scenario with commit
#[test]
fn handle_precommit_positive_scenario_commit() {
    let sandbox = timestamping_sandbox();

    // option: with transaction
    let tx = gen_timestamping_tx();

    let propose = ProposeBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .with_tx_hashes(&[tx.hash()]) //ordinary propose, but with this unreceived tx
        .build();
    let block = BlockBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .with_tx_hash(&tx.hash())
        .with_state_hash(&sandbox.compute_state_hash(&[tx.raw().clone()]))
        .build();

    let precommit_1 = Precommit::new(
        VALIDATOR_1,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_1),
    );
    let precommit_2 = Precommit::new(
        VALIDATOR_2,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_2),
    );
    let precommit_3 = Precommit::new(
        VALIDATOR_3,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_3),
    );

    sandbox.recv(&precommit_1);
    sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.a(VALIDATOR_1),
        &make_request_propose_from_precommit(&sandbox, &precommit_1),
    );
    sandbox.send(
        sandbox.a(VALIDATOR_1),
        &make_request_prevote_from_precommit(&sandbox, &precommit_1),
    );

    sandbox.recv(&precommit_2);
    // second addition is required in order to make sandbox time >= propose time because
    // this condition is checked at node/mod.rs->actual_round()
    sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.a(VALIDATOR_2),
        &make_request_propose_from_precommit(&sandbox, &precommit_2),
    );
    sandbox.send(
        sandbox.a(VALIDATOR_2),
        &make_request_prevote_from_precommit(&sandbox, &precommit_2),
    );
    sandbox.recv(&propose);
    sandbox.recv(&tx);
    sandbox.broadcast(&make_prevote_from_propose(&sandbox, &propose));

    // Here covered negative scenario for requirement: commit only If has +2/3 precommit
    sandbox.assert_state(HEIGHT_ONE, ROUND_ONE);
    // Here consensus.rs->handle_majority_precommits()->//Commit is achieved
    sandbox.recv(&precommit_3);
    sandbox.assert_state(HEIGHT_TWO, ROUND_ONE);
    sandbox.check_broadcast_status(HEIGHT_TWO, &block.hash());
    sandbox.add_time(Duration::from_millis(0));
}

/// LOCK
/// - Send precommit during the lock   // covered in `lock_to_propose_when_get_2_3_prevote_positive`
/// - if there aren't incompatible prevotes
///     - if there're +2/3 precommits
///         - remove precommit request // TODO: This idea is unreachable because there are no
///           other places in the code where `PrecommitRequest` is added. (ECR-1627)
///         - COMMIT // covered in `test_reach_one_height`
///         -> do not send prevotes after the commit
///
/// idea of the scenario:
///  - receive 2 precommits
///  - receive prevotes => make lock, and, hence, one more precommit, and, hence,
///    obtain majority precommits
///  - make commit
///  - increment round
///  - observe absence of broadcast prevote (because lock has been committed already) message
/// idea of alternate scenario
/// (which will occur with commented `precommit_2` and uncommented last broadcast of Prevote):
///  - receive single precommit
///  - receive prevotes => make lock
///  - but do not make commit (because only 2 precommits are collected)
///  - increment round
///  - observe Prevote for lock obtained at previous round
#[test]
fn lock_not_send_prevotes_after_commit() {
    let sandbox = timestamping_sandbox();

    let propose = ProposeBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .build();

    // precommits with this block will be received
    let block = BlockBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .build();

    let precommit_1 = Precommit::new(
        VALIDATOR_1,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_1),
    );
    let precommit_2 = Precommit::new(
        VALIDATOR_2,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_2),
    );

    {
        sandbox.recv(&precommit_1);
        sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
        sandbox.send(
            sandbox.a(VALIDATOR_1),
            &make_request_propose_from_precommit(&sandbox, &precommit_1),
        );
        sandbox.send(
            sandbox.a(VALIDATOR_1),
            &make_request_prevote_from_precommit(&sandbox, &precommit_1),
        );
    }

    {
        // !!! if comment this block, then commit during lock will not occur,
        // and last Prevote would have been observed
        sandbox.recv(&precommit_2);
        // second addition is required in order to make sandbox time >= propose time
        // because this condition is checked at node/mod.rs->actual_round()
        sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
        sandbox.send(
            sandbox.a(VALIDATOR_2),
            &make_request_propose_from_precommit(&sandbox, &precommit_2),
        );
        sandbox.send(
            sandbox.a(VALIDATOR_2),
            &make_request_prevote_from_precommit(&sandbox, &precommit_2),
        );
    }

    {
        // this block will make lock, and, hence, one additional precommit
        sandbox.recv(&propose);
        sandbox.broadcast(&make_prevote_from_propose(&sandbox, &propose));

        sandbox.recv(&Prevote::new(
            VALIDATOR_2,
            HEIGHT_ONE,
            ROUND_ONE,
            &propose.hash(),
            LOCK_ZERO,
            sandbox.s(VALIDATOR_2),
        ));
        sandbox.assert_lock(LOCK_ZERO, None); //do not lock if <2/3 prevotes

        sandbox.recv(&Prevote::new(
            VALIDATOR_3,
            HEIGHT_ONE,
            ROUND_ONE,
            &propose.hash(),
            LOCK_ZERO,
            sandbox.s(VALIDATOR_3),
        ));

        sandbox.broadcast(&Precommit::new(
            VALIDATOR_0,
            HEIGHT_ONE,
            ROUND_ONE,
            &propose.hash(),
            &block.hash(),
            sandbox.time().into(),
            sandbox.s(VALIDATOR_0),
        ));
        sandbox.check_broadcast_status(HEIGHT_TWO, &block.hash());
    }

    //    add rounds to become a leader to observe broadcast messages
    sandbox.add_time(Duration::from_millis(sandbox.round_timeout()));
    {
        // this broadcast of Prevote will occur only if block with precommit_2 is commented
        // it is possible to comment block of code with precommit_2 and uncomment below
        // broadcast of Prevote and test will remain green
        // if block with precommit_2 is uncommented, then during lock commit will occur and
        // lock will disappear and prevotes for disappeared lock (these prevotes are the
        // primary goal of the test) will not be sent
        //  !!!      sandbox.broadcast(&Prevote::new(VALIDATOR_0, HEIGHT_ZERO, ROUND_TWO,
        // &propose.hash(), LOCK_ONE, sandbox.s(VALIDATOR_0)));
    }
}

/// scenario: // HANDLE PRECOMMIT
///         - COMMIT
///         - only if propose is known negative scenario
#[test]
fn do_not_commit_if_propose_is_unknown() {
    let sandbox = timestamping_sandbox();

    // option: with transaction
    let tx = gen_timestamping_tx();

    let propose = ProposeBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .with_tx_hashes(&[tx.hash()]) //ordinary propose, but with this unreceived tx
        .build();

    // this block with transactions should be in real
    let block = BlockBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .with_tx_hash(&tx.hash())
        .build();

    let precommit_1 = Precommit::new(
        VALIDATOR_1,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_1),
    );
    let precommit_2 = Precommit::new(
        VALIDATOR_2,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_2),
    );
    let precommit_3 = Precommit::new(
        VALIDATOR_3,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_3),
    );

    sandbox.recv(&precommit_1);
    sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.a(VALIDATOR_1),
        &make_request_propose_from_precommit(&sandbox, &precommit_1),
    );
    sandbox.send(
        sandbox.a(VALIDATOR_1),
        &make_request_prevote_from_precommit(&sandbox, &precommit_1),
    );

    sandbox.recv(&precommit_2);
    // Second addition is required in order to make sandbox time >= propose time because
    // this condition is checked at node/mod.rs->actual_round()
    sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.a(VALIDATOR_2),
        &make_request_propose_from_precommit(&sandbox, &precommit_2),
    );
    sandbox.send(
        sandbox.a(VALIDATOR_2),
        &make_request_prevote_from_precommit(&sandbox, &precommit_2),
    );
    // !! if this propose would be received, commit would occur and last assert will
    // require height one
    //    sandbox.recv(&propose);

    sandbox.assert_state(HEIGHT_ONE, ROUND_ONE);
    // Here consensus.rs->handle_majority_precommits()->//Commit is achieved
    sandbox.recv(&precommit_3);
    sandbox.assert_state(HEIGHT_ONE, ROUND_ONE);
    sandbox.add_time(Duration::from_millis(0));
}

/// scenario: // HANDLE PRECOMMIT
///         - only if all txs is known negative scenario
#[test]
fn do_not_commit_if_tx_is_unknown() {
    let sandbox = timestamping_sandbox();

    // option: with transaction
    let tx = gen_timestamping_tx();

    let propose = ProposeBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .with_tx_hashes(&[tx.hash()]) //ordinary propose, but with this unreceived tx
        .build();

    // this block with transactions should be in real
    let block = BlockBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .with_tx_hash(&tx.hash())
        .build();

    let precommit_1 = Precommit::new(
        VALIDATOR_1,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_1),
    );
    let precommit_2 = Precommit::new(
        VALIDATOR_2,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_2),
    );
    let precommit_3 = Precommit::new(
        VALIDATOR_3,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_3),
    );

    sandbox.recv(&precommit_1);
    sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.a(VALIDATOR_1),
        &make_request_propose_from_precommit(&sandbox, &precommit_1),
    );
    sandbox.send(
        sandbox.a(VALIDATOR_1),
        &make_request_prevote_from_precommit(&sandbox, &precommit_1),
    );

    sandbox.recv(&precommit_2);
    // Second addition is required in order to make sandbox time >= propose time because
    // this condition is checked at node/mod.rs->actual_round()
    sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.a(VALIDATOR_2),
        &make_request_propose_from_precommit(&sandbox, &precommit_2),
    );
    sandbox.send(
        sandbox.a(VALIDATOR_2),
        &make_request_prevote_from_precommit(&sandbox, &precommit_2),
    );

    sandbox.recv(&propose);
    // !! if this tx would be received, commit would occur and last assert will require height one
    //    sandbox.recv(&tx);

    sandbox.assert_state(HEIGHT_ONE, ROUND_ONE);
    // Here consensus.rs->handle_majority_precommits()->//Commit is achieved
    sandbox.recv(&precommit_3);
    sandbox.assert_state(HEIGHT_ONE, ROUND_ONE);
    sandbox.add_time(Duration::from_millis(0));
}

/// scenario: // HANDLE PRECOMMIT
///         - purpose of the test is to check `add_unknown_propose_with_precommits()`
///         - scenario:
///             - get 3 precommits => majority precommits are observed =>
///               `add_unknown_propose_with_precommits()` is called
///             - then receive valid tx and Propose in order to call `handle_full_propose()` =>
///               commit using `state.unknown_propose_with_precommits`
///         - it appeared that this test is almost the same as
///         `handle_precommit_positive_scenario_commit` the only difference that is in
///         `handle_precommit_positive_scenario_commit` propose and tx are received after second
///         precommit and here propose and tx are received after third precommit
#[test]
fn commit_using_unknown_propose_with_precommits() {
    let sandbox = timestamping_sandbox();

    // option: with transaction
    let tx = gen_timestamping_tx();

    let propose = ProposeBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .with_tx_hashes(&[tx.hash()]) //ordinary propose, but with this unreceived tx
        .build();

    // precommits with this block will be received
    let block = BlockBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .with_tx_hash(&tx.hash())
        .with_state_hash(&sandbox.compute_state_hash(&[tx.raw().clone()]))
        .build();

    let precommit_1 = Precommit::new(
        VALIDATOR_1,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_1),
    );
    let precommit_2 = Precommit::new(
        VALIDATOR_2,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_2),
    );
    let precommit_3 = Precommit::new(
        VALIDATOR_3,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_3),
    );

    sandbox.recv(&precommit_1);
    sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.a(VALIDATOR_1),
        &make_request_propose_from_precommit(&sandbox, &precommit_1),
    );
    sandbox.send(
        sandbox.a(VALIDATOR_1),
        &make_request_prevote_from_precommit(&sandbox, &precommit_1),
    );

    sandbox.recv(&precommit_2);
    // Second addition is required in order to make sandbox time >= propose time because
    // this condition is checked at node/mod.rs->actual_round()
    sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.a(VALIDATOR_2),
        &make_request_propose_from_precommit(&sandbox, &precommit_2),
    );
    sandbox.send(
        sandbox.a(VALIDATOR_2),
        &make_request_prevote_from_precommit(&sandbox, &precommit_2),
    );

    //here consensus.rs->handle_majority_precommits()->//Commit is achieved
    sandbox.recv(&precommit_3);
    sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.a(VALIDATOR_3),
        &make_request_propose_from_precommit(&sandbox, &precommit_3),
    );
    sandbox.send(
        sandbox.a(VALIDATOR_3),
        &make_request_prevote_from_precommit(&sandbox, &precommit_3),
    );

    sandbox.assert_state(HEIGHT_ONE, ROUND_ONE);

    // !! if this tx would be received, commit would occur and last assert will require height one
    sandbox.recv(&tx);
    sandbox.recv(&propose);
    sandbox.broadcast(&Prevote::new(
        VALIDATOR_0,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        LOCK_ZERO,
        sandbox.s(VALIDATOR_0),
    ));
    sandbox.check_broadcast_status(HEIGHT_TWO, &block.hash());

    sandbox.add_time(Duration::from_millis(0));
    sandbox.assert_state(HEIGHT_TWO, ROUND_ONE);
}

/// scenario: // HANDLE PRECOMMIT
///         - purpose of the test is to check `add_unknown_propose_with_precommits()`
///         - scenario:
///             - get 3 precommits (!! with block with wrong state hash) => majority precommits
///               are observed => `add_unknown_propose_with_precommits()` is called
///             - then receive valid tx and Propose in order to call `handle_full_propose()` =>
///               fall with "Full propose: wrong state hash"
///         - it appeared that this test is almost the same as
///         `handle_precommit_positive_scenario_commit` the only difference that is in
///         `handle_precommit_positive_scenario_commit` propose and tx are received after second
///         precommit and here propose and tx are received after third precommit
#[test]
#[should_panic(expected = "Full propose: wrong state hash")]
fn handle_full_propose_wrong_state_hash() {
    let sandbox = timestamping_sandbox();

    // option: with transaction
    let tx = gen_timestamping_tx();

    let propose = ProposeBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .with_tx_hashes(&[tx.hash()]) //ordinary propose, but with this unreceived tx
        .build();

    // precommits with this block will be received
    let block = BlockBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .with_tx_hash(&tx.hash())
        .with_state_hash(&empty_hash())
        .build();

    let precommit_1 = Precommit::new(
        VALIDATOR_1,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_1),
    );
    let precommit_2 = Precommit::new(
        VALIDATOR_2,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_2),
    );
    let precommit_3 = Precommit::new(
        VALIDATOR_3,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_3),
    );

    sandbox.recv(&precommit_1);
    sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.a(VALIDATOR_1),
        &make_request_propose_from_precommit(&sandbox, &precommit_1),
    );
    sandbox.send(
        sandbox.a(VALIDATOR_1),
        &make_request_prevote_from_precommit(&sandbox, &precommit_1),
    );

    sandbox.recv(&precommit_2);
    // Second addition is required in order to make sandbox time >= propose time because
    // this condition is checked at node/mod.rs->actual_round()
    sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.a(VALIDATOR_2),
        &make_request_propose_from_precommit(&sandbox, &precommit_2),
    );
    sandbox.send(
        sandbox.a(VALIDATOR_2),
        &make_request_prevote_from_precommit(&sandbox, &precommit_2),
    );

    // Here consensus.rs->handle_majority_precommits()->//Commit is achieved
    sandbox.recv(&precommit_3);
    sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.a(VALIDATOR_3),
        &make_request_propose_from_precommit(&sandbox, &precommit_3),
    );
    sandbox.send(
        sandbox.a(VALIDATOR_3),
        &make_request_prevote_from_precommit(&sandbox, &precommit_3),
    );

    sandbox.assert_state(HEIGHT_ONE, ROUND_ONE);
    //    let tmp_propose = ProposeBuilder::new(&sandbox)
    //        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
    //        .build();
    // !! if this tx would be received, commit would occur and last assert will require height one
    sandbox.recv(&tx);
    sandbox.recv(&propose);
    sandbox.broadcast(&Prevote::new(
        VALIDATOR_0,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        LOCK_ZERO,
        sandbox.s(VALIDATOR_0),
    ));

    sandbox.add_time(Duration::from_millis(0));
    sandbox.assert_state(HEIGHT_TWO, ROUND_ONE);
}

/// - scenario: do not send precommit if have incompatible prevotes
#[test]
fn do_not_send_precommit_if_has_incompatible_prevotes() {
    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();

    let propose = ProposeBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .build();

    sandbox.recv(&propose);
    sandbox.broadcast(&Prevote::new(
        VALIDATOR_0,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        LOCK_ZERO,
        sandbox.s(VALIDATOR_0),
    ));

    sandbox.recv(&Prevote::new(
        VALIDATOR_1,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        LOCK_ZERO,
        sandbox.s(VALIDATOR_1),
    ));
    sandbox.assert_lock(LOCK_ZERO, None); //do not lock if <2/3 prevotes
    add_round_with_transactions(&sandbox, &sandbox_state, &[]);

    let future_propose = ProposeBuilder::new(&sandbox)
        .with_validator(VALIDATOR_3)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .with_round(ROUND_TWO)
        .build();
    sandbox.recv(&future_propose);
    sandbox.broadcast(&Prevote::new(
        VALIDATOR_0,
        HEIGHT_ONE,
        ROUND_TWO,
        &future_propose.hash(),
        LOCK_ZERO,
        sandbox.s(VALIDATOR_0),
    ));

    sandbox.recv(&Prevote::new(
        VALIDATOR_3,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        LOCK_ZERO,
        sandbox.s(VALIDATOR_3),
    ));
    sandbox.assert_lock(LOCK_ONE, Some(propose.hash())); //only if round > locked round

    // !! lock is obtained, but broadcast(Precommit is absent
    //    sandbox.broadcast(&Precommit::new(VALIDATOR_0, HEIGHT_ZERO, ROUND_ONE, &propose.hash(),
    //          &block.hash(), sandbox.s(VALIDATOR_0)));
    sandbox.assert_lock(LOCK_ONE, Some(propose.hash()));
    sandbox.add_time(Duration::from_millis(0));
}

/// scenario: // COMMIT:
/// // - handle queued messages
/// idea:
/// - same as positive scenario, but
///     - start from 1 height
///     - one precommit get from 0 round and queue it
/// - code is based on `handle_precommit_positive_scenario_commit()`
/// with following updates:
///     - use manually created tx because we need to know which tx will be used
///       in `add_one_height()` function
///         - take into account that in `add_one_height()` tx will be generated
///         and in `add_one_height_with_transaction` tx is taken as param
///     - predict & calculate blocks which would be created in
///       `handle_precommit_positive_scenario_commit()` on zero and one heights
///     - if we know block from 1st height we can construct valid precommit for 1st height and
///       receive it earlier: on zero height.
///     this early precommit will be queued and will be used after 1st height will be achieved
#[test]
fn handle_precommit_positive_scenario_commit_with_queued_precommit() {
    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();

    // create some tx
    let tx = gen_timestamping_tx();

    // Precommits with this block will be received during get 1st height in
    // fn add_one_height_with_transaction()
    let first_block = BlockBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(2 * sandbox.round_timeout() + PROPOSE_TIMEOUT + 1)
        .with_proposer_id(VALIDATOR_0)
        .with_tx_hash(&tx.hash())
        .with_state_hash(&sandbox.compute_state_hash(&[tx.raw().clone()]))
        .build();

    // this propose will be used during second commit
    let height_one_propose = ProposeBuilder::new(&sandbox)
        .with_validator(VALIDATOR_3)
        .with_height(HEIGHT_TWO)
        .with_duration_since_sandbox_time(2 * PROPOSE_TIMEOUT + 2 * sandbox.round_timeout() + 1)
        .with_prev_hash(&first_block.hash())
        .build();

    // this block will be created during second commit while manually creating precommits
    let second_block = BlockBuilder::new(&sandbox)
        .with_proposer_id(VALIDATOR_3)
        .with_height(HEIGHT_TWO)
        .with_duration_since_sandbox_time(2 * PROPOSE_TIMEOUT + 2 * sandbox.round_timeout() + 1)
        .with_prev_hash(&first_block.hash())
        .with_state_hash(&sandbox.compute_state_hash(&[tx.raw().clone()]))
        .build();

    let precommit_1 = Precommit::new(
        VALIDATOR_1,
        HEIGHT_TWO,
        ROUND_ONE,
        &height_one_propose.hash(),
        &second_block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_1),
    );
    let precommit_2 = Precommit::new(
        VALIDATOR_2,
        HEIGHT_TWO,
        ROUND_ONE,
        &height_one_propose.hash(),
        &second_block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_2),
    );
    let precommit_3 = Precommit::new(
        VALIDATOR_3,
        HEIGHT_TWO,
        ROUND_ONE,
        &height_one_propose.hash(),
        &second_block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_3),
    );

    sandbox.recv(&precommit_1); //early precommit from future height

    sandbox.assert_state(HEIGHT_ONE, ROUND_ONE);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[tx.raw().clone()]);
    sandbox.assert_state(HEIGHT_TWO, ROUND_ONE);
    assert_eq!(first_block.hash(), sandbox.last_hash());

    //this precommit is received at previous height and queued
    //    sandbox.recv(&precommit_1);
    sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.a(VALIDATOR_1),
        &make_request_propose_from_precommit(&sandbox, &precommit_1),
    );
    sandbox.send(
        sandbox.a(VALIDATOR_1),
        &make_request_prevote_from_precommit(&sandbox, &precommit_1),
    );

    sandbox.recv(&precommit_2);
    // second addition is required in order to make sandbox time >= propose time because
    // this condition is checked at node/mod.rs->actual_round()
    sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.a(VALIDATOR_2),
        &make_request_propose_from_precommit(&sandbox, &precommit_2),
    );
    sandbox.send(
        sandbox.a(VALIDATOR_2),
        &make_request_prevote_from_precommit(&sandbox, &precommit_2),
    );

    sandbox.recv(&height_one_propose);
    sandbox.broadcast(&Prevote::new(
        VALIDATOR_0,
        HEIGHT_TWO,
        ROUND_ONE,
        &height_one_propose.hash(),
        LOCK_ZERO,
        sandbox.s(VALIDATOR_0),
    ));

    sandbox.assert_state(HEIGHT_TWO, ROUND_ONE);
    // Here consensus.rs->handle_majority_precommits()->//Commit is achieved
    sandbox.recv(&precommit_3);
    sandbox.assert_state(HEIGHT_THREE, ROUND_ONE);
    sandbox.check_broadcast_status(HEIGHT_THREE, &second_block.hash());
    sandbox.add_time(Duration::from_millis(0));

    // update blockchain with new block
    // using feature that sandbox.last_block() is taken from blockchain
    assert_eq!(second_block, sandbox.last_block());
}

/// scenario: // HANDLE PRECOMMIT positive scenario with commit, leader and round timeout
/// idea is to make 0 node a leader during last commit
/// to make 0 node leader at last commit need to increment height 2 times before main scenario
/// and after last commit round will be 1, height will become 3 and node 0 will be leader
#[test]
fn commit_as_leader_send_propose_round_timeout() {
    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();

    // option: with transaction
    let tx = gen_timestamping_tx();

    // here need to make height = 2 because later one more height will be added and
    // node 0 will be leader at 1st round at 3th height
    // if height will be another, then test will fail on last lines because of
    // absent propose and prevote
    add_one_height(&sandbox, &sandbox_state);
    {
        // make node 0 a leader
        // here round 1 is just started
        sandbox.assert_state(HEIGHT_TWO, ROUND_ONE);
        {
            assert_eq!(*sandbox_state.time_millis_since_round_start.borrow(), 0);
        }
        // assert!(sandbox.is_leader());
    }
    let current_round = sandbox.current_round();
    let current_height = sandbox.current_height();

    // this propose will be a valid one when 0 node will become a leader after last commit
    let propose = ProposeBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .with_tx_hashes(&[tx.hash()]) //ordinary propose, but with this unreceived tx
        .build();

    // precommits with this block would be received if transaction will be received
    let block = BlockBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .with_prev_hash(&sandbox_state.accepted_block_hash.borrow())
        .with_tx_hash(&tx.hash())
        .with_state_hash(&sandbox.compute_state_hash(&[tx.raw().clone()]))
        .build();

    let precommit_1 = Precommit::new(
        VALIDATOR_1,
        current_height,
        current_round,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_1),
    );
    let precommit_2 = Precommit::new(
        VALIDATOR_2,
        current_height,
        current_round,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_2),
    );
    let precommit_3 = Precommit::new(
        VALIDATOR_3,
        current_height,
        current_round,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_3),
    );

    sandbox.recv(&precommit_1);
    sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.a(VALIDATOR_1),
        &make_request_propose_from_precommit(&sandbox, &precommit_1),
    );
    sandbox.send(
        sandbox.a(VALIDATOR_1),
        &make_request_prevote_from_precommit(&sandbox, &precommit_1),
    );

    sandbox.recv(&precommit_2);
    // second addition is required in order to make sandbox time >= propose time because
    // this condition is checked at node/mod.rs->actual_round()
    sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.a(VALIDATOR_2),
        &make_request_propose_from_precommit(&sandbox, &precommit_2),
    );
    sandbox.send(
        sandbox.a(VALIDATOR_2),
        &make_request_prevote_from_precommit(&sandbox, &precommit_2),
    );

    {
        // get propose and appropriate transaction
        sandbox.recv(&propose);
        sandbox.recv(&tx);
        sandbox.broadcast(&make_prevote_from_propose(&sandbox, &propose));
    }
    // verify that transaction is received
    assert_eq!(vec![tx.hash()], sandbox.transactions_hashes());

    // receive precommit 3 and start commit process
    sandbox.assert_state(current_height, current_round);
    // Here consensus.rs->handle_majority_precommits()->//Commit is achieved
    sandbox.recv(&precommit_3);

    let new_height = current_height.next();
    sandbox.assert_state(new_height, ROUND_ONE);
    sandbox.check_broadcast_status(new_height, &block.hash());

    let propose = ProposeBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        //        .with_tx_hashes(&[tx.hash()]) //ordinary propose, but with this unreceived tx
        .build();

    sandbox.add_time(Duration::from_millis(PROPOSE_TIMEOUT));
    sandbox.broadcast(&propose);
    sandbox.broadcast(&make_prevote_from_propose(&sandbox, &propose));
    sandbox.add_time(Duration::from_millis(
        sandbox.round_timeout() - PROPOSE_TIMEOUT,
    ));
    sandbox.assert_state(sandbox.current_height(), ROUND_TWO);
}

/// HANDLE TX

/// - if get full propose:
///     - all in handle full propose
/// idea of test is:
/// - to receive propose with unknown tx
/// - receive that tx, so, all required txs are present
/// - call `node/consensus.rs->handle_full_propose()` => broadcast prevote
#[test]
fn handle_tx_handle_full_propose() {
    let sandbox = timestamping_sandbox();

    // option: with transaction
    let tx = gen_timestamping_tx();

    let propose = ProposeBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .with_tx_hashes(&[tx.hash()]) //ordinary propose, but with this unreceived tx
        .build();

    sandbox.recv(&propose);
    sandbox.add_time(Duration::from_millis(TRANSACTIONS_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.a(VALIDATOR_2),
        &TransactionsRequest::new(
            &sandbox.p(VALIDATOR_0),
            &sandbox.p(VALIDATOR_2),
            &[tx.hash()],
            sandbox.s(VALIDATOR_0),
        ),
    );

    // !! here handle_tx()->handle_full_propose() is called => broadcast(Prevote) is observed
    sandbox.recv(&tx);

    sandbox.broadcast(&make_prevote_from_propose(&sandbox, &propose));

    sandbox.add_time(Duration::from_millis(0));
}

/// HANDLE block response

/// - should process block even if tx in pool
/// idea of test is:
/// - receive some tx A
/// - getting Status from other node with later height, send BlockRequest to this node
/// - receive BlockResponse with already known tx A
/// - Block should be executed and committed
#[test]
fn handle_block_response_tx_in_pool() {
    let sandbox = timestamping_sandbox();

    let tx = gen_timestamping_tx();

    let propose = ProposeBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .build();

    let block = BlockBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .with_tx_hash(&tx.hash())
        .with_state_hash(&sandbox.compute_state_hash(&[tx.raw().clone()]))
        .build();

    let precommit_1 = Precommit::new(
        VALIDATOR_1,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_1),
    );
    let precommit_2 = Precommit::new(
        VALIDATOR_2,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_2),
    );
    let precommit_3 = Precommit::new(
        VALIDATOR_3,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_3),
    );

    sandbox.recv(&Status::new(
        &sandbox.p(VALIDATOR_3),
        HEIGHT_TWO,
        &block.hash(),
        sandbox.s(VALIDATOR_3),
    ));

    sandbox.add_time(Duration::from_millis(BLOCK_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.a(VALIDATOR_3),
        &BlockRequest::new(
            &sandbox.p(VALIDATOR_0),
            &sandbox.p(VALIDATOR_3),
            HEIGHT_ONE,
            sandbox.s(VALIDATOR_0),
        ),
    );
    sandbox.recv(&tx);

    sandbox.recv(&BlockResponse::new(
        &sandbox.p(VALIDATOR_3),
        &sandbox.p(VALIDATOR_0),
        block.clone(),
        vec![precommit_1, precommit_2, precommit_3],
        &[tx.hash()],
        sandbox.s(VALIDATOR_3),
    ));

    sandbox.assert_state(HEIGHT_TWO, ROUND_ONE);
    sandbox.broadcast(&Status::new(
        &sandbox.p(VALIDATOR_0),
        HEIGHT_TWO,
        &block.hash(),
        sandbox.s(VALIDATOR_0),
    ));
}

#[test]
fn handle_block_response_with_unknown_tx() {
    let sandbox = timestamping_sandbox();

    let tx = gen_timestamping_tx();

    let propose = ProposeBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .build();

    let block = BlockBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .with_tx_hash(&tx.hash())
        .with_state_hash(&sandbox.compute_state_hash(&[tx.raw().clone()]))
        .build();

    let precommit_1 = Precommit::new(
        VALIDATOR_1,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_1),
    );
    let precommit_2 = Precommit::new(
        VALIDATOR_2,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_2),
    );
    let precommit_3 = Precommit::new(
        VALIDATOR_3,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_3),
    );

    sandbox.recv(&Status::new(
        &sandbox.p(VALIDATOR_3),
        HEIGHT_TWO,
        &block.hash(),
        sandbox.s(VALIDATOR_3),
    ));

    sandbox.add_time(Duration::from_millis(BLOCK_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.a(VALIDATOR_3),
        &BlockRequest::new(
            &sandbox.p(VALIDATOR_0),
            &sandbox.p(VALIDATOR_3),
            HEIGHT_ONE,
            sandbox.s(VALIDATOR_0),
        ),
    );

    sandbox.recv(&BlockResponse::new(
        &sandbox.p(VALIDATOR_3),
        &sandbox.p(VALIDATOR_0),
        block.clone(),
        vec![precommit_1, precommit_2, precommit_3],
        &[tx.hash()],
        sandbox.s(VALIDATOR_3),
    ));

    sandbox.add_time(Duration::from_millis(TRANSACTIONS_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.a(VALIDATOR_3),
        &TransactionsRequest::new(
            &sandbox.p(VALIDATOR_0),
            &sandbox.p(VALIDATOR_3),
            &[tx.hash()],
            sandbox.s(VALIDATOR_0),
        ),
    );

    sandbox.recv(&TransactionsResponse::new(
        &sandbox.p(VALIDATOR_3),
        &sandbox.p(VALIDATOR_0),
        vec![tx.raw().clone()],
        sandbox.s(VALIDATOR_3),
    ));

    sandbox.assert_state(HEIGHT_TWO, ROUND_ONE);
    sandbox.broadcast(&Status::new(
        &sandbox.p(VALIDATOR_0),
        HEIGHT_TWO,
        &block.hash(),
        sandbox.s(VALIDATOR_0),
    ));
}

#[test]
fn handle_block_response_with_invalid_txs_order() {
    let sandbox = timestamping_sandbox();

    let tx1 = gen_timestamping_tx();
    let tx2 = gen_timestamping_tx();

    let propose = ProposeBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .build();

    let block = BlockBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .with_txs_hashes(&[tx1.hash(), tx2.hash()])
        .with_state_hash(&sandbox.compute_state_hash(&[tx1.raw().clone(), tx2.raw().clone()]))
        .build();

    let precommit_1 = Precommit::new(
        VALIDATOR_1,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_1),
    );
    let precommit_2 = Precommit::new(
        VALIDATOR_2,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_2),
    );
    let precommit_3 = Precommit::new(
        VALIDATOR_3,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_3),
    );

    sandbox.recv(&Status::new(
        &sandbox.p(VALIDATOR_3),
        HEIGHT_TWO,
        &block.hash(),
        sandbox.s(VALIDATOR_3),
    ));

    sandbox.add_time(Duration::from_millis(BLOCK_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.a(VALIDATOR_3),
        &BlockRequest::new(
            &sandbox.p(VALIDATOR_0),
            &sandbox.p(VALIDATOR_3),
            HEIGHT_ONE,
            sandbox.s(VALIDATOR_0),
        ),
    );

    // Invalid transactions order.
    sandbox.recv(&BlockResponse::new(
        &sandbox.p(VALIDATOR_3),
        &sandbox.p(VALIDATOR_0),
        block.clone(),
        vec![precommit_1, precommit_2, precommit_3],
        &[tx2.hash(), tx1.hash()],
        sandbox.s(VALIDATOR_3),
    ));

    sandbox.assert_state(HEIGHT_ONE, ROUND_ONE);
}

#[test]
fn handle_block_response_with_invalid_precommits() {
    let sandbox = timestamping_sandbox();

    let tx = gen_timestamping_tx();

    let propose = ProposeBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .build();

    let block1 = BlockBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .with_tx_hash(&tx.hash())
        .with_state_hash(&sandbox.compute_state_hash(&[tx.raw().clone()]))
        .build();

    let block2 = BlockBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .build();

    let precommit_1 = Precommit::new(
        VALIDATOR_1,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block1.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_1),
    );
    let precommit_2 = Precommit::new(
        VALIDATOR_2,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block1.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_2),
    );
    // Precommit with invalid block hash.
    let precommit_for_other_block = Precommit::new(
        VALIDATOR_3,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block2.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_3),
    );

    sandbox.recv(&Status::new(
        &sandbox.p(VALIDATOR_3),
        HEIGHT_TWO,
        &block1.hash(),
        sandbox.s(VALIDATOR_3),
    ));

    sandbox.add_time(Duration::from_millis(BLOCK_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.a(VALIDATOR_3),
        &BlockRequest::new(
            &sandbox.p(VALIDATOR_0),
            &sandbox.p(VALIDATOR_3),
            HEIGHT_ONE,
            sandbox.s(VALIDATOR_0),
        ),
    );
    sandbox.recv(&tx);

    sandbox.recv(&BlockResponse::new(
        &sandbox.p(VALIDATOR_3),
        &sandbox.p(VALIDATOR_0),
        block1.clone(),
        vec![precommit_1, precommit_2, precommit_for_other_block],
        &[tx.hash()],
        sandbox.s(VALIDATOR_3),
    ));

    sandbox.assert_state(HEIGHT_ONE, ROUND_ONE);
}

// - ignore existed transaction (in both blockchain and pool)
/// - idea of test is to receive propose with unknown tx
/// - receive that tx
/// - broadcast prevote
#[test]
fn broadcast_prevote_with_tx_positive() {
    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();

    add_one_height(&sandbox, &sandbox_state);
    sandbox.assert_state(HEIGHT_TWO, ROUND_ONE);

    // option: with transaction
    let tx = gen_timestamping_tx();
    sandbox.recv(&tx);

    let propose = ProposeBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .with_tx_hashes(&[tx.hash()]) //ordinary propose, but with this received tx
        .build();
    sandbox.recv(&propose);

    // check scenario:
    // HANDLE FULL PROPOSE
    // - send prevote when lock=0 for known propose
    sandbox.assert_lock(LOCK_ZERO, None);
    sandbox.broadcast(&Prevote::new(
        VALIDATOR_0,
        HEIGHT_TWO,
        ROUND_ONE,
        &propose.hash(),
        LOCK_ZERO,
        sandbox.s(VALIDATOR_0),
    ));
}

// - ignore existed transaction (in both pool)
/// - idea of test is to receive propose with unknown tx
/// - receive that tx
/// - broadcast prevote
#[test]
fn handle_tx_ignore_existing_tx_in_blockchain() {
    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();

    // option: with transaction
    let tx = gen_timestamping_tx();

    add_one_height_with_transactions(&sandbox, &sandbox_state, &[tx.raw().clone()]);
    sandbox.assert_state(HEIGHT_TWO, ROUND_ONE);

    // add rounds & become leader
    sandbox.add_time(Duration::from_millis(sandbox.round_timeout()));
    assert!(sandbox.is_leader());

    sandbox.recv(&tx);

    sandbox.add_time(Duration::from_millis(PROPOSE_TIMEOUT));
    let propose = ProposeBuilder::new(&sandbox)
        //.with_tx_hashes(&[tx.hash()]) //ordinary propose, but with this received tx
        // !! note that here no tx are expected whereas old tx is received earlier
        .with_tx_hashes(&[])
        .build();
    sandbox.broadcast(&propose);
    sandbox.broadcast(&make_prevote_from_propose(&sandbox, &propose));
    sandbox.add_time(Duration::from_millis(0));
}

/// HANDLE ROUND TIMEOUT:
/// - Ignore if height and round are not the same
/// scenario:
///  - make commit at first round
///  - and verify that at moment when first `round_timeout` is triggered, round remains the same
#[test]
fn handle_round_timeout_ignore_if_height_and_round_are_not_the_same() {
    let sandbox = timestamping_sandbox();

    // option: with transaction
    let tx = gen_timestamping_tx();

    let propose = ProposeBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .with_tx_hashes(&[tx.hash()]) //ordinary propose, but with this unreceived tx
        .build();

    // this block with transactions should be in real
    let block = BlockBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .with_tx_hash(&tx.hash())
        .with_state_hash(&sandbox.compute_state_hash(&[tx.raw().clone()]))
        .build();

    let precommit_1 = Precommit::new(
        VALIDATOR_1,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_1),
    );
    let precommit_2 = Precommit::new(
        VALIDATOR_2,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_2),
    );
    let precommit_3 = Precommit::new(
        VALIDATOR_3,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_3),
    );

    sandbox.recv(&precommit_1);
    sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.a(VALIDATOR_1),
        &make_request_propose_from_precommit(&sandbox, &precommit_1),
    );
    sandbox.send(
        sandbox.a(VALIDATOR_1),
        &make_request_prevote_from_precommit(&sandbox, &precommit_1),
    );

    sandbox.recv(&precommit_2);
    // second addition is required in order to make sandbox time >= propose time because
    // this condition is checked at node/mod.rs->actual_round()
    sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.a(VALIDATOR_2),
        &make_request_propose_from_precommit(&sandbox, &precommit_2),
    );
    sandbox.send(
        sandbox.a(VALIDATOR_2),
        &make_request_prevote_from_precommit(&sandbox, &precommit_2),
    );
    sandbox.recv(&propose);
    sandbox.recv(&tx);
    sandbox.broadcast(&make_prevote_from_propose(&sandbox, &propose));

    sandbox.assert_state(HEIGHT_ONE, ROUND_ONE);
    // Here consensus.rs->handle_majority_precommits()->//Commit is achieved
    sandbox.recv(&precommit_3);
    sandbox.assert_state(HEIGHT_TWO, ROUND_ONE);
    sandbox.check_broadcast_status(HEIGHT_TWO, &block.hash());
    sandbox.add_time(Duration::from_millis(0));

    sandbox.add_time(Duration::from_millis(
        sandbox.round_timeout() - 2 * PROPOSE_REQUEST_TIMEOUT,
    ));
    // This assert would fail if check for same height is absent in
    // node/consensus.rs->handle_round_timeout()
    sandbox.assert_state(HEIGHT_TWO, ROUND_ONE);
}

/// HANDLE ROUND TIMEOUT:
// - add new round timeout
#[test]
fn handle_round_timeout_increment_round_add_new_round_timeout() {
    let sandbox = timestamping_sandbox();

    sandbox.add_time(Duration::from_millis(sandbox.round_timeout() - 1));
    sandbox.assert_state(HEIGHT_ONE, ROUND_ONE);
    sandbox.add_time(Duration::from_millis(1));
    sandbox.assert_state(HEIGHT_ONE, ROUND_TWO);

    // next round timeout is added
    sandbox.add_time(Duration::from_millis(sandbox.round_timeout() - 1));
    sandbox.assert_state(HEIGHT_ONE, ROUND_TWO);
    sandbox.add_time(Duration::from_millis(1));
    sandbox.assert_state(HEIGHT_ONE, ROUND_THREE);
    sandbox.add_time(Duration::from_millis(0));
}

/// idea of the scenario is to become leader
/// then:
///  - propose timeout is added
///   - when propose timeout is triggered - propose is send
#[test]
fn test_send_propose_and_prevote_when_we_are_leader() {
    let sandbox = timestamping_sandbox();

    // round happens
    sandbox.add_time(Duration::from_millis(sandbox.round_timeout()));
    sandbox.add_time(Duration::from_millis(
        sandbox.round_timeout() + PROPOSE_TIMEOUT,
    ));

    sandbox.assert_state(HEIGHT_ONE, ROUND_THREE);

    // ok, we are leader
    let propose = ProposeBuilder::new(&sandbox).build();

    sandbox.broadcast(&propose);
    sandbox.broadcast(&make_prevote_from_propose(&sandbox, &propose));
    sandbox.add_time(Duration::from_millis(0));
}

/// HANDLE ROUND TIMEOUT:
/// - send prevote if locked to propose
/// idea:
///  - lock to propose
///  - trigger `round_timeout`
///  - observe broadcasted prevote
#[test]
fn handle_round_timeout_send_prevote_if_locked_to_propose() {
    // fn test_get_lock_and_send_precommit() {
    let sandbox = timestamping_sandbox();

    let propose = ProposeBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .build();

    let block = BlockBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .build();

    sandbox.recv(&propose);
    sandbox.broadcast(&Prevote::new(
        VALIDATOR_0,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        LOCK_ZERO,
        sandbox.s(VALIDATOR_0),
    ));

    sandbox.recv(&Prevote::new(
        VALIDATOR_1,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        LOCK_ZERO,
        sandbox.s(VALIDATOR_1),
    ));
    sandbox.assert_lock(LOCK_ZERO, None); //do not lock if <2/3 prevotes

    sandbox.recv(&Prevote::new(
        VALIDATOR_2,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        LOCK_ZERO,
        sandbox.s(VALIDATOR_2),
    ));
    sandbox.assert_lock(LOCK_ONE, Some(propose.hash())); //only if round > locked round

    sandbox.broadcast(&Precommit::new(
        VALIDATOR_0,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_0),
    ));
    sandbox.assert_lock(LOCK_ONE, Some(propose.hash()));
    sandbox.add_time(Duration::from_millis(0));

    // trigger round_timeout
    sandbox.add_time(Duration::from_millis(sandbox.round_timeout()));
    //    sandbox.broadcast(&make_prevote_from_propose(&sandbox, &propose));
    sandbox.broadcast(&Prevote::new(
        VALIDATOR_0,
        HEIGHT_ONE,
        ROUND_TWO,
        &propose.hash(),
        LOCK_ONE,
        sandbox.s(VALIDATOR_0),
    ));
    sandbox.add_time(Duration::from_millis(0));
}

/// HANDLE ROUND TIMEOUT:
///  - handle queued messages
/// idea:
///  - lock to propose
///  - trigger `round_timeout`
///  - observe broadcasted prevote
#[test]
#[should_panic(expected = "Send unexpected message Request(ProposeRequest")]
fn test_handle_round_timeout_queue_prevote_message_from_next_round() {
    let sandbox = timestamping_sandbox();

    sandbox.recv(&Prevote::new(
        VALIDATOR_2,
        HEIGHT_ONE,
        ROUND_TWO,
        &empty_hash(),
        Round::zero(),
        sandbox.s(VALIDATOR_2),
    ));

    // trigger round_timeout
    sandbox.add_time(Duration::from_millis(sandbox.round_timeout()));
    // trigger request_propose_timeout
    sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
    // observe requestPropose request
    sandbox.add_time(Duration::from_millis(0));
}

/// - exclude validator from consensus
/// - idea of test is to exclude sandbox validator from consensus
/// - node continues as `full node`
#[test]
fn test_exclude_validator_from_consensus() {
    use storage::StorageValue;

    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();

    add_one_height(&sandbox, &sandbox_state);

    let tx_cfg = {
        let mut consensus_cfg = sandbox.cfg();
        consensus_cfg.validator_keys.swap_remove(0);
        consensus_cfg.actual_from = sandbox.current_height().next().next();
        consensus_cfg.previous_cfg_hash = sandbox.cfg().hash();

        TxConfig::new(
            &sandbox.p(VALIDATOR_0),
            &consensus_cfg.clone().into_bytes(),
            consensus_cfg.actual_from,
            sandbox.s(VALIDATOR_0),
        )
    };

    add_one_height_with_transactions(&sandbox, &sandbox_state, &[tx_cfg.raw().clone()]);
    add_one_height(&sandbox, &sandbox_state);
    // node loses validator status
    add_one_height_with_transactions_from_other_validator(&sandbox, &sandbox_state, &[]);
}

/// - Check `following_configuration` method in schema
/// - idea of the test is check configurations method from schema
#[test]
fn test_schema_config_changes() {
    use storage::StorageValue;

    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();

    add_one_height(&sandbox, &sandbox_state);

    let (tx_cfg, following_cfg) = {
        let mut consensus_cfg = sandbox.cfg();
        consensus_cfg.consensus.txs_block_limit = 2000;
        consensus_cfg.actual_from = sandbox.current_height().next().next();
        consensus_cfg.previous_cfg_hash = sandbox.cfg().hash();

        let tx = TxConfig::new(
            &sandbox.p(VALIDATOR_0),
            &consensus_cfg.clone().into_bytes(),
            consensus_cfg.actual_from,
            sandbox.s(VALIDATOR_0),
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
        Schema::new(&sandbox.blockchain_ref().snapshot()).configuration_by_height(HEIGHT_FOUR),
        prev_cfg
    );
    // Commit a new configuration
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[tx_cfg.raw().clone()]);
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
        Schema::new(&sandbox.blockchain_ref().snapshot()).configuration_by_height(HEIGHT_ZERO),
        prev_cfg
    );
    assert_eq!(
        Schema::new(&sandbox.blockchain_ref().snapshot())
            .configuration_by_height(sandbox.current_height()),
        following_cfg
    );
}

// - lock to propose when get +2/3 prevote
//     - only if propose is known     - covered in request_propose_when_get_prevote()
//     - only if all txs is known     - covered in request_txs_when_get_propose_or_prevote()
//     - only if round > locked round - covered in lock_to_propose_when_get_2_3_prevote_positive()

// LOCK

// - Send precommit when get lock   //covered in lock_to_propose_when_get_2_3_prevote_positive()
// - if haven’t incompatible prevotes
//     - if has +2/3 precommits
//         TODO: This idea is unreachable because there are no any places in the code
//         where RequestPrecommit is added. (ECR-1627)
//         - remove precommit request
//         - COMMIT //covered in test_reach_one_height
//         - We are fucked up   //covered in handle_full_propose_we_are_fucked_up()
//         - not send prevotes after commit     //covered in lock_not_send_prevotes_after_commit()
// - Send prevote       //covered in lock_to_propose_and_send_prevote()
//     - round > locked + 1     //covered in lock_to_propose_when_get_2_3_prevote_positive
//     - round < current_round  //covered in lock_to_past_round_broadcast_prevote()
//     - not send prevote yet
//     - if get +2/3 prevotes
//         - remove prevote request //covered in lock_remove_request_prevotes
//         - lock to propose        //covered in lock_to_propose_when_get_2_3_prevote_positive

// HANDLE PRECOMMIT

// - Request propose if unknown //covered in commit_using_unknown_propose_with_precommits()
// - Request txs if has unknown //covered in request_txs_when_get_propose_or_prevote()
// - Request prevotes           //covered in commit_using_unknown_propose_with_precommits()
//     - if msg.round > locked round    //covered in handle_precommit_remove_request_prevotes()
// - If has +2/3 precommit      //covered in handle_precommit_positive_scenario_commit()
//     TODO: This idea is unreachable because there are no any places in the code where
//     RequestPrecommit is added. (ECR-1627)
//     - remove precommit request
//     - COMMIT
//         - if propose is known    //covered in do_not_commit_if_propose_is_unknown()
//         - has all txs           //covered in do_not_commit_if_tx_is_unknown()
//         - Our block_hash different from precommits one
//           covered in handle_precommit_different_block_hash()
//     - add_unknown_propose_with_precommits
//       covered in commit_using_unknown_propose_with_precommits()

// COMMIT:

// - update height and lock //is checked by verifying height and lock in
//   handle_precommit_positive_scenario_commit()
// - handle queued messages
//   ?covered in handle_precommit_positive_scenario_commit_with_queued_precommit
// - update blockchain
//   covered in the end of handle_precommit_positive_scenario_commit_with_queued_precommit
// - send propose if is leader //covered in commit_as_leader_send_propose_round_timeout
// - add round timeout          //covered in commit_as_leader_send_propose_round_timeout
// - request commits            //commit message is absent at the moment

// HANDLE TX

// - ignore existed transaction (in both blockchain and pool) because with blockchain is covered
//   in handle_tx_ignore_existing_tx_in_blockchain(); case with pool is not covered because can't
//   create such scenario in terms of messages
// - verify signature   //covered in handle_tx_verify_signature
// - if get full propose:
//     - remove tx request      //covered in not_request_txs_when_get_tx_and_propose
//     - all in handle full propose    //covered in handle_tx_handle_full_propose()

// HANDLE ROUND TIMEOUT:

// - Ignore if height and round are not the same
//   covered in handle_round_timeout_ignore_if_height_and_round_are_not_the_same()
// - increment round
//   covered in handle_round_timeout_increment_round_add_new_round_timeout()
// - add new round timeout
//   covered in handle_round_timeout_increment_round_add_new_round_timeout()
// - send prevote if locked to propose
// - send propose if we are leader
//   covered in test_send_propose_and_prevote_when_we_are_leader()
// - handle queued messages
//   covered in test_handle_round_timeout_queue_prevote_message_from_next_round()

// HANDLE REQUEST TIMEOUT:

// TODO: Investigate how check this and come back when '// FIXME: check height?' is fixed. (ECR-1627)
// - check height?
// - Propose/Tx/Prevotes/Precommits/Commit  //looks like that all these are covered
//      send(RequestPropose):       test_queue_prevote_message_from_next_height
//      recv(RequestTransactions):  response_to_request_txs
// - if we have another known node:
//     - send new request message//for RequestTransaction is covered in handle_tx_handle_full_propose()
//     - add timeout             //for RequestTransaction is covered in handle_tx_handle_full_propose()

// TODO: Add scenario for single node network. (ECR-1627)
