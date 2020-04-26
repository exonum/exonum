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

// spell-checker:ignore uncommented, uncomment

//! Tests in this module are designed to test details of the consensus protocol
//! handling such as correct round state transition.

use bit_vec::BitVec;
use exonum::{
    blockchain::{Epoch, ProposerId},
    crypto::Hash,
    helpers::{Height, Round, ValidatorId},
    merkledb::ObjectHash,
    messages::Verified,
};

use std::{collections::HashSet, convert::TryFrom, time::Duration};

use crate::{
    messages::{PrevotesRequest, TransactionsRequest},
    sandbox::{
        sandbox_tests_helper::*, timestamping_sandbox, timestamping_sandbox_builder, Sandbox,
    },
    state::{PREVOTES_REQUEST_TIMEOUT, PROPOSE_REQUEST_TIMEOUT, TRANSACTIONS_REQUEST_TIMEOUT},
};

/// check scenario:
/// HANDLE FULL PROPOSE
/// - send prevote when lock=0 for known propose
#[test]
fn positive_get_propose_send_prevote() {
    let sandbox = timestamping_sandbox();

    let propose = ProposeBuilder::new(&sandbox).build();
    sandbox.recv(&propose);

    // check scenario:
    // HANDLE FULL PROPOSE
    // - send prevote when lock=0 for known propose
    sandbox.assert_lock(NOT_LOCKED, None);
    sandbox.broadcast(&sandbox.create_prevote(
        ValidatorId(0),
        Height(1),
        Round(1),
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(0)),
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

    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(2),
        Height(1),
        Round(1),
        Hash::zero(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(2)),
    ));
    sandbox.add_time(Duration::from_millis(sandbox.current_round_timeout() - 1));
    sandbox.send(
        sandbox.public_key(ValidatorId(2)),
        &Sandbox::create_propose_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(2)),
            Height(1),
            Hash::zero(),
            sandbox.secret_key(ValidatorId(0)),
        ),
    );
    sandbox.add_time(Duration::from_millis(0));
}

/// - request prevotes when get prevote message
#[test]
fn request_prevotes_when_get_prevote_message() {
    let sandbox = timestamping_sandbox();

    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(2),
        Height(1),
        Round(1),
        Hash::zero(),
        Round(1),
        sandbox.secret_key(ValidatorId(2)),
    ));
    sandbox.add_time(Duration::from_millis(sandbox.current_round_timeout() - 1));
    sandbox.send(
        sandbox.public_key(ValidatorId(2)),
        &Sandbox::create_propose_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(2)),
            Height(1),
            Hash::zero(),
            sandbox.secret_key(ValidatorId(0)),
        ),
    );

    let mut validators = BitVec::from_elem(sandbox.validators().len(), false);
    validators.set(ValidatorId(2).into(), true);

    sandbox.send(
        sandbox.public_key(ValidatorId(2)),
        &Sandbox::create_prevote_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(2)),
            Height(1),
            Round(1),
            Hash::zero(),
            validators,
            sandbox.secret_key(ValidatorId(0)),
        ),
    );
    sandbox.add_time(Duration::from_millis(0));
}

/// - lock to propose when get +2/3 prevote
///     - only if round > locked round
/// &
/// - Send prevote
///     - round > locked + 1
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

    let propose = ProposeBuilder::new(&sandbox).build();

    let block = BlockBuilder::new(&sandbox).build();

    sandbox.recv(&propose);
    sandbox.broadcast(&sandbox.create_prevote(
        ValidatorId(0),
        Height(1),
        Round(1),
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(0)),
    ));

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
    sandbox.assert_lock(Round(1), Some(propose.object_hash())); //only if round > locked round

    sandbox.broadcast(&sandbox.create_precommit(
        ValidatorId(0),
        Height(1),
        Round(1),
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(0)),
    ));
    sandbox.assert_lock(Round(1), Some(propose.object_hash()));
    sandbox.add_time(Duration::from_millis(0));

    {
        // Send prevote even if current round > locked + 1
        // add round
        sandbox.add_time(Duration::from_millis(sandbox.current_round_timeout()));
        sandbox.broadcast(&sandbox.create_prevote(
            ValidatorId(0),
            Height(1),
            Round(2),
            propose.object_hash(),
            Round(1),
            sandbox.secret_key(ValidatorId(0)),
        ));

        // add round
        sandbox.add_time(Duration::from_millis(sandbox.current_round_timeout()));
        sandbox.broadcast(&sandbox.create_prevote(
            ValidatorId(0),
            Height(1),
            Round(3),
            propose.object_hash(),
            Round(1),
            sandbox.secret_key(ValidatorId(0)),
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
        sandbox.current_round_timeout() - PROPOSE_TIMEOUT,
    ));
    sandbox.assert_state(Height(1), Round(2));

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
    sandbox.assert_lock(Round(1), Some(propose.object_hash())); //only if round > locked round

    sandbox.broadcast(&sandbox.create_precommit(
        ValidatorId(0),
        Height(1),
        Round(1),
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(0)),
    ));
    sandbox.assert_lock(Round(1), Some(propose.object_hash()));
    // ! here broadcast of
    sandbox.broadcast(&sandbox.create_prevote(
        ValidatorId(0),
        Height(1),
        Round(2),
        propose.object_hash(),
        Round(1),
        sandbox.secret_key(ValidatorId(0)),
    ));
    sandbox.add_time(Duration::from_millis(0));

    {
        // Send prevote even if current round > locked + 1
        // add round
        sandbox.add_time(Duration::from_millis(sandbox.current_round_timeout()));
        sandbox.broadcast(&sandbox.create_prevote(
            ValidatorId(0),
            Height(1),
            Round(3),
            propose.object_hash(),
            Round(1),
            sandbox.secret_key(ValidatorId(0)),
        ));

        // add round
        sandbox.add_time(Duration::from_millis(sandbox.current_round_timeout()));
        sandbox.broadcast(&sandbox.create_prevote(
            ValidatorId(0),
            Height(1),
            Round(4),
            propose.object_hash(),
            Round(1),
            sandbox.secret_key(ValidatorId(0)),
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

    let propose = ProposeBuilder::new(&sandbox).build();

    let block = BlockBuilder::new(&sandbox).build();

    sandbox.recv(&propose);
    sandbox.broadcast(&sandbox.create_prevote(
        ValidatorId(0),
        Height(1),
        Round(1),
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(0)),
    ));

    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(1),
        Height(1),
        Round(1),
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(1)),
    ));
    sandbox.assert_lock(NOT_LOCKED, None); //do not lock if <2/3 prevotes

    {
        // in this block lock is obtained; without this lock requestPrevotes would have been sent
        sandbox.recv(&sandbox.create_prevote(
            ValidatorId(2),
            Height(1),
            Round(1),
            propose.object_hash(),
            NOT_LOCKED,
            sandbox.secret_key(ValidatorId(2)),
        ));
        sandbox.assert_lock(Round(1), Some(propose.object_hash())); //only if round > locked round

        sandbox.broadcast(&sandbox.create_precommit(
            ValidatorId(0),
            Height(1),
            Round(1),
            propose.object_hash(),
            block.object_hash(),
            sandbox.time().into(),
            sandbox.secret_key(ValidatorId(0)),
        ));
        sandbox.assert_lock(Round(1), Some(propose.object_hash()));
        sandbox.add_time(Duration::from_millis(0));
    }

    sandbox.recv(&sandbox.create_precommit(
        ValidatorId(1),
        Height(1),
        Round(1),
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(1)),
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
    let empty_propose = ProposeBuilder::new(&sandbox).build();
    sandbox.recv(&empty_propose);
    sandbox.broadcast(&make_prevote_from_propose(&sandbox, &empty_propose));

    let tx = gen_timestamping_tx();
    sandbox.recv(&tx);

    let propose = ProposeBuilder::new(&sandbox)
        .with_tx_hashes(&[tx.object_hash()])
        .build();
    let block = sandbox.create_block(&[tx]);
    sandbox.recv(&propose);

    // inc round
    sandbox.add_time(Duration::from_millis(sandbox.current_round_timeout()));
    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(1),
        Height(1),
        Round(2),
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(1)),
    ));
    sandbox.assert_lock(NOT_LOCKED, None); //do not lock if <2/3 prevotes

    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(2),
        Height(1),
        Round(2),
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(2)),
    ));
    sandbox.assert_lock(NOT_LOCKED, None);

    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(3),
        Height(1),
        Round(2),
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(3)),
    ));
    sandbox.assert_lock(Round(2), Some(propose.object_hash())); //only if round > locked round

    // !! here broadcast, of prevote from lock() function, occurs
    sandbox.broadcast(&sandbox.create_prevote(
        ValidatorId(0),
        Height(1),
        Round(2),
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(0)),
    ));

    sandbox.broadcast(&sandbox.create_precommit(
        ValidatorId(0),
        Height(1),
        Round(2),
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(0)),
    ));
    sandbox.assert_lock(Round(2), Some(propose.object_hash()));
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
    sandbox.add_time(Duration::from_millis(sandbox.current_round_timeout()));
    let propose = ProposeBuilder::new(&sandbox).build();
    let block = BlockBuilder::new(&sandbox).build();
    sandbox.recv(&propose);
    sandbox.broadcast(&make_prevote_from_propose(&sandbox, &propose));

    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(2),
        Height(1),
        Round(1),
        propose.object_hash(),
        Round(1),
        sandbox.secret_key(ValidatorId(2)),
    ));
    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(3),
        Height(1),
        Round(1),
        propose.object_hash(),
        Round(1),
        sandbox.secret_key(ValidatorId(3)),
    ));

    {
        // without this block RequestPrevotes would have been broadcast
        sandbox.recv(&sandbox.create_prevote(
            ValidatorId(1),
            Height(1),
            Round(1),
            propose.object_hash(),
            Round(1),
            sandbox.secret_key(ValidatorId(1)),
        ));
        sandbox.broadcast(&sandbox.create_prevote(
            ValidatorId(0),
            Height(1),
            Round(1),
            propose.object_hash(),
            NOT_LOCKED,
            sandbox.secret_key(ValidatorId(0)),
        ));
        sandbox.broadcast(&sandbox.create_precommit(
            ValidatorId(0),
            Height(1),
            Round(1),
            propose.object_hash(),
            block.object_hash(),
            sandbox.time().into(),
            sandbox.secret_key(ValidatorId(0)),
        ));
    }
    sandbox.add_time(Duration::from_millis(PREVOTES_REQUEST_TIMEOUT));
}

