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

//! Tests in this module are designed to test ability of the node to recover
//! state after restart/stop.

use std::time::Duration;

use crypto::CryptoHash;
use helpers::user_agent;
use messages::{Connect, PeersRequest, Precommit, Prevote};
use node;

use sandbox::{
    sandbox::{sandbox_with_services_uninitialized, timestamping_sandbox}, sandbox_tests_helper::*,
};

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
    // TODO: use try_add_one_height (ECR-1817)
    let result = try_add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
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
