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
    helpers::{Height, Round, ValidatorId},
    merkledb::ObjectHash,
};

use std::time::Duration;

use crate::{
    sandbox::{sandbox_tests_helper::*, timestamping_sandbox},
    state::{BLOCK_REQUEST_TIMEOUT, TRANSACTIONS_REQUEST_TIMEOUT},
};

/// Handle block response:
///
/// - should process block even if tx in pool
///
/// The idea of test is:
///
/// - receive some tx A
/// - getting `Status` from other node with later height, send a `BlockRequest` to this node
/// - receive `BlockResponse` with already known tx A
/// - Block should be executed and committed
#[test]
fn handle_block_response_tx_in_pool() {
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

    sandbox.recv(&sandbox.create_status(
        sandbox.public_key(ValidatorId(3)),
        Height(2),
        block.object_hash(),
        0,
        sandbox.secret_key(ValidatorId(3)),
    ));

    sandbox.add_time(Duration::from_millis(BLOCK_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.public_key(ValidatorId(3)),
        &sandbox.create_block_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(3)),
            Height(1),
            sandbox.secret_key(ValidatorId(0)),
        ),
    );
    sandbox.recv(&tx);

    sandbox.recv(&sandbox.create_block_response(
        sandbox.public_key(ValidatorId(3)),
        sandbox.public_key(ValidatorId(0)),
        block.clone(),
        vec![precommit_1, precommit_2, precommit_3],
        vec![tx.object_hash()],
        sandbox.secret_key(ValidatorId(3)),
    ));

    sandbox.assert_state(Height(2), Round(1));
    sandbox.broadcast(&sandbox.create_status(
        sandbox.public_key(ValidatorId(0)),
        Height(2),
        block.object_hash(),
        0,
        sandbox.secret_key(ValidatorId(0)),
    ));
}

/// HANDLE block response

/// - should process block if tx is unknown
/// idea of test is:
/// - getting Status from other node with later height, send BlockRequest to this node
/// - receive BlockResponse with unknown tx A
/// - send TransactionsRequest with unknown tx A
/// - receive TransactionsResponse with tx A
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

    sandbox.recv(&sandbox.create_status(
        sandbox.public_key(ValidatorId(3)),
        Height(2),
        block.object_hash(),
        0,
        sandbox.secret_key(ValidatorId(3)),
    ));

    sandbox.add_time(Duration::from_millis(BLOCK_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.public_key(ValidatorId(3)),
        &sandbox.create_block_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(3)),
            Height(1),
            sandbox.secret_key(ValidatorId(0)),
        ),
    );

    sandbox.recv(&sandbox.create_block_response(
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
        &sandbox.create_transactions_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(3)),
            vec![tx.object_hash()],
            sandbox.secret_key(ValidatorId(0)),
        ),
    );

    sandbox.recv(&sandbox.create_transactions_response(
        sandbox.public_key(ValidatorId(3)),
        sandbox.public_key(ValidatorId(0)),
        vec![tx],
        sandbox.secret_key(ValidatorId(3)),
    ));

    sandbox.assert_state(Height(2), Round(1));
    sandbox.broadcast(&sandbox.create_status(
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

    sandbox.recv(&sandbox.create_status(
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
        &sandbox.create_block_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(3)),
            Height(1),
            sandbox.secret_key(ValidatorId(0)),
        ),
    );

    sandbox.recv(&sandbox.create_block_response(
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
            &sandbox.create_transactions_request(
                sandbox.public_key(ValidatorId(0)),
                sandbox.public_key(ValidatorId(3)),
                vec![incorrect_tx.object_hash()],
                sandbox.secret_key(ValidatorId(0)),
            ),
        );

        sandbox.recv(&sandbox.create_transactions_response(
            sandbox.public_key(ValidatorId(3)),
            sandbox.public_key(ValidatorId(0)),
            vec![incorrect_tx],
            sandbox.secret_key(ValidatorId(3)),
        ));
    }

    // Here IncompleteBlock will become complete and since it contains
    // an incorrect tx, node should panic.
}