/// scenario: // HANDLE PRECOMMIT positive scenario
///         - Our `block_hash` is different from the precommits one.
#[test]
#[should_panic(expected = "handle_majority_precommits: wrong block hash.")]
fn handle_precommit_different_block_hash() {
    let sandbox = timestamping_sandbox();

    // option: with transaction
    let tx = gen_timestamping_tx();

    let propose = ProposeBuilder::new(&sandbox)
        .with_tx_hashes(&[tx.object_hash()]) //ordinary propose, but with this unreceived tx
        .build();

    // precommits with this block will be received
    // without tx
    let block = BlockBuilder::new(&sandbox).build();
    let precommit_1 = sandbox.create_precommit(
        ValidatorId(1),
        Height(1),
        Round(1),
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(1)),
    );
    let precommit_2 = sandbox.create_precommit(
        ValidatorId(2),
        Height(1),
        Round(1),
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(2)),
    );
    let precommit_3 = sandbox.create_precommit(
        ValidatorId(3),
        Height(1),
        Round(1),
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(3)),
    );

    sandbox.recv(&precommit_1);
    sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.public_key(ValidatorId(1)),
        &make_request_propose_from_precommit(&sandbox, precommit_1.as_ref()),
    );
    sandbox.send(
        sandbox.public_key(ValidatorId(1)),
        &make_request_prevote_from_precommit(&sandbox, precommit_1.as_ref()),
    );
    sandbox.recv(&propose);
    sandbox.recv(&tx);
    sandbox.broadcast(&make_prevote_from_propose(&sandbox, &propose));

    sandbox.recv(&precommit_2);
    // Here consensus.rs->handle_majority_precommits()->//Commit is achieved
    sandbox.recv(&precommit_3);
}

fn test_handle_precommit_incorrect_tx(known_before_propose: bool) {
    let sandbox = timestamping_sandbox();

    // Create a transaction for a non-existing service.
    let incorrect_tx = gen_incorrect_tx();
    // Create propose.
    let propose = ProposeBuilder::new(&sandbox)
        .with_tx_hashes(&[incorrect_tx.object_hash()])
        .build();
    // Create block.
    let block = sandbox.create_block(&[incorrect_tx.clone()]);

    let precommit_1 = sandbox.create_precommit(
        ValidatorId(1),
        Height(1),
        Round(1),
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(1)),
    );
    let precommit_2 = sandbox.create_precommit(
        ValidatorId(2),
        Height(1),
        Round(1),
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(2)),
    );
    let precommit_3 = sandbox.create_precommit(
        ValidatorId(3),
        Height(1),
        Round(1),
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(3)),
    );

    sandbox.recv(&precommit_1);
    sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.public_key(ValidatorId(1)),
        &make_request_propose_from_precommit(&sandbox, precommit_1.as_ref()),
    );
    sandbox.send(
        sandbox.public_key(ValidatorId(1)),
        &make_request_prevote_from_precommit(&sandbox, precommit_1.as_ref()),
    );

    if known_before_propose {
        sandbox.recv(&incorrect_tx);
        sandbox.assert_tx_cache_len(0);
        sandbox.recv(&propose);
    } else {
        sandbox.recv(&propose);
        sandbox.recv(&incorrect_tx);
    }

    // In normal conditions, here we should sent prevote, but since we consider
    // the propose to be incorrect, we won't do it.

    // However, majority of nodes decide this propose to be OK.
    sandbox.recv(&precommit_2);
    sandbox.recv(&precommit_3);

    // Here majority of precommits is achieved and node should panic.
}

