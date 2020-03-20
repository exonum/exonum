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

//! Tests in this module are designed to test communication related to block requests.

use exonum::{
    blockchain::Epoch,
    crypto::Hash,
    helpers::{Height, Round, ValidatorId},
    merkledb::ObjectHash,
};

use std::time::Duration;

use crate::{
    sandbox::{sandbox_tests_helper::*, timestamping_sandbox, Sandbox},
    state::{BLOCK_REQUEST_TIMEOUT, TRANSACTIONS_REQUEST_TIMEOUT},
};

/// - get `Status` from other node with later height, send `BlockRequest` to this node
/// - receive `BlockResponse` with unknown tx A
/// - send `TransactionsRequest` with unknown tx A
/// - receive `TransactionsResponse` with tx A
/// - Block should be executed and committed
#[test]
fn handle_block_response_with_unknown_tx() {
    let sandbox = timestamping_sandbox();
    let tx = gen_timestamping_tx();
    let propose = ProposeBuilder::new(&sandbox).build();
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

    sandbox.recv(&Sandbox::create_status(
        sandbox.public_key(ValidatorId(3)),
        Height(2),
        block.object_hash(),
        0,
        sandbox.secret_key(ValidatorId(3)),
    ));

    sandbox.add_time(Duration::from_millis(BLOCK_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.public_key(ValidatorId(3)),
        &Sandbox::create_block_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(3)),
            Height(1),
            sandbox.secret_key(ValidatorId(0)),
        ),
    );

    sandbox.recv(&Sandbox::create_block_response(
        sandbox.public_key(ValidatorId(3)),
        sandbox.public_key(ValidatorId(0)),
        block.clone(),
        vec![precommit_1, precommit_2, precommit_3],
        vec![tx.object_hash()],
        sandbox.secret_key(ValidatorId(3)),
    ));

    sandbox.add_time(Duration::from_millis(TRANSACTIONS_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.public_key(ValidatorId(3)),
        &Sandbox::create_transactions_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(3)),
            vec![tx.object_hash()],
            sandbox.secret_key(ValidatorId(0)),
        ),
    );

    sandbox.recv(&Sandbox::create_transactions_response(
        sandbox.public_key(ValidatorId(3)),
        sandbox.public_key(ValidatorId(0)),
        vec![tx],
        sandbox.secret_key(ValidatorId(3)),
    ));

    sandbox.assert_state(Height(2), Round(1));
    sandbox.broadcast(&Sandbox::create_status(
        sandbox.public_key(ValidatorId(0)),
        Height(2),
        block.object_hash(),
        0,
        sandbox.secret_key(ValidatorId(0)),
    ));
}

