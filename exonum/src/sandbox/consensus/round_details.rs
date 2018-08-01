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

// spell-checker:ignore uncommented, uncomment

//! Tests in this module are designed to test details of the consensus protocol
//! handling such as correct round state transition.

use bit_vec::BitVec;

use std::time::Duration;

use crypto::CryptoHash;
use messages::{Message, Precommit, Prevote, PrevotesRequest, ProposeRequest, TransactionsRequest};
use node::state::{
    PREVOTES_REQUEST_TIMEOUT, PROPOSE_REQUEST_TIMEOUT, TRANSACTIONS_REQUEST_TIMEOUT,
};
use sandbox::{sandbox::timestamping_sandbox, sandbox_tests_helper::*};

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

    let block_1_delay = 2 * sandbox.round_timeout() + PROPOSE_TIMEOUT + 1;
    let block_2_delay = 2 * sandbox.round_timeout() + 2 * PROPOSE_TIMEOUT + 1;

    // create some tx
    let tx = gen_timestamping_tx();

    // Precommits with this block will be received during get 1st height in
    // fn add_one_height_with_transaction()
    let first_block = BlockBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(block_1_delay)
        .with_proposer_id(VALIDATOR_0)
        .with_tx_hash(&tx.hash())
        .with_state_hash(&sandbox.compute_state_hash(&[tx.raw().clone()]))
        .build();

    // this propose will be used during second commit
    let height_one_propose = ProposeBuilder::new(&sandbox)
        .with_validator(VALIDATOR_3)
        .with_height(HEIGHT_TWO)
        .with_duration_since_sandbox_time(block_2_delay)
        .with_prev_hash(&first_block.hash())
        .build();

    // this block will be created during second commit while manually creating precommits
    let second_block = BlockBuilder::new(&sandbox)
        .with_proposer_id(VALIDATOR_3)
        .with_height(HEIGHT_TWO)
        .with_duration_since_sandbox_time(block_2_delay)
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