/// Scenario for this test is similar to the `handle_precommit_different_block_hash`.
///
/// Here, node receives majority of precommits for a block with incorrect tx.
///
/// Normally, after receiving all the transactions for a propose, node should send a prevote for it.
/// In our case, propose contains the incorrect tx, so we expect node **NOT** to vote for it.
/// Later, when majority of nodes will send precommits (meaning that they agree with propose),
/// node should panic because it doesn't agree with the block being accepted.
#[test]
#[should_panic(expected = "handle_majority_precommits: propose contains")]
fn handle_precommit_incorrect_tx() {
    test_handle_precommit_incorrect_tx(false);
}

#[test]
#[should_panic(expected = "handle_majority_precommits: propose contains")]
fn handle_precommit_incorrect_tx_received_before_propose() {
    test_handle_precommit_incorrect_tx(true);
}

/// Here, node receives majority of prevotes but has propose with invalid tx.
///
/// Normally, after receiving majority of the prevotes node should send a precommit for it.
/// In our case, propose contains incorrect tx, so we expect node not to send precommit for it.
#[test]
fn not_sending_precommit_for_proposal_with_incorrect_tx() {
    let sandbox = timestamping_sandbox();

    let tx = gen_timestamping_tx();
    // Create propose.
    let propose = ProposeBuilder::new(&sandbox)
        .with_tx_hashes(&[tx.object_hash()])
        .build();
    // Create block.
    let block = sandbox.create_block(&[tx]);

    let precommit_1 = sandbox.create_precommit(
        ValidatorId(1),
        Height(1),
        Round(1),
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(1)),
    );

    sandbox.recv(&precommit_1);
    sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.public_key(ValidatorId(1)),
        &make_request_propose_from_precommit(&sandbox, precommit_1.as_ref()),
    );
    sandbox.send(
        sandbox.public_key(ValidatorId(1)),
        &make_request_prevote_from_precommit(&sandbox, precommit_1.as_ref()),
    );

    let incorrect_tx = gen_incorrect_tx();
    let incorrect_propose = ProposeBuilder::new(&sandbox)
        .with_tx_hashes(&[incorrect_tx.object_hash()])
        .build();
    sandbox.recv(&incorrect_propose);
    sandbox.recv(&incorrect_tx);

    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(1),
        Height(1),
        Round(1),
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(1)),
    ));
    sandbox.assert_lock(NOT_LOCKED, None);
    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(2),
        Height(1),
        Round(1),
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(2)),
    ));
    sandbox.assert_lock(NOT_LOCKED, None); //do not lock if <2/3 prevotes
    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(3),
        Height(1),
        Round(1),
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(3)),
    ));

    sandbox.add_time(Duration::from_millis(0));
}

#[test]
fn invalid_tx_does_not_invalidate_unrelated_proposes() {
    let sandbox = timestamping_sandbox();
    let invalid_tx = gen_incorrect_tx();

    let propose = ProposeBuilder::new(&sandbox).with_tx_hashes(&[]).build();
    sandbox.recv(&propose);
    let our_prevote = sandbox.create_prevote(
        ValidatorId(0),
        Height(1),
        Round(1),
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(0)),
    );
    sandbox.broadcast(&our_prevote);

    sandbox.recv(&invalid_tx);
    {
        let inner = sandbox.inner.borrow();
        let propose_state = inner.handler.state.propose(&propose.object_hash()).unwrap();
        assert!(!propose_state.has_invalid_txs());
    }

    let block = sandbox.create_block(&[]);
    let precommits = (1..4).map(|i| {
        let validator_id = ValidatorId(i);
        sandbox.create_precommit(
            validator_id,
            Height(1),
            Round(1),
            propose.object_hash(),
            block.object_hash(),
            sandbox.time().into(),
            sandbox.secret_key(validator_id),
        )
    });

    for precommit in precommits {
        sandbox.recv(&precommit);
    }
    sandbox.assert_state(Height(2), Round(1));
    sandbox.check_broadcast_status(Height(2), block.object_hash());
}

/// scenario: // HANDLE PRECOMMIT positive scenario with commit
#[test]
fn handle_precommit_positive_scenario_commit() {
    let sandbox = timestamping_sandbox();

    // option: with transaction
    let tx = gen_timestamping_tx();

    let propose = ProposeBuilder::new(&sandbox)
        .with_tx_hashes(&[tx.object_hash()]) //ordinary propose, but with this unreceived tx
        .build();
    let block = sandbox.create_block(&[tx.clone()]);

    let precommit_1 = sandbox.create_precommit(
        ValidatorId(1),
        Height(1),
        Round(1),
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(1)),
    );
    let precommit_2 = sandbox.create_precommit(
        ValidatorId(2),
        Height(1),
        Round(1),
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(2)),
    );
    let precommit_3 = sandbox.create_precommit(
        ValidatorId(3),
        Height(1),
        Round(1),
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(3)),
    );

    sandbox.recv(&precommit_1);
    sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.public_key(ValidatorId(1)),
        &make_request_propose_from_precommit(&sandbox, precommit_1.as_ref()),
    );
    sandbox.send(
        sandbox.public_key(ValidatorId(1)),
        &make_request_prevote_from_precommit(&sandbox, precommit_1.as_ref()),
    );

    sandbox.recv(&precommit_2);
    // second addition is required in order to make sandbox time >= propose time because
    // this condition is checked at node/mod.rs->actual_round()
    sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.public_key(ValidatorId(2)),
        &make_request_propose_from_precommit(&sandbox, precommit_2.as_ref()),
    );
    sandbox.send(
        sandbox.public_key(ValidatorId(2)),
        &make_request_prevote_from_precommit(&sandbox, precommit_2.as_ref()),
    );
    sandbox.recv(&propose);
    sandbox.recv(&tx);
    sandbox.broadcast(&make_prevote_from_propose(&sandbox, &propose));

    // Here covered negative scenario for requirement: commit only If has +2/3 precommit
    sandbox.assert_state(Height(1), Round(1));
    // Here consensus.rs->handle_majority_precommits()->//Commit is achieved
    sandbox.recv(&precommit_3);
    sandbox.assert_state(Height(2), Round(1));
    sandbox.check_broadcast_status(Height(2), block.object_hash());
    sandbox.add_time(Duration::from_millis(0));
}