fn test_handle_block_response_with_incorrect_tx(known_before_block: bool) {
    let sandbox = timestamping_sandbox();

    // Create correct tx, and then sign with the wrong destination.
    let incorrect_tx = gen_incorrect_tx();
    let propose = ProposeBuilder::new(&sandbox).build();
    let block = sandbox.create_block(&[incorrect_tx.clone()]);

    let precommits: Vec<_> = (1..=3)
        .map(|id| {
            sandbox.create_precommit(
                ValidatorId(id),
                Height(1),
                Round(1),
                propose.object_hash(),
                block.object_hash(),
                sandbox.time().into(),
                sandbox.secret_key(ValidatorId(id)),
            )
        })
        .collect();

    sandbox.recv(&Sandbox::create_status(
        sandbox.public_key(ValidatorId(3)),
        Height(2),
        block.object_hash(),
        0,
        sandbox.secret_key(ValidatorId(3)),
    ));

    if known_before_block {
        // Receive incorrect tx if we need it before block.
        sandbox.recv(&incorrect_tx);
    }

    sandbox.add_time(Duration::from_millis(BLOCK_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.public_key(ValidatorId(3)),
        &Sandbox::create_block_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(3)),
            Height(1),
            sandbox.secret_key(ValidatorId(0)),
        ),
    );

    sandbox.recv(&Sandbox::create_block_response(
        sandbox.public_key(ValidatorId(3)),
        sandbox.public_key(ValidatorId(0)),
        block,
        precommits,
        vec![incorrect_tx.object_hash()],
        sandbox.secret_key(ValidatorId(3)),
    ));

    if !known_before_block {
        // Request transaction if we haven't received it yet.
        sandbox.add_time(Duration::from_millis(TRANSACTIONS_REQUEST_TIMEOUT));
        sandbox.send(
            sandbox.public_key(ValidatorId(3)),
            &Sandbox::create_transactions_request(
                sandbox.public_key(ValidatorId(0)),
                sandbox.public_key(ValidatorId(3)),
                vec![incorrect_tx.object_hash()],
                sandbox.secret_key(ValidatorId(0)),
            ),
        );

        sandbox.recv(&Sandbox::create_transactions_response(
            sandbox.public_key(ValidatorId(3)),
            sandbox.public_key(ValidatorId(0)),
            vec![incorrect_tx],
            sandbox.secret_key(ValidatorId(3)),
        ));
    }

    // Here IncompleteBlock will become complete and since it contains
    // an incorrect tx, node should panic.
}

/// Node should **NOT** process block if tx is incorrect.
///
/// - get `Status` from other node with later height, send `BlockRequest` to this node
/// - receive `BlockResponse` with unknown tx A
/// - send `TransactionsRequest` with unknown tx A
/// - receive `TransactionsResponse` with tx A
/// - Figure out that tx A is incorrect
/// - Node should panic because of committed block with incorrect tx.
#[test]
#[should_panic(expected = "Received a block with transaction known as invalid")]
fn handle_block_response_with_incorrect_tx() {
    test_handle_block_response_with_incorrect_tx(false);
}

#[test]
#[should_panic(expected = "Received a block with transaction known as invalid")]
fn handle_block_response_with_known_incorrect_tx() {
    test_handle_block_response_with_incorrect_tx(true);
}

/// A block with an incorrect transactions order should not be processed.
///
/// - get `Status` from other node with later height, send `BlockRequest` to this node
/// - receive `BlockResponse` with unknown txs A and B in invalid order
/// - the processing of the block must be interrupted
#[test]
fn handle_block_response_with_invalid_txs_order() {
    let sandbox = timestamping_sandbox();

    let tx1 = gen_timestamping_tx();
    let tx2 = gen_timestamping_tx();
    let propose = ProposeBuilder::new(&sandbox).build();
    let block = sandbox.create_block(&[tx1.clone(), tx2.clone()]);

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

    sandbox.recv(&Sandbox::create_status(
        sandbox.public_key(ValidatorId(3)),
        Height(2),
        block.object_hash(),
        0,
        sandbox.secret_key(ValidatorId(3)),
    ));

    sandbox.add_time(Duration::from_millis(BLOCK_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.public_key(ValidatorId(3)),
        &Sandbox::create_block_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(3)),
            Height(1),
            sandbox.secret_key(ValidatorId(0)),
        ),
    );

    // Invalid transactions order.
    sandbox.recv(&Sandbox::create_block_response(
        sandbox.public_key(ValidatorId(3)),
        sandbox.public_key(ValidatorId(0)),
        block,
        vec![precommit_1, precommit_2, precommit_3],
        vec![tx2.object_hash(), tx1.object_hash()],
        sandbox.secret_key(ValidatorId(3)),
    ));

    sandbox.assert_state(Height(1), Round(1));
}

