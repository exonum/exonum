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

//! Tests in this module are designed to test communication related to block requests.

use std::time::Duration;

use crypto::CryptoHash;
use helpers::{Height, Round, ValidatorId};
use messages::Message;
use node::state::{BLOCK_REQUEST_TIMEOUT, TRANSACTIONS_REQUEST_TIMEOUT};
use sandbox::{sandbox::timestamping_sandbox, sandbox_tests_helper::*};

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

    let propose = ProposeBuilder::new(&sandbox).build();

    let block = BlockBuilder::new(&sandbox)
        .with_tx_hash(&tx.hash())
        .with_state_hash(&sandbox.compute_state_hash(&[tx.raw().clone()]))
        .build();

    let precommit_1 = sandbox.create_precommit(
        ValidatorId(1),
        Height(1),
        Round(1),
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(ValidatorId(1)),
    );
    let precommit_2 = sandbox.create_precommit(
        ValidatorId(2),
        Height(1),
        Round(1),
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(ValidatorId(2)),
    );
    let precommit_3 = sandbox.create_precommit(
        ValidatorId(3),
        Height(1),
        Round(1),
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(ValidatorId(3)),
    );

    sandbox.recv(&sandbox.create_status(
        &sandbox.p(ValidatorId(3)),
        Height(2),
        &block.hash(),
        sandbox.s(ValidatorId(3)),
    ));

    sandbox.add_time(Duration::from_millis(BLOCK_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.a(ValidatorId(3)),
        &sandbox.create_block_request(
            &sandbox.p(ValidatorId(0)),
            &sandbox.p(ValidatorId(3)),
            Height(1),
            sandbox.s(ValidatorId(0)),
        ),
    );
    sandbox.recv(&tx);

    sandbox.recv(&sandbox.create_block_response(
        &sandbox.p(ValidatorId(3)),
        &sandbox.p(ValidatorId(0)),
        block.clone(),
        vec![precommit_1, precommit_2, precommit_3],
        &[tx.hash()],
        sandbox.s(ValidatorId(3)),
    ));

    sandbox.assert_state(Height(2), Round(1));
    sandbox.broadcast(&sandbox.create_status(
        &sandbox.p(ValidatorId(0)),
        Height(2),
        &block.hash(),
        sandbox.s(ValidatorId(0)),
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

    let block = BlockBuilder::new(&sandbox)
        .with_tx_hash(&tx.hash())
        .with_state_hash(&sandbox.compute_state_hash(&[tx.raw().clone()]))
        .build();

    let precommit_1 = sandbox.create_precommit(
        ValidatorId(1),
        Height(1),
        Round(1),
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(ValidatorId(1)),
    );
    let precommit_2 = sandbox.create_precommit(
        ValidatorId(2),
        Height(1),
        Round(1),
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(ValidatorId(2)),
    );
    let precommit_3 = sandbox.create_precommit(
        ValidatorId(3),
        Height(1),
        Round(1),
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(ValidatorId(3)),
    );

    sandbox.recv(&sandbox.create_status(
        &sandbox.p(ValidatorId(3)),
        Height(2),
        &block.hash(),
        sandbox.s(ValidatorId(3)),
    ));

    sandbox.add_time(Duration::from_millis(BLOCK_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.a(ValidatorId(3)),
        &sandbox.create_block_request(
            &sandbox.p(ValidatorId(0)),
            &sandbox.p(ValidatorId(3)),
            Height(1),
            sandbox.s(ValidatorId(0)),
        ),
    );

    sandbox.recv(&sandbox.create_block_response(
        &sandbox.p(ValidatorId(3)),
        &sandbox.p(ValidatorId(0)),
        block.clone(),
        vec![precommit_1, precommit_2, precommit_3],
        &[tx.hash()],
        sandbox.s(ValidatorId(3)),
    ));

    sandbox.add_time(Duration::from_millis(TRANSACTIONS_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.a(ValidatorId(3)),
        &sandbox.create_transactions_request(
            &sandbox.p(ValidatorId(0)),
            &sandbox.p(ValidatorId(3)),
            &[tx.hash()],
            sandbox.s(ValidatorId(0)),
        ),
    );

    sandbox.recv(&sandbox.create_transactions_response(
        &sandbox.p(ValidatorId(3)),
        &sandbox.p(ValidatorId(0)),
        vec![tx.raw().clone()],
        sandbox.s(ValidatorId(3)),
    ));

    sandbox.assert_state(Height(2), Round(1));
    sandbox.broadcast(&sandbox.create_status(
        &sandbox.p(ValidatorId(0)),
        Height(2),
        &block.hash(),
        sandbox.s(ValidatorId(0)),
    ));
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

    let block = BlockBuilder::new(&sandbox)
        .with_txs_hashes(&[tx1.hash(), tx2.hash()])
        .with_state_hash(&sandbox.compute_state_hash(&[tx1.raw().clone(), tx2.raw().clone()]))
        .build();

    let precommit_1 = sandbox.create_precommit(
        ValidatorId(1),
        Height(1),
        Round(1),
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(ValidatorId(1)),
    );
    let precommit_2 = sandbox.create_precommit(
        ValidatorId(2),
        Height(1),
        Round(1),
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(ValidatorId(2)),
    );
    let precommit_3 = sandbox.create_precommit(
        ValidatorId(3),
        Height(1),
        Round(1),
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(ValidatorId(3)),
    );

    sandbox.recv(&sandbox.create_status(
        &sandbox.p(ValidatorId(3)),
        Height(2),
        &block.hash(),
        sandbox.s(ValidatorId(3)),
    ));

    sandbox.add_time(Duration::from_millis(BLOCK_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.a(ValidatorId(3)),
        &sandbox.create_block_request(
            &sandbox.p(ValidatorId(0)),
            &sandbox.p(ValidatorId(3)),
            Height(1),
            sandbox.s(ValidatorId(0)),
        ),
    );

    // Invalid transactions order.
    sandbox.recv(&sandbox.create_block_response(
        &sandbox.p(ValidatorId(3)),
        &sandbox.p(ValidatorId(0)),
        block.clone(),
        vec![precommit_1, precommit_2, precommit_3],
        &[tx2.hash(), tx1.hash()],
        sandbox.s(ValidatorId(3)),
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

    let block1 = BlockBuilder::new(&sandbox)
        .with_tx_hash(&tx.hash())
        .with_state_hash(&sandbox.compute_state_hash(&[tx.raw().clone()]))
        .build();

    let block2 = BlockBuilder::new(&sandbox).build();

    let precommit_1 = sandbox.create_precommit(
        ValidatorId(1),
        Height(1),
        Round(1),
        &propose.hash(),
        &block1.hash(),
        sandbox.time().into(),
        sandbox.s(ValidatorId(1)),
    );
    let precommit_2 = sandbox.create_precommit(
        ValidatorId(2),
        Height(1),
        Round(1),
        &propose.hash(),
        &block1.hash(),
        sandbox.time().into(),
        sandbox.s(ValidatorId(2)),
    );
    // Precommit with invalid block hash.
    let precommit_for_other_block = sandbox.create_precommit(
        ValidatorId(3),
        Height(1),
        Round(1),
        &propose.hash(),
        &block2.hash(),
        sandbox.time().into(),
        sandbox.s(ValidatorId(3)),
    );

    sandbox.recv(&sandbox.create_status(
        &sandbox.p(ValidatorId(3)),
        Height(2),
        &block1.hash(),
        sandbox.s(ValidatorId(3)),
    ));

    sandbox.add_time(Duration::from_millis(BLOCK_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.a(ValidatorId(3)),
        &sandbox.create_block_request(
            &sandbox.p(ValidatorId(0)),
            &sandbox.p(ValidatorId(3)),
            Height(1),
            sandbox.s(ValidatorId(0)),
        ),
    );
    sandbox.recv(&tx);

    sandbox.recv(&sandbox.create_block_response(
        &sandbox.p(ValidatorId(3)),
        &sandbox.p(ValidatorId(0)),
        block1.clone(),
        vec![precommit_1, precommit_2, precommit_for_other_block],
        &[tx.hash()],
        sandbox.s(ValidatorId(3)),
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

    let block = BlockBuilder::new(&sandbox)
        .with_txs_hashes(&[tx1.hash(), tx2.hash()])
        .with_state_hash(&sandbox.compute_state_hash(&[tx1.raw().clone(), tx2.raw().clone()]))
        .build();

    let precommit_1 = sandbox.create_precommit(
        ValidatorId(1),
        Height(1),
        Round(1),
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(ValidatorId(1)),
    );
    let precommit_2 = sandbox.create_precommit(
        ValidatorId(2),
        Height(1),
        Round(1),
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(ValidatorId(2)),
    );
    let precommit_3 = sandbox.create_precommit(
        ValidatorId(3),
        Height(1),
        Round(1),
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(ValidatorId(3)),
    );

    sandbox.recv(&sandbox.create_status(
        &sandbox.p(ValidatorId(3)),
        Height(2),
        &block.hash(),
        sandbox.s(ValidatorId(3)),
    ));

    sandbox.add_time(Duration::from_millis(BLOCK_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.a(ValidatorId(3)),
        &sandbox.create_block_request(
            &sandbox.p(ValidatorId(0)),
            &sandbox.p(ValidatorId(3)),
            Height(1),
            sandbox.s(ValidatorId(0)),
        ),
    );

    sandbox.recv(&sandbox.create_block_response(
        &sandbox.p(ValidatorId(3)),
        &sandbox.p(ValidatorId(0)),
        block.clone(),
        vec![precommit_1, precommit_2, precommit_3],
        &[tx1.hash(), tx2.hash()],
        sandbox.s(ValidatorId(3)),
    ));

    sandbox.add_time(Duration::from_millis(TRANSACTIONS_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.a(ValidatorId(3)),
        &sandbox.create_transactions_request(
            &sandbox.p(ValidatorId(0)),
            &sandbox.p(ValidatorId(3)),
            &[tx2.hash()],
            sandbox.s(ValidatorId(0)),
        ),
    );

    sandbox.recv(&sandbox.create_transactions_response(
        &sandbox.p(ValidatorId(3)),
        &sandbox.p(ValidatorId(0)),
        vec![tx2.raw().clone()],
        sandbox.s(ValidatorId(3)),
    ));

    sandbox.assert_state(Height(2), Round(1));
    sandbox.broadcast(&sandbox.create_status(
        &sandbox.p(ValidatorId(0)),
        Height(2),
        &block.hash(),
        sandbox.s(ValidatorId(0)),
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

    let block = BlockBuilder::new(&sandbox)
        .with_txs_hashes(&[tx1.hash(), tx2.hash()])
        .with_state_hash(&sandbox.compute_state_hash(&[tx1.raw().clone(), tx2.raw().clone()]))
        .build();

    let precommit_1 = sandbox.create_precommit(
        ValidatorId(1),
        Height(1),
        Round(1),
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(ValidatorId(1)),
    );
    let precommit_2 = sandbox.create_precommit(
        ValidatorId(2),
        Height(1),
        Round(1),
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(ValidatorId(2)),
    );
    let precommit_3 = sandbox.create_precommit(
        ValidatorId(3),
        Height(1),
        Round(1),
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(ValidatorId(3)),
    );

    sandbox.recv(&sandbox.create_status(
        &sandbox.p(ValidatorId(3)),
        Height(2),
        &block.hash(),
        sandbox.s(ValidatorId(3)),
    ));

    sandbox.add_time(Duration::from_millis(BLOCK_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.a(ValidatorId(3)),
        &sandbox.create_block_request(
            &sandbox.p(ValidatorId(0)),
            &sandbox.p(ValidatorId(3)),
            Height(1),
            sandbox.s(ValidatorId(0)),
        ),
    );

    sandbox.recv(&sandbox.create_block_response(
        &sandbox.p(ValidatorId(3)),
        &sandbox.p(ValidatorId(0)),
        block.clone(),
        vec![precommit_1, precommit_2, precommit_3],
        &[tx1.hash(), tx2.hash()],
        sandbox.s(ValidatorId(3)),
    ));

    sandbox.assert_state(Height(2), Round(1));
    sandbox.broadcast(&sandbox.create_status(
        &sandbox.p(ValidatorId(0)),
        Height(2),
        &block.hash(),
        sandbox.s(ValidatorId(0)),
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
        .with_tx_hashes(&[tx.hash()])
        .build();

    let block = BlockBuilder::new(&sandbox)
        .with_tx_hash(&tx.hash())
        .with_state_hash(&sandbox.compute_state_hash(&[tx.raw().clone()]))
        .build();

    sandbox.recv(&sandbox.create_status(
        &sandbox.p(ValidatorId(3)),
        Height(2),
        &block.hash(),
        sandbox.s(ValidatorId(3)),
    ));

    let precommit_1 = sandbox.create_precommit(
        ValidatorId(1),
        Height(1),
        Round(1),
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(ValidatorId(1)),
    );
    let precommit_2 = sandbox.create_precommit(
        ValidatorId(2),
        Height(1),
        Round(1),
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(ValidatorId(2)),
    );
    let precommit_3 = sandbox.create_precommit(
        ValidatorId(3),
        Height(1),
        Round(1),
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(ValidatorId(3)),
    );

    sandbox.add_time(Duration::from_millis(BLOCK_REQUEST_TIMEOUT));

    sandbox.send(
        sandbox.a(ValidatorId(3)),
        &sandbox.create_block_request(
            &sandbox.p(ValidatorId(0)),
            &sandbox.p(ValidatorId(3)),
            Height(1),
            sandbox.s(ValidatorId(0)),
        ),
    );

    sandbox.recv(&sandbox.create_block_response(
        &sandbox.p(ValidatorId(3)),
        &sandbox.p(ValidatorId(0)),
        block.clone(),
        vec![precommit_1, precommit_2, precommit_3],
        &[tx.hash()],
        sandbox.s(ValidatorId(3)),
    ));

    sandbox.recv(&propose);

    sandbox.add_time(Duration::from_millis(TRANSACTIONS_REQUEST_TIMEOUT));

    sandbox.send(
        sandbox.a(ValidatorId(2)),
        &sandbox.create_transactions_request(
            &sandbox.p(ValidatorId(0)),
            &sandbox.p(ValidatorId(2)),
            &[tx.hash()],
            sandbox.s(ValidatorId(0)),
        ),
    );

    sandbox.send(
        sandbox.a(ValidatorId(3)),
        &sandbox.create_transactions_request(
            &sandbox.p(ValidatorId(0)),
            &sandbox.p(ValidatorId(3)),
            &[tx.hash()],
            sandbox.s(ValidatorId(0)),
        ),
    );

    sandbox.recv(&sandbox.create_transactions_response(
        &sandbox.p(ValidatorId(3)),
        &sandbox.p(ValidatorId(0)),
        vec![tx.raw().clone()],
        sandbox.s(ValidatorId(3)),
    ));

    sandbox.broadcast(&make_prevote_from_propose(&sandbox, &propose));

    sandbox.broadcast(&sandbox.create_status(
        &sandbox.p(ValidatorId(0)),
        Height(2),
        &block.hash(),
        sandbox.s(ValidatorId(0)),
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

    let block = BlockBuilder::new(&sandbox)
        .with_tx_hash(&tx.hash())
        .with_state_hash(&sandbox.compute_state_hash(&[tx.raw().clone()]))
        .build();

    sandbox.recv(&sandbox.create_status(
        &sandbox.p(ValidatorId(3)),
        Height(2),
        &block.hash(),
        sandbox.s(ValidatorId(3)),
    ));

    let precommit_1 = sandbox.create_precommit(
        ValidatorId(1),
        Height(1),
        Round(1),
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(ValidatorId(1)),
    );
    let precommit_2 = sandbox.create_precommit(
        ValidatorId(2),
        Height(1),
        Round(1),
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(ValidatorId(2)),
    );
    let precommit_3 = sandbox.create_precommit(
        ValidatorId(3),
        Height(1),
        Round(1),
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(ValidatorId(3)),
    );

    sandbox.add_time(Duration::from_millis(BLOCK_REQUEST_TIMEOUT));

    sandbox.send(
        sandbox.a(ValidatorId(3)),
        &sandbox.create_block_request(
            &sandbox.p(ValidatorId(0)),
            &sandbox.p(ValidatorId(3)),
            Height(1),
            sandbox.s(ValidatorId(0)),
        ),
    );

    sandbox.recv(&sandbox.create_block_response(
        &sandbox.p(ValidatorId(3)),
        &sandbox.p(ValidatorId(0)),
        block.clone(),
        vec![
            precommit_1.clone(),
            precommit_2.clone(),
            precommit_3.clone(),
        ],
        &[tx.hash()],
        sandbox.s(ValidatorId(3)),
    ));

    sandbox.recv(&sandbox.create_block_response(
        &sandbox.p(ValidatorId(3)),
        &sandbox.p(ValidatorId(0)),
        block.clone(),
        vec![precommit_1, precommit_2, precommit_3],
        &[tx.hash()],
        sandbox.s(ValidatorId(3)),
    ));

    sandbox.add_time(Duration::from_millis(TRANSACTIONS_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.a(ValidatorId(3)),
        &sandbox.create_transactions_request(
            &sandbox.p(ValidatorId(0)),
            &sandbox.p(ValidatorId(3)),
            &[tx.hash()],
            sandbox.s(ValidatorId(0)),
        ),
    );

    sandbox.recv(&sandbox.create_transactions_response(
        &sandbox.p(ValidatorId(3)),
        &sandbox.p(ValidatorId(0)),
        vec![tx.raw().clone()],
        sandbox.s(ValidatorId(3)),
    ));

    sandbox.assert_state(Height(2), Round(1));
    sandbox.broadcast(&sandbox.create_status(
        &sandbox.p(ValidatorId(0)),
        Height(2),
        &block.hash(),
        sandbox.s(ValidatorId(0)),
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

    let block = BlockBuilder::new(&sandbox)
        .with_tx_hash(&tx.hash())
        .with_state_hash(&sandbox.compute_state_hash(&[tx.raw().clone()]))
        .build();

    sandbox.recv(&sandbox.create_status(
        &sandbox.p(ValidatorId(2)),
        Height(2),
        &block.hash(),
        sandbox.s(ValidatorId(2)),
    ));

    sandbox.recv(&sandbox.create_status(
        &sandbox.p(ValidatorId(3)),
        Height(2),
        &block.hash(),
        sandbox.s(ValidatorId(3)),
    ));

    let precommit_1 = sandbox.create_precommit(
        ValidatorId(1),
        Height(1),
        Round(1),
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(ValidatorId(1)),
    );
    let precommit_2 = sandbox.create_precommit(
        ValidatorId(2),
        Height(1),
        Round(1),
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(ValidatorId(2)),
    );
    let precommit_3 = sandbox.create_precommit(
        ValidatorId(3),
        Height(1),
        Round(1),
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(ValidatorId(3)),
    );

    sandbox.add_time(Duration::from_millis(BLOCK_REQUEST_TIMEOUT));

    sandbox.send(
        sandbox.a(ValidatorId(2)),
        &sandbox.create_block_request(
            &sandbox.p(ValidatorId(0)),
            &sandbox.p(ValidatorId(2)),
            Height(1),
            sandbox.s(ValidatorId(0)),
        ),
    );

    sandbox.recv(&sandbox.create_block_response(
        &sandbox.p(ValidatorId(3)),
        &sandbox.p(ValidatorId(0)),
        block.clone(),
        vec![precommit_1, precommit_2, precommit_3],
        &[tx.hash()],
        sandbox.s(ValidatorId(3)),
    ));

    sandbox.add_time(Duration::from_millis(TRANSACTIONS_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.a(ValidatorId(2)),
        &sandbox.create_transactions_request(
            &sandbox.p(ValidatorId(0)),
            &sandbox.p(ValidatorId(2)),
            &[tx.hash()],
            sandbox.s(ValidatorId(0)),
        ),
    );

    sandbox.add_time(Duration::from_millis(TRANSACTIONS_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.a(ValidatorId(3)),
        &sandbox.create_transactions_request(
            &sandbox.p(ValidatorId(0)),
            &sandbox.p(ValidatorId(3)),
            &[tx.hash()],
            sandbox.s(ValidatorId(0)),
        ),
    );

    sandbox.recv(&sandbox.create_transactions_response(
        &sandbox.p(ValidatorId(2)),
        &sandbox.p(ValidatorId(0)),
        vec![tx.raw().clone()],
        sandbox.s(ValidatorId(2)),
    ));

    sandbox.assert_state(Height(2), Round(1));
    sandbox.broadcast(&sandbox.create_status(
        &sandbox.p(ValidatorId(0)),
        Height(2),
        &block.hash(),
        sandbox.s(ValidatorId(0)),
    ));
}