/// Checks that `handle_majority_precommits` saves a `Propose` confirmed by the supermajority
/// if it has unknown transactions or the proposal itself is not known.
fn test_transaction_after_propose_and_precommits(precommits_before_propose: usize) {
    let sandbox = timestamping_sandbox();

    let tx = gen_timestamping_tx();
    let propose = ProposeBuilder::new(&sandbox)
        .with_validator(ValidatorId(2))
        .with_tx_hashes(&[tx.object_hash()])
        .with_height(Height(1))
        .with_round(Round(1))
        .with_prev_hash(&sandbox.last_hash())
        .build();

    let block = sandbox.create_block(&[tx.clone()]);
    let mut precommits = (1..4).map(|i| {
        let validator_id = ValidatorId(i);
        sandbox.create_precommit(
            validator_id,
            Height(1),
            Round(1),
            propose.object_hash(),
            block.object_hash(),
            sandbox.time().into(),
            sandbox.secret_key(validator_id),
        )
    });

    for precommit in precommits.by_ref().take(precommits_before_propose) {
        sandbox.recv(&precommit);
    }
    sandbox.recv(&propose);
    for precommit in precommits {
        sandbox.recv(&precommit);
    }

    // Finally, the node receives the transaction from the propose.
    sandbox.recv(&tx);
    // This should be enough to commit the block.
    sandbox.assert_state(Height(2), Round(1));

    let our_prevote = sandbox.create_prevote(
        ValidatorId(0),
        Height(1),
        Round(1),
        propose.object_hash(),
        Round::zero(),
        sandbox.secret_key(ValidatorId(0)),
    );
    sandbox.broadcast(&our_prevote);
    sandbox.check_broadcast_status(Height(2), block.object_hash());
}

#[test]
fn transaction_after_propose_and_precommits() {
    test_transaction_after_propose_and_precommits(0);
}

#[test]
fn transaction_after_1_precommit_and_propose() {
    test_transaction_after_propose_and_precommits(1);
}

#[test]
fn transaction_after_2_precommits_and_propose() {
    test_transaction_after_propose_and_precommits(2);
}

#[test]
fn transaction_after_all_precommits_and_propose() {
    test_transaction_after_propose_and_precommits(3);
}