/// A block with an invalid precommit should not be processed.
///
/// - get `Status` from other node with later height, send `BlockRequest` to this node
/// - receive `BlockResponse` with one invalid `Precommit`
/// - the processing of the block must be interrupted
#[test]
fn handle_block_response_with_invalid_precommits() {
    let sandbox = timestamping_sandbox();
    let tx = gen_timestamping_tx();
    let propose = ProposeBuilder::new(&sandbox).build();

    let block1 = sandbox.create_block(&[tx.clone()]);
    let block2 = BlockBuilder::new(&sandbox).build();

    let precommit_1 = sandbox.create_precommit(
        ValidatorId(1),
        Height(1),
        Round(1),
        propose.object_hash(),
        block1.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(1)),
    );
    let precommit_2 = sandbox.create_precommit(
        ValidatorId(2),
        Height(1),
        Round(1),
        propose.object_hash(),
        block1.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(2)),
    );
    // Precommit with invalid block hash.
    let precommit_for_other_block = sandbox.create_precommit(
        ValidatorId(3),
        Height(1),
        Round(1),
        propose.object_hash(),
        block2.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(3)),
    );

    sandbox.recv(&Sandbox::create_status(
        sandbox.public_key(ValidatorId(3)),
        Height(2),
        block1.object_hash(),
        0,
        sandbox.secret_key(ValidatorId(3)),
    ));

    sandbox.add_time(Duration::from_millis(BLOCK_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.public_key(ValidatorId(3)),
        &Sandbox::create_block_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(3)),
            Height(1),
            sandbox.secret_key(ValidatorId(0)),
        ),
    );
    sandbox.recv(&tx);

    sandbox.recv(&Sandbox::create_block_response(
        sandbox.public_key(ValidatorId(3)),
        sandbox.public_key(ValidatorId(0)),
        block1,
        vec![precommit_1, precommit_2, precommit_for_other_block],
        vec![tx.object_hash()],
        sandbox.secret_key(ValidatorId(3)),
    ));

    sandbox.assert_state(Height(1), Round(1));
}

/// The block with some already known transactions should be processed.
///
/// - receive some tx A
/// - get `Status` from other node with later height, send `BlockRequest` to this node
/// - receive `BlockResponse` with one known tx A and unknown tx B
/// - send `TransactionsRequest` with txs A and B
/// - receive `TransactionsResponse` with txs A and B
/// - Block should be executed and committed
#[test]
fn handle_block_response_with_known_transaction() {
    let sandbox = timestamping_sandbox();
    let tx1 = gen_timestamping_tx();
    let tx2 = gen_timestamping_tx();
    sandbox.recv(&tx1);

    let propose = ProposeBuilder::new(&sandbox).build();
    let block = sandbox.create_block(&[tx1.clone(), tx2.clone()]);

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

    sandbox.recv(&Sandbox::create_status(
        sandbox.public_key(ValidatorId(3)),
        Height(2),
        block.object_hash(),
        0,
        sandbox.secret_key(ValidatorId(3)),
    ));

    sandbox.add_time(Duration::from_millis(BLOCK_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.public_key(ValidatorId(3)),
        &Sandbox::create_block_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(3)),
            Height(1),
            sandbox.secret_key(ValidatorId(0)),
        ),
    );

    sandbox.recv(&Sandbox::create_block_response(
        sandbox.public_key(ValidatorId(3)),
        sandbox.public_key(ValidatorId(0)),
        block.clone(),
        vec![precommit_1, precommit_2, precommit_3],
        vec![tx1.object_hash(), tx2.object_hash()],
        sandbox.secret_key(ValidatorId(3)),
    ));

    sandbox.add_time(Duration::from_millis(TRANSACTIONS_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.public_key(ValidatorId(3)),
        &Sandbox::create_transactions_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(3)),
            vec![tx2.object_hash()],
            sandbox.secret_key(ValidatorId(0)),
        ),
    );

    sandbox.recv(&Sandbox::create_transactions_response(
        sandbox.public_key(ValidatorId(3)),
        sandbox.public_key(ValidatorId(0)),
        vec![tx2],
        sandbox.secret_key(ValidatorId(3)),
    ));

    sandbox.assert_state(Height(2), Round(1));
    sandbox.broadcast(&Sandbox::create_status(
        sandbox.public_key(ValidatorId(0)),
        Height(2),
        block.object_hash(),
        0,
        sandbox.secret_key(ValidatorId(0)),
    ));
}