/// - should **NOT** process block if tx is incorrect
/// idea of test is:
/// - getting Status from other node with later height, send BlockRequest to this node
/// - receive BlockResponse with unknown tx A
/// - send TransactionsRequest with unknown tx A
/// - receive TransactionsResponse with tx A
/// - Figure out that tx A is incorrect
/// - Node should panic because of committed block with incorrect tx.
#[test]
#[should_panic(expected = "Received a block with transaction known as invalid")]
fn handle_block_response_with_incorrect_tx() {
    test_handle_block_response_with_incorrect_tx(false);
}

/// - should **NOT** process block if tx is incorrect
/// idea of test is:
/// - getting Status from other node with later height, send BlockRequest to this node
/// - receive incorrect tx
/// - receive BlockResponse with known tx A
/// - Figure out that block is incorrect
/// - Node should panic because of committed block with incorrect tx.
#[test]
#[should_panic(expected = "Received a block with transaction known as invalid")]
fn handle_block_response_with_known_incorrect_tx() {
    test_handle_block_response_with_incorrect_tx(true);
}

/// HANDLE block response

/// - A block with an incorrect transactions order should not be processed
/// idea of test is:
/// - getting Status from other node with later height, send BlockRequest to this node
/// - receive BlockResponse with unknown txs A and B in invalid order
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

    sandbox.recv(&sandbox.create_status(
        sandbox.public_key(ValidatorId(3)),
        Height(2),
        block.object_hash(),
        0,
        sandbox.secret_key(ValidatorId(3)),
    ));

    sandbox.add_time(Duration::from_millis(BLOCK_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.public_key(ValidatorId(3)),
        &sandbox.create_block_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(3)),
            Height(1),
            sandbox.secret_key(ValidatorId(0)),
        ),
    );

    // Invalid transactions order.
    sandbox.recv(&sandbox.create_block_response(
        sandbox.public_key(ValidatorId(3)),
        sandbox.public_key(ValidatorId(0)),
        block,
        vec![precommit_1, precommit_2, precommit_3],
        vec![tx2.object_hash(), tx1.object_hash()],
        sandbox.secret_key(ValidatorId(3)),
    ));

    sandbox.assert_state(Height(1), Round(1));
}

/// HANDLE block response

/// - A block with an invalid precommit should not be processed
/// idea of test is:
/// - getting Status from other node with later height, send BlockRequest to this node
/// - receive BlockResponse with one invalid precommit
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

    sandbox.recv(&sandbox.create_status(
        sandbox.public_key(ValidatorId(3)),
        Height(2),
        block1.object_hash(),
        0,
        sandbox.secret_key(ValidatorId(3)),
    ));

    sandbox.add_time(Duration::from_millis(BLOCK_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.public_key(ValidatorId(3)),
        &sandbox.create_block_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(3)),
            Height(1),
            sandbox.secret_key(ValidatorId(0)),
        ),
    );
    sandbox.recv(&tx);

    sandbox.recv(&sandbox.create_block_response(
        sandbox.public_key(ValidatorId(3)),
        sandbox.public_key(ValidatorId(0)),
        block1,
        vec![precommit_1, precommit_2, precommit_for_other_block],
        vec![tx.object_hash()],
        sandbox.secret_key(ValidatorId(3)),
    ));

    sandbox.assert_state(Height(1), Round(1));
}

/// HANDLE block response