/// LOCK
/// - Send precommit during the lock   // covered in `lock_to_propose_when_get_2_3_prevote_positive`
/// - if there aren't incompatible prevotes
///     - if there're +2/3 precommits
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
    let propose = ProposeBuilder::new(&sandbox).build();

    // precommits with this block will be received
    let block = BlockBuilder::new(&sandbox).build();
    let precommit_1 = sandbox.create_precommit(
        ValidatorId(1),
        Height(1),
        Round(1),
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(1)),
    );
    let precommit_2 = sandbox.create_precommit(
        ValidatorId(2),
        Height(1),
        Round(1),
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(2)),
    );

    {
        sandbox.recv(&precommit_1);
        sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
        sandbox.send(
            sandbox.public_key(ValidatorId(1)),
            &make_request_propose_from_precommit(&sandbox, precommit_1.as_ref()),
        );
        sandbox.send(
            sandbox.public_key(ValidatorId(1)),
            &make_request_prevote_from_precommit(&sandbox, precommit_1.as_ref()),
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
            sandbox.public_key(ValidatorId(2)),
            &make_request_propose_from_precommit(&sandbox, precommit_2.as_ref()),
        );
        sandbox.send(
            sandbox.public_key(ValidatorId(2)),
            &make_request_prevote_from_precommit(&sandbox, precommit_2.as_ref()),
        );
    }

    {
        // this block will make lock, and, hence, one additional precommit
        sandbox.recv(&propose);
        sandbox.broadcast(&make_prevote_from_propose(&sandbox, &propose));

        sandbox.recv(&sandbox.create_prevote(
            ValidatorId(2),
            Height(1),
            Round(1),
            propose.object_hash(),
            NOT_LOCKED,
            sandbox.secret_key(ValidatorId(2)),
        ));
        sandbox.assert_lock(NOT_LOCKED, None); //do not lock if <2/3 prevotes

        sandbox.recv(&sandbox.create_prevote(
            ValidatorId(3),
            Height(1),
            Round(1),
            propose.object_hash(),
            NOT_LOCKED,
            sandbox.secret_key(ValidatorId(3)),
        ));

        sandbox.broadcast(&sandbox.create_precommit(
            ValidatorId(0),
            Height(1),
            Round(1),
            propose.object_hash(),
            block.object_hash(),
            sandbox.time().into(),
            sandbox.secret_key(ValidatorId(0)),
        ));
        sandbox.check_broadcast_status(Height(2), block.object_hash());
    }

    //    add rounds to become a leader to observe broadcast messages
    sandbox.add_time(Duration::from_millis(sandbox.current_round_timeout()));
    {
        // this broadcast of Prevote will occur only if block with precommit_2 is commented
        // it is possible to comment block of code with precommit_2 and uncomment below
        // broadcast of Prevote and test will remain green
        // if block with precommit_2 is uncommented, then during lock commit will occur and
        // lock will disappear and prevotes for disappeared lock (these prevotes are the
        // primary goal of the test) will not be sent
        //  !!!      sandbox.broadcast(&sandbox.create_prevote(ValidatorId(0), Height(0), Round(2),
        // propose.object_hash(), Round(1), sandbox.s(ValidatorId(0))));
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
        .with_tx_hashes(&[tx.object_hash()]) //ordinary propose, but with this unreceived tx
        .build();

    // this block with transactions should be in real
    let block = BlockBuilder::new(&sandbox)
        .with_tx_hash(&tx.object_hash())
        .build();

    let precommit_1 = sandbox.create_precommit(
        ValidatorId(1),
        Height(1),
        Round(1),
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(1)),
    );
    let precommit_2 = sandbox.create_precommit(
        ValidatorId(2),
        Height(1),
        Round(1),
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(2)),
    );
    let precommit_3 = sandbox.create_precommit(
        ValidatorId(3),
        Height(1),
        Round(1),
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(3)),
    );

    sandbox.recv(&precommit_1);
    sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.public_key(ValidatorId(1)),
        &make_request_propose_from_precommit(&sandbox, precommit_1.as_ref()),
    );
    sandbox.send(
        sandbox.public_key(ValidatorId(1)),
        &make_request_prevote_from_precommit(&sandbox, precommit_1.as_ref()),
    );

    sandbox.recv(&precommit_2);
    // Second addition is required in order to make sandbox time >= propose time because
    // this condition is checked at node/mod.rs->actual_round()
    sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.public_key(ValidatorId(2)),
        &make_request_propose_from_precommit(&sandbox, precommit_2.as_ref()),
    );
    sandbox.send(
        sandbox.public_key(ValidatorId(2)),
        &make_request_prevote_from_precommit(&sandbox, precommit_2.as_ref()),
    );
    // !! if this propose would be received, commit would occur and last assert will
    // require height one
    //    sandbox.recv(&propose);

    sandbox.assert_state(Height(1), Round(1));
    // Here consensus.rs->handle_majority_precommits()->//Commit is achieved
    sandbox.recv(&precommit_3);
    sandbox.assert_state(Height(1), Round(1));
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
        .with_tx_hashes(&[tx.object_hash()]) //ordinary propose, but with this unreceived tx
        .build();

    // this block with transactions should be in real
    let block = BlockBuilder::new(&sandbox)
        .with_tx_hash(&tx.object_hash())
        .build();

    let precommit_1 = sandbox.create_precommit(
        ValidatorId(1),
        Height(1),
        Round(1),
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(1)),
    );
    let precommit_2 = sandbox.create_precommit(
        ValidatorId(2),
        Height(1),
        Round(1),
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(2)),
    );
    let precommit_3 = sandbox.create_precommit(
        ValidatorId(3),
        Height(1),
        Round(1),
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(3)),
    );

    sandbox.recv(&precommit_1);
    sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.public_key(ValidatorId(1)),
        &make_request_propose_from_precommit(&sandbox, precommit_1.as_ref()),
    );
    sandbox.send(
        sandbox.public_key(ValidatorId(1)),
        &make_request_prevote_from_precommit(&sandbox, precommit_1.as_ref()),
    );

    sandbox.recv(&precommit_2);
    // Second addition is required in order to make sandbox time >= propose time because
    // this condition is checked at node/mod.rs->actual_round()
    sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.public_key(ValidatorId(2)),
        &make_request_propose_from_precommit(&sandbox, precommit_2.as_ref()),
    );
    sandbox.send(
        sandbox.public_key(ValidatorId(2)),
        &make_request_prevote_from_precommit(&sandbox, precommit_2.as_ref()),
    );

    sandbox.recv(&propose);
    // !! if this tx would be received, commit would occur and last assert will require height one
    //    sandbox.recv(&tx);

    sandbox.assert_state(Height(1), Round(1));
    // Here consensus.rs->handle_majority_precommits()->//Commit is achieved
    sandbox.recv(&precommit_3);
    sandbox.assert_state(Height(1), Round(1));
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
        .with_tx_hashes(&[tx.object_hash()]) //ordinary propose, but with this unreceived tx
        .build();

    // precommits with this block will be received
    let block = sandbox.create_block(&[tx.clone()]);

    let precommit_1 = sandbox.create_precommit(
        ValidatorId(1),
        Height(1),
        Round(1),
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(1)),
    );
    let precommit_2 = sandbox.create_precommit(
        ValidatorId(2),
        Height(1),
        Round(1),
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(2)),
    );
    let precommit_3 = sandbox.create_precommit(
        ValidatorId(3),
        Height(1),
        Round(1),
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(3)),
    );

    sandbox.recv(&precommit_1);
    sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.public_key(ValidatorId(1)),
        &make_request_propose_from_precommit(&sandbox, precommit_1.as_ref()),
    );
    sandbox.send(
        sandbox.public_key(ValidatorId(1)),
        &make_request_prevote_from_precommit(&sandbox, precommit_1.as_ref()),
    );

    sandbox.recv(&precommit_2);
    // Second addition is required in order to make sandbox time >= propose time because
    // this condition is checked at node/mod.rs->actual_round()
    sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.public_key(ValidatorId(2)),
        &make_request_propose_from_precommit(&sandbox, precommit_2.as_ref()),
    );
    sandbox.send(
        sandbox.public_key(ValidatorId(2)),
        &make_request_prevote_from_precommit(&sandbox, precommit_2.as_ref()),
    );

    //here consensus.rs->handle_majority_precommits()->//Commit is achieved
    sandbox.recv(&precommit_3);
    sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.public_key(ValidatorId(3)),
        &make_request_propose_from_precommit(&sandbox, precommit_3.as_ref()),
    );
    sandbox.send(
        sandbox.public_key(ValidatorId(3)),
        &make_request_prevote_from_precommit(&sandbox, precommit_3.as_ref()),
    );

    sandbox.assert_state(Height(1), Round(1));

    // !! if this tx would be received, commit would occur and last assert will require height one
    sandbox.recv(&tx);
    sandbox.recv(&propose);
    sandbox.broadcast(&sandbox.create_prevote(
        ValidatorId(0),
        Height(1),
        Round(1),
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(0)),
    ));
    sandbox.check_broadcast_status(Height(2), block.object_hash());

    sandbox.add_time(Duration::from_millis(0));
    sandbox.assert_state(Height(2), Round(1));
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
#[should_panic(expected = "handle_full_propose: wrong block hash")]
fn handle_full_propose_wrong_state_hash() {
    let sandbox = timestamping_sandbox();

    // option: with transaction
    let tx = gen_timestamping_tx();

    let propose = ProposeBuilder::new(&sandbox)
        .with_tx_hashes(&[tx.object_hash()]) //ordinary propose, but with this unreceived tx
        .build();

    // precommits with this block will be received
    let block = BlockBuilder::new(&sandbox)
        .with_tx_hash(&tx.object_hash())
        .with_state_hash(&Hash::zero())
        .build();

    let precommit_1 = sandbox.create_precommit(
        ValidatorId(1),
        Height(1),
        Round(1),
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(1)),
    );
    let precommit_2 = sandbox.create_precommit(
        ValidatorId(2),
        Height(1),
        Round(1),
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(2)),
    );
    let precommit_3 = sandbox.create_precommit(
        ValidatorId(3),
        Height(1),
        Round(1),
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(3)),
    );

    sandbox.recv(&precommit_1);
    sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.public_key(ValidatorId(1)),
        &make_request_propose_from_precommit(&sandbox, precommit_1.as_ref()),
    );
    sandbox.send(
        sandbox.public_key(ValidatorId(1)),
        &make_request_prevote_from_precommit(&sandbox, precommit_1.as_ref()),
    );

    sandbox.recv(&precommit_2);
    // Second addition is required in order to make sandbox time >= propose time because
    // this condition is checked at node/mod.rs->actual_round()
    sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.public_key(ValidatorId(2)),
        &make_request_propose_from_precommit(&sandbox, precommit_2.as_ref()),
    );
    sandbox.send(
        sandbox.public_key(ValidatorId(2)),
        &make_request_prevote_from_precommit(&sandbox, precommit_2.as_ref()),
    );

    // Here consensus.rs->handle_majority_precommits()->//Commit is achieved
    sandbox.recv(&precommit_3);
    sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.public_key(ValidatorId(3)),
        &make_request_propose_from_precommit(&sandbox, precommit_3.as_ref()),
    );
    sandbox.send(
        sandbox.public_key(ValidatorId(3)),
        &make_request_prevote_from_precommit(&sandbox, precommit_3.as_ref()),
    );

    sandbox.assert_state(Height(1), Round(1));

    // !! if this tx would be received, commit would occur and last assert will require height one
    sandbox.recv(&tx);
    sandbox.recv(&propose);
    sandbox.broadcast(&sandbox.create_prevote(
        ValidatorId(0),
        Height(1),
        Round(1),
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(0)),
    ));

    sandbox.add_time(Duration::from_millis(0));
    sandbox.assert_state(Height(2), Round(1));
}