/// The block with already known transactions should be processed.
///
/// - receive some txs A and B
/// - get `Status` from other node with later height, send `BlockRequest` to this node
/// - receive `BlockResponse` with known txs A and B
/// - Block should be executed and committed
#[test]
fn handle_block_response_with_all_known_transactions() {
    let sandbox = timestamping_sandbox();
    let tx1 = gen_timestamping_tx();
    let tx2 = gen_timestamping_tx();
    sandbox.recv(&tx1);
    sandbox.recv(&tx2);

    let propose = ProposeBuilder::new(&sandbox).build();
    let block = sandbox.create_block(&[tx1.clone(), tx2.clone()]);

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

    sandbox.recv(&Sandbox::create_status(
        sandbox.public_key(ValidatorId(3)),
        Height(2),
        block.object_hash(),
        0,
        sandbox.secret_key(ValidatorId(3)),
    ));

    sandbox.add_time(Duration::from_millis(BLOCK_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.public_key(ValidatorId(3)),
        &Sandbox::create_block_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(3)),
            Height(1),
            sandbox.secret_key(ValidatorId(0)),
        ),
    );

    sandbox.recv(&Sandbox::create_block_response(
        sandbox.public_key(ValidatorId(3)),
        sandbox.public_key(ValidatorId(0)),
        block.clone(),
        vec![precommit_1, precommit_2, precommit_3],
        vec![tx1.object_hash(), tx2.object_hash()],
        sandbox.secret_key(ValidatorId(3)),
    ));

    sandbox.assert_state(Height(2), Round(1));
    sandbox.broadcast(&Sandbox::create_status(
        sandbox.public_key(ValidatorId(0)),
        Height(2),
        block.object_hash(),
        0,
        sandbox.secret_key(ValidatorId(0)),
    ));
}

/// The block should be processed even if there is a pending full propose.
///
/// - get `Status` from other node with later height, send `BlockRequest` to this node
/// - receive `BlockResponse` with unknown tx A
/// - receive `Propose` with unknown tx A
/// - send `TransactionsRequest` with unknown tx A for `Propose`
/// - send `TransactionsRequest` with unknown tx A for `Block`
/// - receive `TransactionsResponse` with tx A
/// - Block should be executed and committed
#[test]
fn received_block_while_there_is_full_propose() {
    let sandbox = timestamping_sandbox();
    let tx = gen_timestamping_tx();

    let propose = ProposeBuilder::new(&sandbox)
        .with_height(Height(1))
        .with_validator(ValidatorId(2))
        .with_tx_hashes(&[tx.object_hash()])
        .build();
    let block = sandbox.create_block(&[tx.clone()]);

    sandbox.recv(&Sandbox::create_status(
        sandbox.public_key(ValidatorId(3)),
        Height(2),
        block.object_hash(),
        0,
        sandbox.secret_key(ValidatorId(3)),
    ));

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

    sandbox.add_time(Duration::from_millis(BLOCK_REQUEST_TIMEOUT));

    sandbox.send(
        sandbox.public_key(ValidatorId(3)),
        &Sandbox::create_block_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(3)),
            Height(1),
            sandbox.secret_key(ValidatorId(0)),
        ),
    );

    sandbox.recv(&Sandbox::create_block_response(
        sandbox.public_key(ValidatorId(3)),
        sandbox.public_key(ValidatorId(0)),
        block.clone(),
        vec![precommit_1, precommit_2, precommit_3],
        vec![tx.object_hash()],
        sandbox.secret_key(ValidatorId(3)),
    ));

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

    sandbox.send(
        sandbox.public_key(ValidatorId(3)),
        &Sandbox::create_transactions_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(3)),
            vec![tx.object_hash()],
            sandbox.secret_key(ValidatorId(0)),
        ),
    );

    sandbox.recv(&Sandbox::create_transactions_response(
        sandbox.public_key(ValidatorId(3)),
        sandbox.public_key(ValidatorId(0)),
        vec![tx],
        sandbox.secret_key(ValidatorId(3)),
    ));

    sandbox.broadcast(&make_prevote_from_propose(&sandbox, &propose));

    sandbox.broadcast(&Sandbox::create_status(
        sandbox.public_key(ValidatorId(0)),
        Height(2),
        block.object_hash(),
        0,
        sandbox.secret_key(ValidatorId(0)),
    ));
}

