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

//! Tests in this module are designed to test details of round timeout handling.

use std::time::Duration;

use crypto::CryptoHash;
use helpers::Round;
use messages::{Message, Precommit, Prevote};
use node::state::PROPOSE_REQUEST_TIMEOUT;

use sandbox::{sandbox::timestamping_sandbox, sandbox_tests_helper::*};

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
