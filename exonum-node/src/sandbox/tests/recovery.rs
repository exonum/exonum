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

//! Tests in this module are designed to test ability of the node to recover
//! state after restart/stop.

use exonum::{
    helpers::{user_agent, Height, Round, ValidatorId},
    merkledb::ObjectHash,
};

use std::time::Duration;

use crate::sandbox::{sandbox_tests_helper::*, timestamping_sandbox, Sandbox, SandboxBuilder};

/// Scenario:
/// - Node sends `Propose` and `Prevote`.
/// - Node restarts.
/// - Node should recover to previous state: jump into the round before stop and does not send
///   `Propose` again.
#[test]
fn should_not_send_propose_after_node_restart() {
    let sandbox = timestamping_sandbox();

    // Round happens.
    let round_timeout = sandbox.current_round_timeout();
    sandbox.add_time(Duration::from_millis(round_timeout));
    let new_round_timeout = sandbox.current_round_timeout();
    sandbox.add_time(Duration::from_millis(new_round_timeout + PROPOSE_TIMEOUT));

    assert!(sandbox.is_leader());
    sandbox.assert_state(Height(1), Round(3));

    // OK, we are the leader.
    let propose = ProposeBuilder::new(&sandbox).build();
    let prevote = make_prevote_from_propose(&sandbox, &propose);
    sandbox.broadcast(&propose);
    sandbox.broadcast(&prevote);

    let current_epoch = sandbox.current_epoch();
    let current_round = sandbox.current_round();

    let sandbox_restarted = sandbox.restart();
    sandbox_restarted.broadcast(&prevote);
    sandbox_restarted.assert_lock(NOT_LOCKED, None);
    sandbox_restarted.assert_state(current_epoch, current_round);
    sandbox_restarted.add_time(Duration::from_millis(PROPOSE_TIMEOUT));

    // Now we should be sure that node recovered its state but didn't send any messages.
    // Here `sandbox_restarted` goes out of scope and `sandbox_restarted.drop()` will cause panic
    // if there any sent messages.
}

#[test]
fn should_not_send_propose_after_node_restart_in_first_round() {
    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();
    // Increase blockchain height so that our node is the leader in the first round.
    add_one_height(&sandbox, &sandbox_state);
    add_one_height(&sandbox, &sandbox_state);
    sandbox.add_time(Duration::from_millis(PROPOSE_TIMEOUT));

    assert!(sandbox.is_leader());
    sandbox.assert_state(Height(3), Round(1));
    let propose = ProposeBuilder::new(&sandbox).build();
    let prevote = make_prevote_from_propose(&sandbox, &propose);
    sandbox.broadcast(&propose);
    sandbox.broadcast(&prevote);

    let sandbox_restarted = sandbox.restart();
    sandbox_restarted.broadcast(&prevote);
    sandbox_restarted.assert_lock(NOT_LOCKED, None);
    sandbox_restarted.assert_state(Height(3), Round(1));
    sandbox_restarted.add_time(Duration::from_millis(PROPOSE_TIMEOUT));
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
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(1)),
    ));
    sandbox.assert_lock(NOT_LOCKED, None); // Do not lock if <2/3 prevotes

    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(2),
        Height(1),
        Round(1),
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(2)),
    ));
    sandbox.assert_lock(Round(1), Some(propose.object_hash()));

    let precommit = sandbox.create_precommit(
        ValidatorId(0),
        Height(1),
        Round(1),
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(0)),
    );
    sandbox.broadcast(&precommit);
    sandbox.assert_lock(Round(1), Some(propose.object_hash()));
    let current_height = sandbox.current_epoch();
    let current_round = sandbox.current_round();

    // Simulate node restart.
    let sandbox_restarted = sandbox.restart();

    // Assert that consensus messages were recovered and we're in locked state now.
    sandbox_restarted.assert_lock(Round(1), Some(propose.object_hash()));
    sandbox_restarted.assert_state(current_height, current_round);
    sandbox_restarted.broadcast(&prevote);
    sandbox_restarted.broadcast(&precommit);

    // Receive another propose within the round
    let tx = gen_timestamping_tx();
    receive_valid_propose_with_transactions(&sandbox_restarted, &[tx.object_hash()]);

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
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(1)),
    ));
    sandbox.assert_lock(NOT_LOCKED, None); //do not lock if <2/3 prevotes

    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(2),
        Height(1),
        Round(1),
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(2)),
    ));
    sandbox.assert_lock(Round(1), Some(propose.object_hash()));

    let precommit = sandbox.create_precommit(
        ValidatorId(0),
        Height(1),
        Round(1),
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(0)),
    );

    sandbox.broadcast(&precommit);

    let current_height = sandbox.current_epoch();
    let current_round = sandbox.current_round();

    // Simulate node restart
    let sandbox_restarted = sandbox.restart();

    // assert that consensus messages were recovered and we're in locked state now
    sandbox_restarted.assert_lock(Round(1), Some(propose.object_hash()));
    sandbox_restarted.assert_state(current_height, current_round);
    sandbox_restarted.broadcast(&prevote);
    sandbox_restarted.broadcast(&precommit);

    sandbox_restarted.recv(&sandbox_restarted.create_precommit(
        ValidatorId(1),
        Height(1),
        Round(1),
        propose.object_hash(),
        block.object_hash(),
        sandbox_restarted.time().into(),
        sandbox_restarted.secret_key(ValidatorId(1)),
    ));

    sandbox_restarted.recv(&sandbox_restarted.create_precommit(
        ValidatorId(2),
        Height(1),
        Round(1),
        propose.object_hash(),
        block.object_hash(),
        sandbox_restarted.time().into(),
        sandbox_restarted.secret_key(ValidatorId(2)),
    ));

    sandbox_restarted.assert_state(Height(2), Round(1));
    sandbox_restarted.check_broadcast_status(Height(2), block.object_hash());
}