/// The block should be processed even if there is a pending incomplete block.
///
/// - get `Status` from other node with later height, send `BlockRequest` to this node
/// - receive `BlockResponse` with unknown tx A
/// - receive one more `BlockResponse` with unknown tx A
/// - send `TransactionsRequest` with unknown tx A
/// - receive `TransactionsResponse` with tx A
/// - Block should be executed and committed
#[test]
fn received_block_while_there_is_pending_block() {
    let sandbox = timestamping_sandbox();
    let tx = gen_timestamping_tx();
    let propose = ProposeBuilder::new(&sandbox).build();
    let block = sandbox.create_block(&[tx.clone()]);

    sandbox.recv(&Sandbox::create_status(
        sandbox.public_key(ValidatorId(3)),
        Height(2),
        block.object_hash(),
        0,
        sandbox.secret_key(ValidatorId(3)),
    ));

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

    sandbox.add_time(Duration::from_millis(BLOCK_REQUEST_TIMEOUT));

    sandbox.send(
        sandbox.public_key(ValidatorId(3)),
        &Sandbox::create_block_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(3)),
            Height(1),
            sandbox.secret_key(ValidatorId(0)),
        ),
    );

    sandbox.recv(&Sandbox::create_block_response(
        sandbox.public_key(ValidatorId(3)),
        sandbox.public_key(ValidatorId(0)),
        block.clone(),
        vec![
            precommit_1.clone(),
            precommit_2.clone(),
            precommit_3.clone(),
        ],
        vec![tx.object_hash()],
        sandbox.secret_key(ValidatorId(3)),
    ));

    sandbox.recv(&Sandbox::create_block_response(
        sandbox.public_key(ValidatorId(3)),
        sandbox.public_key(ValidatorId(0)),
        block.clone(),
        vec![precommit_1, precommit_2, precommit_3],
        vec![tx.object_hash()],
        sandbox.secret_key(ValidatorId(3)),
    ));

    sandbox.add_time(Duration::from_millis(TRANSACTIONS_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.public_key(ValidatorId(3)),
        &Sandbox::create_transactions_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(3)),
            vec![tx.object_hash()],
            sandbox.secret_key(ValidatorId(0)),
        ),
    );

    sandbox.recv(&Sandbox::create_transactions_response(
        sandbox.public_key(ValidatorId(3)),
        sandbox.public_key(ValidatorId(0)),
        vec![tx],
        sandbox.secret_key(ValidatorId(3)),
    ));

    sandbox.assert_state(Height(2), Round(1));
    sandbox.broadcast(&Sandbox::create_status(
        sandbox.public_key(ValidatorId(0)),
        Height(2),
        block.object_hash(),
        0,
        sandbox.secret_key(ValidatorId(0)),
    ));
}