/// - scenario: do not send precommit if have incompatible prevotes
#[test]
fn do_not_send_precommit_if_has_incompatible_prevotes() {
    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();

    let propose = ProposeBuilder::new(&sandbox).build();

    sandbox.recv(&propose);
    sandbox.broadcast(&sandbox.create_prevote(
        ValidatorId(0),
        Height(1),
        Round(1),
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(0)),
    ));

    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(1),
        Height(1),
        Round(1),
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(1)),
    ));
    sandbox.assert_lock(NOT_LOCKED, None); //do not lock if <2/3 prevotes
    add_round_with_transactions(&sandbox, &sandbox_state, &[]);

    let future_propose = ProposeBuilder::new(&sandbox)
        .with_validator(ValidatorId(3))
        .with_round(Round(2))
        .build();
    sandbox.recv(&future_propose);
    sandbox.broadcast(&sandbox.create_prevote(
        ValidatorId(0),
        Height(1),
        Round(2),
        future_propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(0)),
    ));

    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(3),
        Height(1),
        Round(1),
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(3)),
    ));
    sandbox.assert_lock(Round(1), Some(propose.object_hash())); //only if round > locked round

    // !! lock is obtained, but broadcast(Precommit is absent
    //    sandbox.broadcast(&sandbox.create_precommit(ValidatorId(0), Height(0), Round(1), propose.object_hash(),
    //          block.object_hash(), sandbox.s(ValidatorId(0))));
    sandbox.assert_lock(Round(1), Some(propose.object_hash()));
    sandbox.add_time(Duration::from_millis(0));
}

/// Same as positive scenario, but
///
/// - Start from 1 height
/// - One precommit is received from 0 round and queued.
#[test]
fn handle_precommit_positive_scenario_commit_with_queued_precommit() {
    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();

    // create some tx
    let tx = gen_timestamping_tx();

    // Precommits with this block will be received during get 1st height in
    // fn add_one_height_with_transaction()
    let mut first_block = sandbox.create_block(&[tx.clone()]);
    first_block.add_header::<ProposerId>(ValidatorId(0));

    // this propose will be used during second commit
    let height_one_propose = ProposeBuilder::new(&sandbox)
        .with_validator(ValidatorId(3))
        .with_height(Height(2))
        .with_prev_hash(&first_block.object_hash())
        .build();

    // This empty block will be created during second commit while manually creating precommits.
    let mut second_block = BlockBuilder::new(&sandbox)
        .with_proposer_id(ValidatorId(3))
        .with_state_hash(&first_block.state_hash)
        .build();
    second_block.height = Height(2);
    second_block.prev_hash = first_block.object_hash();
    second_block.additional_headers.insert::<Epoch>(Height(2));

    let precommit_1 = sandbox.create_precommit(
        ValidatorId(1),
        Height(2),
        Round(1),
        height_one_propose.object_hash(),
        second_block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(1)),
    );
    let precommit_2 = sandbox.create_precommit(
        ValidatorId(2),
        Height(2),
        Round(1),
        height_one_propose.object_hash(),
        second_block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(2)),
    );
    let precommit_3 = sandbox.create_precommit(
        ValidatorId(3),
        Height(2),
        Round(1),
        height_one_propose.object_hash(),
        second_block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(3)),
    );

    sandbox.recv(&precommit_1); // Early precommit from future height

    sandbox.assert_state(Height(1), Round(1));
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[tx]);
    sandbox.assert_state(Height(2), Round(1));
    assert_eq!(first_block.object_hash(), sandbox.last_hash());

    //this precommit is received at previous height and queued
    //    sandbox.recv(&precommit_1);
    sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.public_key(ValidatorId(1)),
        &make_request_propose_from_precommit(&sandbox, precommit_1.as_ref()),
    );
    sandbox.send(
        sandbox.public_key(ValidatorId(1)),
        &make_request_prevote_from_precommit(&sandbox, precommit_1.as_ref()),
    );

    sandbox.recv(&precommit_2);
    // second addition is required in order to make sandbox time >= propose time because
    // this condition is checked at node/mod.rs->actual_round()
    sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.public_key(ValidatorId(2)),
        &make_request_propose_from_precommit(&sandbox, precommit_2.as_ref()),
    );
    sandbox.send(
        sandbox.public_key(ValidatorId(2)),
        &make_request_prevote_from_precommit(&sandbox, precommit_2.as_ref()),
    );

    sandbox.recv(&height_one_propose);
    sandbox.broadcast(&sandbox.create_prevote(
        ValidatorId(0),
        Height(2),
        Round(1),
        height_one_propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(0)),
    ));

    sandbox.assert_state(Height(2), Round(1));
    // Here consensus.rs->handle_majority_precommits()->//Commit is achieved
    sandbox.recv(&precommit_3);
    sandbox.assert_state(Height(3), Round(1));
    sandbox.check_broadcast_status(Height(3), second_block.object_hash());
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
        sandbox.assert_state(Height(2), Round(1));
        {
            assert_eq!(*sandbox_state.time_since_round_start.borrow(), 0);
        }
        // assert!(sandbox.is_leader());
    }
    let current_round = sandbox.current_round();
    let current_height = sandbox.current_epoch();

    // this propose will be a valid one when 0 node will become a leader after last commit
    let propose = ProposeBuilder::new(&sandbox)
        .with_tx_hashes(&[tx.object_hash()]) //ordinary propose, but with this unreceived tx
        .build();

    // precommits with this block would be received if transaction will be received
    let block = sandbox.create_block(&[tx.clone()]);
    let precommit_1 = sandbox.create_precommit(
        ValidatorId(1),
        current_height,
        current_round,
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(1)),
    );
    let precommit_2 = sandbox.create_precommit(
        ValidatorId(2),
        current_height,
        current_round,
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(2)),
    );
    let precommit_3 = sandbox.create_precommit(
        ValidatorId(3),
        current_height,
        current_round,
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(3)),
    );

    sandbox.recv(&precommit_1);
    sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.public_key(ValidatorId(1)),
        &make_request_propose_from_precommit(&sandbox, precommit_1.as_ref()),
    );
    sandbox.send(
        sandbox.public_key(ValidatorId(1)),
        &make_request_prevote_from_precommit(&sandbox, precommit_1.as_ref()),
    );

    sandbox.recv(&precommit_2);
    // second addition is required in order to make sandbox time >= propose time because
    // this condition is checked at node/mod.rs->actual_round()
    sandbox.add_time(Duration::from_millis(PROPOSE_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.public_key(ValidatorId(2)),
        &make_request_propose_from_precommit(&sandbox, precommit_2.as_ref()),
    );
    sandbox.send(
        sandbox.public_key(ValidatorId(2)),
        &make_request_prevote_from_precommit(&sandbox, precommit_2.as_ref()),
    );

    {
        // get propose and appropriate transaction
        sandbox.recv(&propose);
        sandbox.recv(&tx);
        sandbox.broadcast(&make_prevote_from_propose(&sandbox, &propose));
    }
    // verify that transaction is received
    assert_eq!(vec![tx.object_hash()], sandbox.transactions_hashes());

    // receive precommit 3 and start commit process
    sandbox.assert_state(current_height, current_round);
    // Here consensus.rs->handle_majority_precommits()->//Commit is achieved
    sandbox.recv(&precommit_3);

    let new_height = current_height.next();
    sandbox.assert_state(new_height, Round(1));
    sandbox.check_broadcast_status(new_height, block.object_hash());

    let propose = ProposeBuilder::new(&sandbox).build();

    sandbox.add_time(Duration::from_millis(PROPOSE_TIMEOUT));
    sandbox.broadcast(&propose);
    sandbox.broadcast(&make_prevote_from_propose(&sandbox, &propose));
    sandbox.add_time(Duration::from_millis(
        sandbox.current_round_timeout() - PROPOSE_TIMEOUT,
    ));
    sandbox.assert_state(sandbox.current_epoch(), Round(2));
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
        .with_tx_hashes(&[tx.object_hash()]) //ordinary propose, but with this unreceived tx
        .build();

    sandbox.recv(&propose);
    sandbox.add_time(Duration::from_millis(TRANSACTIONS_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.public_key(ValidatorId(2)),
        &Sandbox::create_transactions_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(2)),
            vec![tx.object_hash()],
            sandbox.secret_key(ValidatorId(0)),
        ),
    );

    // here handle_tx() -> handle_full_propose() is called => broadcast(Prevote) is observed
    sandbox.recv(&tx);
    sandbox.broadcast(&make_prevote_from_propose(&sandbox, &propose));
    sandbox.add_time(Duration::from_millis(0));
}