/// Idea:
/// - Node locks on some Propose.
/// - Node doesn't get any precommits within the round.
/// - Next round appears.
/// - Node receives new propose from new leader and locks on it.
/// - Node restarts.
/// - Node should recover in same round and locked on same `Propose`.
#[test]
#[allow(clippy::too_many_lines)] // fine for tests
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
        first_propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(1)),
    ));
    sandbox.assert_lock(NOT_LOCKED, None); //do not lock if <2/3 prevotes

    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(2),
        Height(1),
        Round(1),
        first_propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(2)),
    ));
    sandbox.assert_lock(Round(1), Some(first_propose.object_hash()));

    let first_precommit = sandbox.create_precommit(
        ValidatorId(0),
        Height(1),
        Round(1),
        first_propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(0)),
    );

    sandbox.broadcast(&first_precommit);

    sandbox.assert_state(Height(1), Round(1));
    sandbox.add_time(Duration::from_millis(sandbox.current_round_timeout()));
    sandbox.assert_state(Height(1), Round(2));

    // make sure we broadcasted same Prevote for second round
    let first_updated_prevote = sandbox.create_prevote(
        first_prevote.payload().validator,
        first_prevote.payload().epoch,
        Round(2),
        first_prevote.payload().propose_hash,
        Round(1),
        sandbox.secret_key(ValidatorId(0)),
    );
    sandbox.broadcast(&first_updated_prevote);

    let second_propose = ProposeBuilder::new(&sandbox).build();
    let second_block = BlockBuilder::new(&sandbox).build();

    sandbox.recv(&second_propose);

    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(1),
        Height(1),
        Round(2),
        second_propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(1)),
    ));

    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(2),
        Height(1),
        Round(2),
        second_propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(2)),
    ));

    sandbox.assert_lock(Round(1), Some(first_propose.object_hash()));

    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(3),
        Height(1),
        Round(2),
        second_propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(3)),
    ));

    sandbox.assert_lock(Round(2), Some(second_propose.object_hash()));

    let second_precommit = sandbox.create_precommit(
        ValidatorId(0),
        Height(1),
        Round(2),
        second_propose.object_hash(),
        second_block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(0)),
    );
    sandbox.broadcast(&second_precommit);

    // Restart node.
    let saved_time = sandbox.time();
    let sandbox_new = sandbox.restart_with_time(saved_time);

    sandbox_new.assert_lock(Round(2), Some(second_propose.object_hash()));
    sandbox_new.assert_state(Height(1), Round(2));
    sandbox_new.broadcast(&first_prevote);

    let first_precommit_new_time = sandbox_new.create_precommit(
        first_precommit.payload().validator,
        first_precommit.payload().epoch,
        first_precommit.payload().round,
        first_precommit.payload().propose_hash,
        first_precommit.payload().block_hash,
        sandbox_new.time().into(),
        sandbox_new.secret_key(ValidatorId(0)),
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
    let public_key0 = sandbox.public_key(v0);
    let secret_key0 = sandbox.secret_key(v0).clone();
    let address0 = sandbox.address(v0);
    let public_key1 = sandbox.public_key(v1);
    let secret_key1 = sandbox.secret_key(v1).clone();
    let address1 = sandbox.address(v1);

    let time = sandbox.time();
    let connect_from_0 = Sandbox::create_connect(
        &public_key0,
        address0,
        time.into(),
        &user_agent(),
        &secret_key0,
    );
    let connect_from_1 = Sandbox::create_connect(
        &public_key1,
        address1,
        time.into(),
        &user_agent(),
        &secret_key1,
    );
    let peers_request = Sandbox::create_peers_request(public_key1, public_key0, &secret_key1);

    // check that peers are absent
    sandbox.recv(&peers_request);

    // receive a `Connect` message and the respond on it
    sandbox.recv(&connect_from_1);
    sandbox.send(public_key1, &connect_from_0);

    // restart the node
    let sandbox_restarted = sandbox.restart_uninitialized();

    // check that the node is connecting with the peer
    sandbox_restarted.send(public_key1, &connect_from_0);

    // check that the peer is restored
    sandbox_restarted.recv(&peers_request);
    sandbox_restarted.send(public_key1, &connect_from_1);
}