/// The block should be processed by requesting unknown transactions in several validators.
///
/// - get `Status` from second node with later height
/// - get `Status` from third node with later height
/// - send `BlockResponse` to second node
/// - receive `BlockResponse` with unknown tx A from third node
/// - send `TransactionsRequest` with unknown tx A to second node
/// - send `TransactionsRequest` with unknown tx A to third node
/// - receive `TransactionsResponse` with tx A from second node
/// - Block should be executed and committed
#[test]
#[ignore]
fn transactions_request_to_multiple_nodes() {
    let sandbox = timestamping_sandbox();
    let tx = gen_timestamping_tx();
    let propose = ProposeBuilder::new(&sandbox).build();
    let block = sandbox.create_block(&[tx.clone()]);

    sandbox.recv(&Sandbox::create_status(
        sandbox.public_key(ValidatorId(2)),
        Height(2),
        block.object_hash(),
        0,
        sandbox.secret_key(ValidatorId(2)),
    ));

    sandbox.recv(&Sandbox::create_status(
        sandbox.public_key(ValidatorId(3)),
        Height(2),
        block.object_hash(),
        0,
        sandbox.secret_key(ValidatorId(3)),
    ));

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

    sandbox.add_time(Duration::from_millis(BLOCK_REQUEST_TIMEOUT));

    sandbox.send(
        sandbox.public_key(ValidatorId(2)),
        &Sandbox::create_block_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(2)),
            Height(1),
            sandbox.secret_key(ValidatorId(0)),
        ),
    );

    sandbox.recv(&Sandbox::create_block_response(
        sandbox.public_key(ValidatorId(3)),
        sandbox.public_key(ValidatorId(0)),
        block.clone(),
        vec![precommit_1, precommit_2, precommit_3],
        vec![tx.object_hash()],
        sandbox.secret_key(ValidatorId(3)),
    ));

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

    sandbox.add_time(Duration::from_millis(TRANSACTIONS_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.public_key(ValidatorId(3)),
        &Sandbox::create_transactions_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(3)),
            vec![tx.object_hash()],
            sandbox.secret_key(ValidatorId(0)),
        ),
    );

    sandbox.recv(&Sandbox::create_transactions_response(
        sandbox.public_key(ValidatorId(2)),
        sandbox.public_key(ValidatorId(0)),
        vec![tx],
        sandbox.secret_key(ValidatorId(2)),
    ));

    sandbox.assert_state(Height(2), Round(1));
    sandbox.broadcast(&Sandbox::create_status(
        sandbox.public_key(ValidatorId(0)),
        Height(2),
        block.object_hash(),
        0,
        sandbox.secret_key(ValidatorId(0)),
    ));
}

/// Checks processing of a block from a future epoch, but the appropriate height.
fn test_block_request_with_epoch(
    epoch: Height,
    precommit_epoch: Height,
    tx_known_before_block: bool,
) {
    let sandbox = timestamping_sandbox();
    let tx = gen_timestamping_tx();
    let propose_hash = Hash::zero();

    let mut block = sandbox.create_block(&[tx.clone()]);
    block.additional_headers.insert::<Epoch>(epoch);
    let block_hash = block.object_hash();

    let precommits = (1..4).map(|i| {
        let validator = ValidatorId(i);
        sandbox.create_precommit(
            validator,
            precommit_epoch,
            Round(1),
            propose_hash,
            block_hash,
            sandbox.time().into(),
            sandbox.secret_key(validator),
        )
    });

    let status = sandbox.create_status_with_custom_epoch(ValidatorId(3), Height(2), Height(10));
    sandbox.recv(&status);

    sandbox.add_time(Duration::from_millis(BLOCK_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.public_key(ValidatorId(3)),
        &Sandbox::create_block_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(3)),
            Height(1),
            sandbox.secret_key(ValidatorId(0)),
        ),
    );

    if tx_known_before_block {
        sandbox.recv(&tx);
    }
    sandbox.recv(&Sandbox::create_block_response(
        sandbox.public_key(ValidatorId(3)),
        sandbox.public_key(ValidatorId(0)),
        block,
        precommits,
        vec![tx.object_hash()],
        sandbox.secret_key(ValidatorId(3)),
    ));
    if !tx_known_before_block {
        sandbox.recv(&tx);
    }

    if precommit_epoch == epoch {
        sandbox.assert_state(epoch.next(), Round(1));
        assert_eq!(sandbox.node_state().blockchain_height(), Height(2));
        let our_status = sandbox.create_our_status(epoch.next(), Height(2), 0);
        sandbox.broadcast(&our_status);
    } else {
        assert_eq!(sandbox.current_epoch(), Height(1));
    }
}