/// - receive propose with unknown tx
/// - receive that tx
/// - broadcast prevote
#[test]
fn broadcast_prevote_with_tx_positive() {
    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();

    add_one_height(&sandbox, &sandbox_state);
    sandbox.assert_state(Height(2), Round(1));

    // option: with transaction
    let tx = gen_timestamping_tx();
    sandbox.recv(&tx);

    let propose = ProposeBuilder::new(&sandbox)
        .with_tx_hashes(&[tx.object_hash()]) // ordinary propose, but with this received tx
        .build();
    sandbox.recv(&propose);

    // check scenario:
    // HANDLE FULL PROPOSE
    // - send prevote when lock=0 for known propose
    sandbox.assert_lock(NOT_LOCKED, None);
    sandbox.broadcast(&sandbox.create_prevote(
        ValidatorId(0),
        Height(2),
        Round(1),
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(0)),
    ));
}

/// - receive propose with unknown tx
/// - receive that tx
/// - broadcast prevote
#[test]
fn handle_tx_ignore_existing_tx_in_blockchain() {
    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();

    // option: with transaction
    let tx = gen_timestamping_tx();

    add_one_height_with_transactions(&sandbox, &sandbox_state, &[tx.clone()]);
    sandbox.assert_state(Height(2), Round(1));

    // add rounds & become leader
    sandbox.add_time(Duration::from_millis(sandbox.current_round_timeout()));
    assert!(sandbox.is_leader());

    sandbox.recv(&tx);

    sandbox.add_time(Duration::from_millis(PROPOSE_TIMEOUT));
    let propose = ProposeBuilder::new(&sandbox)
        //.with_tx_hashes(&[tx.object_hash()]) //ordinary propose, but with this received tx
        // !! note that here no tx are expected whereas old tx is received earlier
        .with_tx_hashes(&[])
        .build();
    sandbox.broadcast(&propose);
    sandbox.broadcast(&make_prevote_from_propose(&sandbox, &propose));
    sandbox.add_time(Duration::from_millis(0));
}

/// Ignore transactions that fail `BlockchainMut::check_tx`.
/// Idea of test is to receive invalid tx (which is expected to be ignored) and
/// then broadcast prevote without this tx.
#[test]
fn handle_tx_ignore_invalid_tx() {
    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();

    add_one_height(&sandbox, &sandbox_state);
    sandbox.assert_state(Height(2), Round(1));

    // add rounds & become leader
    sandbox.add_time(Duration::from_millis(sandbox.current_round_timeout()));
    assert!(sandbox.is_leader());

    // Create a transaction to the non-existing service.
    let incorrect_tx = gen_incorrect_tx();
    // And create one correct tx that **should** be accepted.
    let correct_tx = gen_timestamping_tx();

    // Receive those messages.
    sandbox.recv(&incorrect_tx);
    sandbox.assert_tx_cache_len(0);

    sandbox.recv(&correct_tx);
    sandbox.assert_tx_cache_len(1);

    sandbox.add_time(Duration::from_millis(PROPOSE_TIMEOUT));

    // Create propose **without** this tx (but with correct tx).
    // It should be accepted, since invalid tx wasn't processed
    // due to failed `BlockchainMut::check_tx` validation.
    let propose = ProposeBuilder::new(&sandbox)
        .with_tx_hashes(&[correct_tx.object_hash()])
        .build();
    sandbox.broadcast(&propose);
    sandbox.broadcast(&make_prevote_from_propose(&sandbox, &propose));
    sandbox.add_time(Duration::from_millis(0));
}

#[test]
fn handle_precommit_remove_propose_request() {
    let sandbox = timestamping_sandbox_builder().build();
    let tx = gen_timestamping_tx();

    let propose = ProposeBuilder::new(&sandbox)
        .with_tx_hashes(&[tx.object_hash()])
        .build();
    let block = BlockBuilder::new(&sandbox)
        .with_tx_hash(&tx.object_hash())
        .build();

    let precommit = sandbox.create_precommit(
        propose.payload().validator,
        Height(1),
        Round(1),
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(propose.payload().validator),
    );

    sandbox.recv(&precommit);

    // Propose request shouldn't be sent now.
    sandbox.recv(&propose);
    sandbox.add_time(Duration::from_millis(TRANSACTIONS_REQUEST_TIMEOUT));

    sandbox.send(
        sandbox.public_key(propose.payload().validator),
        &Sandbox::create_transactions_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(propose.payload().validator),
            vec![tx.object_hash()],
            sandbox.secret_key(ValidatorId(0)),
        ),
    );

    // There is no known prevoters.
    let prevoters = BitVec::from_elem(sandbox.validators().len(), false);

    sandbox.send(
        sandbox.public_key(propose.payload().validator),
        &Sandbox::create_prevote_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(propose.payload().validator),
            Height(1),
            Round(1),
            propose.object_hash(),
            prevoters,
            sandbox.secret_key(ValidatorId(0)),
        ),
    );
}

