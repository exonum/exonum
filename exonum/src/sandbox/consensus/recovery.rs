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
use helpers::{user_agent, Height, Round, ValidatorId};
use node;

use sandbox::{
    sandbox::{timestamping_sandbox, SandboxBuilder}, sandbox_tests_helper::*,
};

#[test]
fn test_disable_and_enable() {
    let mut sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();

    sandbox.assert_state(Height(1), Round(1));
    try_add_one_height(&sandbox, &sandbox_state).unwrap();
    sandbox.assert_state(Height(2), Round(1));

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
    sandbox.assert_state(Height(2), Round(1));
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
    sandbox.assert_state(Height(2), Round(1));

    // Reset the time.
    sandbox.set_time(time_saved);

    // The node should work fine now
    try_add_one_height(&sandbox, &sandbox_state).unwrap();
    sandbox.assert_state(Height(3), Round(1));
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
    sandbox.add_time(Duration::from_millis(sandbox.current_round_timeout()));
    sandbox.add_time(Duration::from_millis(
        sandbox.current_round_timeout() + PROPOSE_TIMEOUT,
    ));

    assert!(sandbox.is_leader());
    sandbox.assert_state(Height(1), Round(3));

    // ok, we are leader
    let propose = ProposeBuilder::new(&sandbox).build();

    let prevote = make_prevote_from_propose(&sandbox, &propose);
    sandbox.broadcast(&propose);
    sandbox.broadcast(&prevote);

    let current_height = sandbox.current_height();
    let current_round = sandbox.current_round();

    let sandbox_restarted = sandbox.restart();

    sandbox_restarted.broadcast(&prevote);
    sandbox_restarted.assert_lock(NOT_LOCKED, None);
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

    let propose = ProposeBuilder::new(&sandbox).build();

    let prevote = make_prevote_from_propose(&sandbox, &propose);

    let block = BlockBuilder::new(&sandbox).build();

    sandbox.recv(&propose);

    sandbox.broadcast(&prevote);

    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(1),
        Height(1),
        Round(1),
        &propose.hash(),
        NOT_LOCKED,
        sandbox.s(ValidatorId(1)),
    ));
    sandbox.assert_lock(NOT_LOCKED, None); // Do not lock if <2/3 prevotes

    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(2),
        Height(1),
        Round(1),
        &propose.hash(),
        NOT_LOCKED,
        sandbox.s(ValidatorId(2)),
    ));
    sandbox.assert_lock(Round(1), Some(propose.hash()));

    let precommit = sandbox.create_precommit(
        ValidatorId(0),
        Height(1),
        Round(1),
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(ValidatorId(0)),
    );
    sandbox.broadcast(&precommit);
    sandbox.assert_lock(Round(1), Some(propose.hash()));
    let current_height = sandbox.current_height();
    let current_round = sandbox.current_round();

    // Simulate node restart.
    let sandbox_restarted = sandbox.restart();

    // Assert that consensus messages were recovered and we're in locked state now.
    sandbox_restarted.assert_lock(Round(1), Some(propose.hash()));
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

    let propose = ProposeBuilder::new(&sandbox).build();

    let prevote = make_prevote_from_propose(&sandbox, &propose);

    let block = BlockBuilder::new(&sandbox).build();

    sandbox.recv(&propose);

    sandbox.broadcast(&prevote);

    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(1),
        Height(1),
        Round(1),
        &propose.hash(),
        NOT_LOCKED,
        sandbox.s(ValidatorId(1)),
    ));
    sandbox.assert_lock(NOT_LOCKED, None); //do not lock if <2/3 prevotes

    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(2),
        Height(1),
        Round(1),
        &propose.hash(),
        NOT_LOCKED,
        sandbox.s(ValidatorId(2)),
    ));
    sandbox.assert_lock(Round(1), Some(propose.hash()));

    let precommit = sandbox.create_precommit(
        ValidatorId(0),
        Height(1),
        Round(1),
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(ValidatorId(0)),
    );

    sandbox.broadcast(&precommit);

    let current_height = sandbox.current_height();
    let current_round = sandbox.current_round();

    // Simulate node restart
    let sandbox_restarted = sandbox.restart();

    // assert that consensus messages were recovered and we're in locked state now
    sandbox_restarted.assert_lock(Round(1), Some(propose.hash()));
    sandbox_restarted.assert_state(current_height, current_round);
    sandbox_restarted.broadcast(&prevote);
    sandbox_restarted.broadcast(&precommit);

    sandbox_restarted.recv(&sandbox_restarted.create_precommit(
        ValidatorId(1),
        Height(1),
        Round(1),
        &propose.hash(),
        &block.hash(),
        sandbox_restarted.time().into(),
        sandbox_restarted.s(ValidatorId(1)),
    ));

    sandbox_restarted.recv(&sandbox_restarted.create_precommit(
        ValidatorId(2),
        Height(1),
        Round(1),
        &propose.hash(),
        &block.hash(),
        sandbox_restarted.time().into(),
        sandbox_restarted.s(ValidatorId(2)),
    ));

    sandbox_restarted.assert_state(Height(2), Round(1));
    sandbox_restarted.check_broadcast_status(Height(2), &block.hash());
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

    let first_propose = ProposeBuilder::new(&sandbox).build();
    let first_prevote = make_prevote_from_propose(&sandbox, &first_propose);
    let block = BlockBuilder::new(&sandbox).build();

    sandbox.recv(&first_propose);

    sandbox.broadcast(&first_prevote);

    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(1),
        Height(1),
        Round(1),
        &first_propose.hash(),
        NOT_LOCKED,
        sandbox.s(ValidatorId(1)),
    ));
    sandbox.assert_lock(NOT_LOCKED, None); //do not lock if <2/3 prevotes

    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(2),
        Height(1),
        Round(1),
        &first_propose.hash(),
        NOT_LOCKED,
        sandbox.s(ValidatorId(2)),
    ));
    sandbox.assert_lock(Round(1), Some(first_propose.hash()));

    let first_precommit = sandbox.create_precommit(
        ValidatorId(0),
        Height(1),
        Round(1),
        &first_propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(ValidatorId(0)),
    );

    sandbox.broadcast(&first_precommit);

    sandbox.assert_state(Height(1), Round(1));
    sandbox.add_time(Duration::from_millis(sandbox.current_round_timeout()));
    sandbox.assert_state(Height(1), Round(2));

    // make sure we broadcasted same Prevote for second round
    let first_updated_prevote = sandbox.create_prevote(
        first_prevote.validator(),
        first_prevote.height(),
        Round(2),
        first_prevote.propose_hash(),
        Round(1),
        sandbox.s(ValidatorId(0)),
    );
    sandbox.broadcast(&first_updated_prevote);

    let second_propose = ProposeBuilder::new(&sandbox).build();
    let second_block = BlockBuilder::new(&sandbox).build();

    sandbox.recv(&second_propose);

    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(1),
        Height(1),
        Round(2),
        &second_propose.hash(),
        NOT_LOCKED,
        sandbox.s(ValidatorId(1)),
    ));

    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(2),
        Height(1),
        Round(2),
        &second_propose.hash(),
        NOT_LOCKED,
        sandbox.s(ValidatorId(2)),
    ));

    sandbox.assert_lock(Round(1), Some(first_propose.hash()));

    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(3),
        Height(1),
        Round(2),
        &second_propose.hash(),
        NOT_LOCKED,
        sandbox.s(ValidatorId(3)),
    ));

    sandbox.assert_lock(Round(2), Some(second_propose.hash()));

    let second_precommit = sandbox.create_precommit(
        ValidatorId(0),
        Height(1),
        Round(2),
        &second_propose.hash(),
        &second_block.hash(),
        sandbox.time().into(),
        sandbox.s(ValidatorId(0)),
    );
    sandbox.broadcast(&second_precommit);

    // Restart node.
    let saved_time = sandbox.time();
    let sandbox_new = sandbox.restart_with_time(saved_time);

    sandbox_new.assert_lock(Round(2), Some(second_propose.hash()));
    sandbox_new.assert_state(Height(1), Round(2));
    sandbox_new.broadcast(&first_prevote);

    let first_precommit_new_time = sandbox_new.create_precommit(
        first_precommit.validator(),
        first_precommit.height(),
        first_precommit.round(),
        first_precommit.propose_hash(),
        first_precommit.block_hash(),
        sandbox_new.time().into(),
        sandbox_new.s(ValidatorId(0)),
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
    let sandbox = SandboxBuilder::new()
        .do_not_initialize_connections()
        .build();

    let (v0, v1) = (ValidatorId(0), ValidatorId(1));
    let (p0, s0, a0) = (sandbox.p(v0), sandbox.s(v0).clone(), sandbox.a(v0));
    let (p1, s1, a1) = (sandbox.p(v1), sandbox.s(v1).clone(), sandbox.a(v1));

    let time = sandbox.time();
    let connect_from_0 = sandbox.create_connect(&p0, a0, time.into(), &user_agent::get(), &s0);
    let connect_from_1 = sandbox.create_connect(&p1, a1, time.into(), &user_agent::get(), &s1);
    let peers_request = sandbox.create_peers_request(&p1, &p0, &s1);

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