#[test]
fn block_request_with_immediate_epoch() {
    test_block_request_with_epoch(Height(1), Height(1), false);
}

#[test]
fn block_request_with_distanced_epoch() {
    test_block_request_with_epoch(Height(7), Height(7), true);
}

#[test]
fn block_request_with_distanced_epoch_and_unknown_tx() {
    test_block_request_with_epoch(Height(7), Height(7), false);
}

#[test]
fn block_request_with_invalid_precommits() {
    test_block_request_with_epoch(Height(7), Height(5), true);
}

fn test_skip_request(sandbox: &TimestampingSandbox, epoch: Height, precommit_epoch: Height) {
    let propose_hash = Hash::zero();
    let mut block = sandbox.create_block_skip();
    block.additional_headers.insert::<Epoch>(epoch);
    let block_hash = block.object_hash();

    let precommits = (1..4).map(|i| {
        let validator = ValidatorId(i);
        sandbox.create_precommit(
            validator,
            precommit_epoch,
            Round(1),
            propose_hash,
            block_hash,
            sandbox.time().into(),
            sandbox.secret_key(validator),
        )
    });

    let status = sandbox.create_status_with_custom_epoch(ValidatorId(3), Height(1), Height(10));
    sandbox.recv(&status);

    sandbox.add_time(Duration::from_millis(BLOCK_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.public_key(ValidatorId(3)),
        &Sandbox::create_full_block_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(3)),
            Height(1),
            Height(1),
            sandbox.secret_key(ValidatorId(0)),
        ),
    );

    sandbox.recv(&Sandbox::create_block_response(
        sandbox.public_key(ValidatorId(3)),
        sandbox.public_key(ValidatorId(0)),
        block,
        precommits,
        vec![],
        sandbox.secret_key(ValidatorId(3)),
    ));

    if precommit_epoch == epoch {
        sandbox.assert_state(epoch.next(), Round(1));
        assert_eq!(sandbox.node_state().blockchain_height(), Height(1));
        let our_status = sandbox.create_our_status(epoch.next(), Height(1), 0);
        sandbox.broadcast(&our_status);
    } else {
        assert_eq!(sandbox.current_epoch(), Height(1));
    }
}

#[test]
fn skip_request() {
    test_skip_request(&timestamping_sandbox(), Height(7), Height(7));
}

#[test]
fn skip_request_with_invalid_precommits() {
    test_skip_request(&timestamping_sandbox(), Height(7), Height(6));
}

#[test]
fn sequential_skip_requests() {
    let sandbox = timestamping_sandbox();
    test_skip_request(&sandbox, Height(4), Height(4));

    // Since the node doesn't achieve the epoch indicated in the peer status (10),
    // it should ask the node again, this time with a greater epoch (5 instead of 1).
    sandbox.add_time(Duration::from_millis(BLOCK_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.public_key(ValidatorId(3)),
        &Sandbox::create_full_block_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(3)),
            Height(1),
            Height(5),
            sandbox.secret_key(ValidatorId(0)),
        ),
    );
}