/// - the block with some already known transactions should be processed
/// idea of test is:
/// - receive some tx A
/// - getting Status from other node with later height, send BlockRequest to this node
/// - receive BlockResponse with one known tx A and unknown tx B
/// - send TransactionsRequest with txs A and B
/// - receive TransactionsResponse with txs A and B
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

    sandbox.recv(&sandbox.create_status(
        sandbox.public_key(ValidatorId(3)),
        Height(2),
        block.object_hash(),
        0,
        sandbox.secret_key(ValidatorId(3)),
    ));

    sandbox.add_time(Duration::from_millis(BLOCK_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.public_key(ValidatorId(3)),
        &sandbox.create_block_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(3)),
            Height(1),
            sandbox.secret_key(ValidatorId(0)),
        ),
    );

    sandbox.recv(&sandbox.create_block_response(
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
        &sandbox.create_transactions_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(3)),
            vec![tx2.object_hash()],
            sandbox.secret_key(ValidatorId(0)),
        ),
    );

    sandbox.recv(&sandbox.create_transactions_response(
        sandbox.public_key(ValidatorId(3)),
        sandbox.public_key(ValidatorId(0)),
        vec![tx2],
        sandbox.secret_key(ValidatorId(3)),
    ));

    sandbox.assert_state(Height(2), Round(1));
    sandbox.broadcast(&sandbox.create_status(
        sandbox.public_key(ValidatorId(0)),
        Height(2),
        block.object_hash(),
        0,
        sandbox.secret_key(ValidatorId(0)),
    ));
}

/// HANDLE block response

/// - the block with already known transactions should be processed
/// idea of test is:
/// - receive some txs A and B
/// - getting Status from other node with later height, send BlockRequest to this node
/// - receive BlockResponse with known txs A and B
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

    sandbox.recv(&sandbox.create_status(
        sandbox.public_key(ValidatorId(3)),
        Height(2),
        block.object_hash(),
        0,
        sandbox.secret_key(ValidatorId(3)),
    ));

    sandbox.add_time(Duration::from_millis(BLOCK_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.public_key(ValidatorId(3)),
        &sandbox.create_block_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(3)),
            Height(1),
            sandbox.secret_key(ValidatorId(0)),
        ),
    );

    sandbox.recv(&sandbox.create_block_response(
        sandbox.public_key(ValidatorId(3)),
        sandbox.public_key(ValidatorId(0)),
        block.clone(),
        vec![precommit_1, precommit_2, precommit_3],
        vec![tx1.object_hash(), tx2.object_hash()],
        sandbox.secret_key(ValidatorId(3)),
    ));

    sandbox.assert_state(Height(2), Round(1));
    sandbox.broadcast(&sandbox.create_status(
        sandbox.public_key(ValidatorId(0)),
        Height(2),
        block.object_hash(),
        0,
        sandbox.secret_key(ValidatorId(0)),
    ));
}

/// HANDLE block response

/// - the block should be processed even if there is a pending full propose
/// idea of test is:
/// - getting Status from other node with later height, send BlockRequest to this node
/// - receive BlockResponse with unknown tx A
/// - receive Propose with unknown tx A
/// - send TransactionsRequest with unknown tx A for Propose
/// - send TransactionsRequest with unknown tx A for Block
/// - receive TransactionsResponse with tx A
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

    sandbox.recv(&sandbox.create_status(
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
        &sandbox.create_block_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(3)),
            Height(1),
            sandbox.secret_key(ValidatorId(0)),
        ),
    );

    sandbox.recv(&sandbox.create_block_response(
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
        &sandbox.create_transactions_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(2)),
            vec![tx.object_hash()],
            sandbox.secret_key(ValidatorId(0)),
        ),
    );

    sandbox.send(
        sandbox.public_key(ValidatorId(3)),
        &sandbox.create_transactions_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(3)),
            vec![tx.object_hash()],
            sandbox.secret_key(ValidatorId(0)),
        ),
    );

    sandbox.recv(&sandbox.create_transactions_response(
        sandbox.public_key(ValidatorId(3)),
        sandbox.public_key(ValidatorId(0)),
        vec![tx],
        sandbox.secret_key(ValidatorId(3)),
    ));

    sandbox.broadcast(&make_prevote_from_propose(&sandbox, &propose));

    sandbox.broadcast(&sandbox.create_status(
        sandbox.public_key(ValidatorId(0)),
        Height(2),
        block.object_hash(),
        0,
        sandbox.secret_key(ValidatorId(0)),
    ));
}

/// HANDLE block response