/// The idea of the test:
/// - node receives one transaction (tx2);
/// - node receives all the precommits and prevotes for one proposal;
/// - node receives multiple proposals within one round:
///   - first proposal contains one transaction (tx1),
///     second one contains two transactions (tx1, tx2);
///   - both proposals are incomplete yet (tx1 is missing).
/// - node receives transaction (tx1);
/// - both proposals are now complete, node should send prevote and precommit.
///
/// Motivation:
/// consensus code contained a bug when during processing of full proposals
/// the second proposal was processed even though node already bumped the height.
#[test]
fn handle_receive_multiple_proposals_same_round() {
    let sandbox = timestamping_sandbox_builder().build();
    let tx_1 = gen_timestamping_tx();
    let tx_2 = gen_timestamping_tx();

    let propose_1 = ProposeBuilder::new(&sandbox)
        .with_tx_hashes(&[tx_1.object_hash()])
        .build();
    let propose_2 = ProposeBuilder::new(&sandbox)
        .with_tx_hashes(&[tx_1.object_hash(), tx_2.object_hash()])
        .build();

    let block_1 = BlockBuilder::new(&sandbox)
        .with_txs_hashes(&[tx_1.object_hash()])
        .build();
    let block_2 = BlockBuilder::new(&sandbox)
        .with_txs_hashes(&[tx_1.object_hash(), tx_2.object_hash()])
        .build();

    sandbox.recv(&tx_2);

    // Since multiple proposes for the same value will be sorted by the propose hash,
    // we want to send precommits/prevotes for the propose that will be processed **first**:
    // in this test we check, that if the first propose was properly processed, the second
    // propose won't break anything (at the moment of this test being added, processing of the
    // second propose results in a panic).
    let (propose_hash, block_hash, txs_in_pool) =
        if propose_1.object_hash() < propose_2.object_hash() {
            // `tx_2` will be left in pool.
            (propose_1.object_hash(), block_1.object_hash(), 1)
        } else {
            // No txs will be left in pool.
            (propose_2.object_hash(), block_2.object_hash(), 0)
        };

    // Receive prevotes.
    for i in 1..sandbox.validators().len() as u16 {
        sandbox.recv(&sandbox.create_prevote(
            ValidatorId(i),
            Height(1),
            Round(1),
            propose_hash,
            NOT_LOCKED,
            sandbox.secret_key(ValidatorId(i)),
        ));
    }

    // Receive precommits.
    for i in 1..sandbox.validators().len() as u16 {
        sandbox.recv(&sandbox.create_precommit(
            ValidatorId(i),
            Height(1),
            Round(1),
            propose_hash,
            block_hash,
            sandbox.time().into(),
            sandbox.secret_key(ValidatorId(i)),
        ))
    }

    sandbox.recv(&propose_1);
    sandbox.recv(&propose_2);

    sandbox.recv(&tx_1);
    // ^-- here both proposes become complete, no panic should occur.

    // We should send a prevote/precommit for the first (ordered by hash) proposal.
    sandbox.broadcast(&sandbox.create_prevote(
        ValidatorId(0),
        Height(1),
        Round(1),
        propose_hash,
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(0)),
    ));

    sandbox.broadcast(&sandbox.create_precommit(
        ValidatorId(0),
        Height(1),
        Round(1),
        propose_hash,
        block_hash,
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(0)),
    ));

    // Block should be applied.
    sandbox.broadcast(&Sandbox::create_status(
        sandbox.public_key(ValidatorId(0)),
        Height(2),
        block_hash,
        txs_in_pool,
        sandbox.secret_key(ValidatorId(0)),
    ));
}

#[test]
fn handle_precommit_remove_propose_request_ask_prevoters() {
    let sandbox = timestamping_sandbox_builder().build();

    let tx = gen_timestamping_tx();

    let propose = ProposeBuilder::new(&sandbox)
        .with_tx_hashes(&[tx.object_hash()])
        .build();

    let block = BlockBuilder::new(&sandbox)
        .with_tx_hash(&tx.object_hash())
        .build();

    let precommit = sandbox.create_precommit(
        propose.payload().validator,
        Height(1),
        Round(1),
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(propose.payload().validator),
    );

    sandbox.recv(&precommit);

    let mut prevoters = BitVec::from_elem(sandbox.validators().len(), false);
    for i in 1..sandbox.validators().len() as u16 {
        sandbox.recv(&sandbox.create_prevote(
            ValidatorId(i),
            Height(1),
            Round(1),
            propose.object_hash(),
            Round(0),
            sandbox.secret_key(ValidatorId(i)),
        ));
        prevoters.set(i as usize, true);
    }

    // Propose request shouldn't be sent now.
    sandbox.recv(&propose);

    let mut validators = (1..sandbox.validators().len() as u16)
        .map(|x| sandbox.public_key(ValidatorId(x)))
        .collect::<HashSet<_>>();

    for i in 1..sandbox.validators().len() {
        sandbox.add_time(Duration::from_millis(TRANSACTIONS_REQUEST_TIMEOUT));
        sandbox.process_events();

        let (_, msg) = sandbox.pop_sent_message().unwrap();
        let msg = Verified::<TransactionsRequest>::try_from(msg)
            .expect("Incorrect message. TransactionsRequest was expected.");

        assert!(
            validators.remove(&msg.payload().to),
            "Unexpected validator's PublicKey"
        );

        if i == 1 {
            sandbox.send(
                sandbox.public_key(propose.payload().validator),
                &Sandbox::create_prevote_request(
                    sandbox.public_key(ValidatorId(0)),
                    sandbox.public_key(propose.payload().validator),
                    Height(1),
                    Round(1),
                    propose.object_hash(),
                    prevoters.clone(),
                    sandbox.secret_key(ValidatorId(0)),
                ),
            );
        }
    }

    assert!(
        validators.is_empty(),
        "Should send TransactionsRequest to all validators"
    );
}

#[test]
fn handle_precommit_remove_propose_request_ask_precommitters() {
    let sandbox = timestamping_sandbox();
    let tx = gen_timestamping_tx();

    let propose = ProposeBuilder::new(&sandbox)
        .with_tx_hashes(&[tx.object_hash()])
        .build();

    let block = BlockBuilder::new(&sandbox)
        .with_tx_hash(&tx.object_hash())
        .build();

    for i in 1..sandbox.validators().len() as u16 {
        sandbox.recv(&sandbox.create_precommit(
            ValidatorId(i),
            Height(1),
            Round(1),
            propose.object_hash(),
            block.object_hash(),
            sandbox.time().into(),
            sandbox.secret_key(ValidatorId(i)),
        ))
    }

    // Propose request shouldn't be sent now.
    sandbox.recv(&propose);

    let mut validators = (1..sandbox.validators().len() as u16)
        .map(|x| sandbox.public_key(ValidatorId(x)))
        .collect::<HashSet<_>>();

    for _ in 1..sandbox.validators().len() {
        sandbox.add_time(Duration::from_millis(TRANSACTIONS_REQUEST_TIMEOUT));
        sandbox.process_events();

        let (_, msg) = sandbox.pop_sent_message().unwrap();
        let msg = Verified::<TransactionsRequest>::try_from(msg)
            .expect("Incorrect message. TransactionsRequest was expected.");

        assert!(
            validators.remove(&msg.payload().to),
            "Unexpected validator's PublicKey"
        );

        let (_, msg) = sandbox.pop_sent_message().unwrap();
        Verified::<PrevotesRequest>::try_from(msg)
            .expect("Incorrect message. PrevotesRequest was expected.");
    }

    assert!(
        validators.is_empty(),
        "Should send TransactionsRequest to all validators"
    );
}