#[test]
fn skip_request_with_small_epoch() {
    let sandbox = timestamping_sandbox();
    add_one_height(&sandbox, &SandboxState::new());

    let propose_hash = Hash::zero();
    let past_epoch = Height(1);
    let mut block = sandbox.create_block_skip();
    block.additional_headers.insert::<Epoch>(past_epoch);
    let block_hash = block.object_hash();

    let precommits = (1..4).map(|i| {
        let validator = ValidatorId(i);
        sandbox.create_precommit(
            validator,
            past_epoch,
            Round(1),
            propose_hash,
            block_hash,
            sandbox.time().into(),
            sandbox.secret_key(validator),
        )
    });

    sandbox.recv(&Sandbox::create_block_response(
        sandbox.public_key(ValidatorId(3)),
        sandbox.public_key(ValidatorId(0)),
        block,
        precommits,
        vec![],
        sandbox.secret_key(ValidatorId(3)),
    ));
    sandbox.assert_state(Height(2), Round(1));
}

fn send_skip_request(block_height: Height, epoch: Height) -> TimestampingSandbox {
    let sandbox = timestamping_sandbox();
    let propose_hash = Hash::zero();
    let block_epoch = Height(8);
    let mut block = sandbox.create_block_skip();
    block.additional_headers.insert::<Epoch>(block_epoch);
    let block_hash = block.object_hash();

    let precommits = (1..4).map(|i| {
        let validator = ValidatorId(i);
        sandbox.create_precommit(
            validator,
            block_epoch,
            Round(1),
            propose_hash,
            block_hash,
            sandbox.time().into(),
            sandbox.secret_key(validator),
        )
    });

    // Reach some large epoch (9).
    let block_response = Sandbox::create_block_response(
        sandbox.public_key(ValidatorId(1)),
        sandbox.public_key(ValidatorId(0)),
        block,
        precommits,
        vec![],
        sandbox.secret_key(ValidatorId(1)),
    );
    sandbox.recv(&block_response);
    sandbox.broadcast(&sandbox.create_our_status(block_epoch.next(), Height(1), 0));

    let request = Sandbox::create_full_block_request(
        sandbox.public_key(ValidatorId(1)),
        sandbox.public_key(ValidatorId(0)),
        block_height,
        epoch,
        sandbox.secret_key(ValidatorId(1)),
    );
    sandbox.recv(&request);
    sandbox
}

#[test]
fn handle_skip_request() {
    let sandbox = send_skip_request(Height(1), Height(3));

    let proof = sandbox.block_skip_and_precommits().unwrap();
    let response = Sandbox::create_block_response(
        sandbox.public_key(ValidatorId(0)),
        sandbox.public_key(ValidatorId(1)),
        proof.block,
        proof.precommits,
        vec![],
        sandbox.secret_key(ValidatorId(0)),
    );
    sandbox.send(sandbox.public_key(ValidatorId(1)), &response);
}

#[test]
fn ignoring_skip_request_with_future_epoch() {
    let _sandbox = send_skip_request(Height(1), Height(20));
    // The sandbox will panic on drop if it sent a response.
}

#[test]
fn ignoring_skip_request_with_future_height() {
    let _sandbox = send_skip_request(Height(2), Height(7));
    // The sandbox will panic on drop if it sent a response.
}

#[test]
fn handle_outdated_skip_request() {
    let sandbox = timestamping_sandbox();
    let tx = gen_timestamping_tx();
    add_one_height_with_transactions(&sandbox, &SandboxState::new(), vec![&tx]);

    let request = Sandbox::create_full_block_request(
        sandbox.public_key(ValidatorId(1)),
        sandbox.public_key(ValidatorId(0)),
        Height(1),
        Height(1),
        sandbox.secret_key(ValidatorId(1)),
    );
    sandbox.recv(&request);

    let proof = sandbox.block_and_precommits(Height(1)).unwrap();
    let response = Sandbox::create_block_response(
        sandbox.public_key(ValidatorId(0)),
        sandbox.public_key(ValidatorId(1)),
        proof.block,
        proof.precommits,
        vec![tx.object_hash()],
        sandbox.secret_key(ValidatorId(0)),
    );
    sandbox.send(sandbox.public_key(ValidatorId(1)), &response);
}