/// - the block should be processed even if there is a pending incomplete block
/// idea of test is:
/// - getting Status from other node with later height, send BlockRequest to this node
/// - receive BlockResponse with unknown tx A
/// - receive one more BlockResponse with unknown tx A
/// - send TransactionsRequest with unknown tx A
/// - receive TransactionsResponse with tx A
/// - Block should be executed and committed
#[test]
fn received_block_while_there_is_pending_block() {
    let sandbox = timestamping_sandbox();
    let tx = gen_timestamping_tx();
    let propose = ProposeBuilder::new(&sandbox).build();
    let block = sandbox.create_block(&[tx.clone()]);

    sandbox.recv(&sandbox.create_status(
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
        &sandbox.create_block_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(3)),
            Height(1),
            sandbox.secret_key(ValidatorId(0)),
        ),
    );

    sandbox.recv(&sandbox.create_block_response(
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

    sandbox.recv(&sandbox.create_block_response(
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
        &sandbox.create_transactions_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(3)),
            vec![tx.object_hash()],
            sandbox.secret_key(ValidatorId(0)),
        ),
    );

    sandbox.recv(&sandbox.create_transactions_response(
        sandbox.public_key(ValidatorId(3)),
        sandbox.public_key(ValidatorId(0)),
        vec![tx],
        sandbox.secret_key(ValidatorId(3)),
    ));

    sandbox.assert_state(Height(2), Round(1));
    sandbox.broadcast(&sandbox.create_status(
        sandbox.public_key(ValidatorId(0)),
        Height(2),
        block.object_hash(),
        0,
        sandbox.secret_key(ValidatorId(0)),
    ));
}

// TODO: Rewrite sandbox methods so that you can receive/send messages in batches.
// Now the same messages are sent to the validators in a random order. (ECR-376)

/// HANDLE block response

/// - the block should be processed by requesting unknown transactions in several validators
/// idea of test is:
/// - getting Status from second node with later height
/// - getting Status from third node with later height
/// - send BlockResponse to second node
/// - receive BlockResponse with unknown tx A from third node
/// - send TransactionsRequest with unknown tx A to second node
/// - send TransactionsRequest with unknown tx A to third node
/// - receive TransactionsResponse with tx A from second node
/// - Block should be executed and committed
#[test]
#[ignore]
fn transactions_request_to_multiple_nodes() {
    let sandbox = timestamping_sandbox();
    let tx = gen_timestamping_tx();
    let propose = ProposeBuilder::new(&sandbox).build();
    let block = sandbox.create_block(&[tx.clone()]);

    sandbox.recv(&sandbox.create_status(
        sandbox.public_key(ValidatorId(2)),
        Height(2),
        block.object_hash(),
        0,
        sandbox.secret_key(ValidatorId(2)),
    ));

    sandbox.recv(&sandbox.create_status(
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
        &sandbox.create_block_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(2)),
            Height(1),
            sandbox.secret_key(ValidatorId(0)),
        ),
    );

    sandbox.recv(&sandbox.create_block_response(
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
        &sandbox.create_transactions_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(2)),
            vec![tx.object_hash()],
            sandbox.secret_key(ValidatorId(0)),
        ),
    );

    sandbox.add_time(Duration::from_millis(TRANSACTIONS_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.public_key(ValidatorId(3)),
        &sandbox.create_transactions_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(3)),
            vec![tx.object_hash()],
            sandbox.secret_key(ValidatorId(0)),
        ),
    );

    sandbox.recv(&sandbox.create_transactions_response(
        sandbox.public_key(ValidatorId(2)),
        sandbox.public_key(ValidatorId(0)),
        vec![tx],
        sandbox.secret_key(ValidatorId(2)),
    ));

    sandbox.assert_state(Height(2), Round(1));
    sandbox.broadcast(&sandbox.create_status(
        sandbox.public_key(ValidatorId(0)),
        Height(2),
        block.object_hash(),
        0,
        sandbox.secret_key(ValidatorId(0)),
    ));
}
