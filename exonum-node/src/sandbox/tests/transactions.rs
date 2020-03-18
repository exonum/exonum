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

//! Tests in this module are designed to test details of transaction handling.

use bit_vec::BitVec;
use exonum::{
    blockchain::{ProposerId, TransactionCache},
    crypto::{Hash, KeyPair, PublicKey},
    helpers::{Height, Round, ValidatorId},
    merkledb::{BinaryValue, ObjectHash},
    messages::{AnyTx, Verified},
};

use std::{collections::BTreeSet, iter::FromIterator, time::Duration};

use crate::{
    messages::{TX_RES_EMPTY_SIZE, TX_RES_PB_OVERHEAD_PAYLOAD},
    proposer::{Pool, ProposeBlock, ProposeParams, ProposeTemplate},
    sandbox::{
        sandbox_tests_helper::*,
        supervisor::{Supervisor, SupervisorService, TxConfig},
        timestamping::{
            Timestamping as _, TimestampingService, TimestampingTxGenerator, DATA_SIZE,
        },
        timestamping_sandbox, timestamping_sandbox_builder, Milliseconds, Sandbox,
    },
    state::TRANSACTIONS_REQUEST_TIMEOUT,
};
use exonum::blockchain::Blockchain;

const MAX_PROPOSE_TIMEOUT: Milliseconds = 200;
const MIN_PROPOSE_TIMEOUT: Milliseconds = 10;
const PROPOSE_THRESHOLD: u32 = 3;

fn timestamping_sandbox_with_threshold() -> Sandbox {
    let sandbox = timestamping_sandbox_builder()
        .with_consensus(|config| {
            config.max_propose_timeout = MAX_PROPOSE_TIMEOUT;
            config.min_propose_timeout = MIN_PROPOSE_TIMEOUT;
            config.propose_timeout_threshold = PROPOSE_THRESHOLD;
        })
        .build();

    // Wait for us to become the leader.
    sandbox.add_time(Duration::from_millis(sandbox.current_round_timeout()));
    sandbox.add_time(Duration::from_millis(sandbox.current_round_timeout()));
    sandbox
}

fn tx_hashes(transactions: &[Verified<AnyTx>]) -> Vec<Hash> {
    let mut hashes = transactions
        .iter()
        .map(Verified::object_hash)
        .collect::<Vec<_>>();
    hashes.sort();
    hashes
}

/// idea of the test is to verify request transaction scenario: other node requests
/// transaction from our node
#[test]
fn response_to_request_txs() {
    let sandbox = timestamping_sandbox();

    let tx = gen_timestamping_tx();
    sandbox.recv(&tx);

    sandbox.recv(&Sandbox::create_transactions_request(
        sandbox.public_key(ValidatorId(1)),
        sandbox.public_key(ValidatorId(0)),
        vec![tx.object_hash()],
        sandbox.secret_key(ValidatorId(1)),
    ));

    sandbox.send(
        sandbox.public_key(ValidatorId(1)),
        &Sandbox::create_transactions_response(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(1)),
            vec![tx],
            sandbox.secret_key(ValidatorId(0)),
        ),
    );
}

#[test]
fn empty_tx_request() {
    let sandbox = timestamping_sandbox();

    sandbox.recv(&Sandbox::create_transactions_request(
        sandbox.public_key(ValidatorId(1)),
        sandbox.public_key(ValidatorId(0)),
        vec![],
        sandbox.secret_key(ValidatorId(1)),
    ));
}

// if tx was received after execute but before commit it produce conflict patch.
// Test case:
// 1. add tx
// 2. create and execute propose
// 3. add other tx
// 4. commit propose.
#[test]
fn tx_pool_size_overflow() {
    let mut tx_gen = TimestampingTxGenerator::new(DATA_SIZE);
    let tx1 = tx_gen.next().unwrap();
    let tx2 = tx_gen.next().unwrap();
    let sandbox = timestamping_sandbox();

    sandbox.recv(&tx1);

    let propose = sandbox.create_propose(
        ValidatorId(2),
        Height(1),
        Round(1),
        sandbox.last_hash(),
        vec![tx1.object_hash()],
        sandbox.secret_key(ValidatorId(2)),
    );

    let mut block = sandbox.create_block(&[tx1]);
    block.add_header::<ProposerId>(ValidatorId(2));
    block.height = Height(1);

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
    sandbox.assert_lock(NOT_LOCKED, None);
    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(2),
        Height(1),
        Round(1),
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(2)),
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
    sandbox.assert_lock(Round(1), Some(propose.object_hash()));
    sandbox.recv(&tx2);
    sandbox.assert_tx_cache_len(2);
    sandbox.assert_pool_len(0);

    sandbox.recv(&sandbox.create_precommit(
        ValidatorId(1),
        Height(1),
        Round(1),
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(1)),
    ));
    sandbox.recv(&sandbox.create_precommit(
        ValidatorId(2),
        Height(1),
        Round(1),
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(2)),
    ));

    // The first tx should be committed; the second tx should remain in cache.
    sandbox.assert_tx_cache_len(1);
    sandbox.assert_pool_len(0);
    sandbox.broadcast(&Sandbox::create_status(
        sandbox.public_key(ValidatorId(0)),
        Height(2),
        block.object_hash(),
        1,
        sandbox.secret_key(ValidatorId(0)),
    ));
}

#[test]
fn duplicate_tx_in_pool() {
    let sandbox = timestamping_sandbox();

    let mut tx_gen = TimestampingTxGenerator::new(DATA_SIZE);
    let tx1 = tx_gen.next().unwrap();
    let tx2 = tx_gen.next().unwrap();

    let propose = ProposeBuilder::new(&sandbox)
        .with_tx_hashes(&[tx1.object_hash()])
        .build();

    sandbox.recv(&propose);
    sandbox.add_time(Duration::from_millis(TRANSACTIONS_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.public_key(ValidatorId(2)),
        &Sandbox::create_transactions_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(2)),
            vec![tx1.object_hash()],
            sandbox.secret_key(ValidatorId(0)),
        ),
    );
    sandbox.recv(&tx1);
    sandbox.broadcast(&make_prevote_from_propose(&sandbox, &propose));

    sandbox.recv(&tx2);

    sandbox.recv(&Sandbox::create_transactions_response(
        sandbox.public_key(ValidatorId(2)),
        sandbox.public_key(ValidatorId(0)),
        vec![tx1],
        sandbox.secret_key(ValidatorId(2)),
    ));
}

#[test]
fn response_size_larger_than_max_message_len() {
    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();
    // Create 4 transactions.
    // The size of the fourth transactions is 1 more than size of the first three.
    let tx1 = gen_timestamping_tx();
    let tx2 = gen_timestamping_tx();
    let tx3 = gen_timestamping_tx();
    let tx4 = TimestampingTxGenerator::new(DATA_SIZE + 1).next().unwrap();

    assert_eq!(
        tx1.as_raw().to_bytes().len() + tx2.as_raw().to_bytes().len() + 1,
        tx3.as_raw().to_bytes().len() + tx4.as_raw().to_bytes().len()
    );

    // Create new config. Set the size of the message to a size
    // that is exactly equal to the message to send the first two transactions.
    let tx_cfg = {
        let mut consensus_cfg = sandbox.cfg();
        consensus_cfg.max_message_len = (TX_RES_EMPTY_SIZE
            + TX_RES_PB_OVERHEAD_PAYLOAD * 2
            + tx1.to_bytes().len()
            + tx2.to_bytes().len()) as u32;
        let actual_from = sandbox.current_epoch().next();

        TxConfig::create_signed(
            sandbox.public_key(ValidatorId(0)),
            &consensus_cfg.into_bytes(),
            actual_from,
            sandbox.secret_key(ValidatorId(0)),
        )
    };

    add_one_height_with_transactions(&sandbox, &sandbox_state, &[tx_cfg]);

    sandbox.recv(&tx1);
    sandbox.recv(&tx2);

    // Send request with `tx1` and `tx2`.
    sandbox.recv(&Sandbox::create_transactions_request(
        sandbox.public_key(ValidatorId(1)),
        sandbox.public_key(ValidatorId(0)),
        vec![tx1.object_hash(), tx2.object_hash()],
        sandbox.secret_key(ValidatorId(1)),
    ));

    // Receive response with `tx1` and `tx2`.
    sandbox.send(
        sandbox.public_key(ValidatorId(1)),
        &Sandbox::create_transactions_response(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(1)),
            vec![tx1, tx2],
            sandbox.secret_key(ValidatorId(0)),
        ),
    );

    sandbox.recv(&tx3);
    sandbox.recv(&tx4);

    // Send request with `tx3` and `tx4`.
    sandbox.recv(&Sandbox::create_transactions_request(
        sandbox.public_key(ValidatorId(1)),
        sandbox.public_key(ValidatorId(0)),
        vec![tx3.object_hash(), tx4.object_hash()],
        sandbox.secret_key(ValidatorId(1)),
    ));

    // Receive separate responses with `tx3` and `tx4`.
    sandbox.send(
        sandbox.public_key(ValidatorId(1)),
        &Sandbox::create_transactions_response(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(1)),
            vec![tx3],
            sandbox.secret_key(ValidatorId(0)),
        ),
    );

    sandbox.send(
        sandbox.public_key(ValidatorId(1)),
        &Sandbox::create_transactions_response(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(1)),
            vec![tx4],
            sandbox.secret_key(ValidatorId(0)),
        ),
    );
}

/// idea of the test is to
///  - become a leader
///  - receive tx
///  - broadcast propose & prevote
///  - receive 2 prevotes
///  - broadcast precommit
///  - receive requests for tx, propose, prevote & precommit
///   - send responses to above requests
#[test]
#[allow(clippy::too_many_lines)] // fine for tests
fn respond_to_request_tx_propose_prevotes_precommits() {
    let sandbox = timestamping_sandbox();

    // round happens to make us a leader
    sandbox.add_time(Duration::from_millis(sandbox.current_round_timeout()));
    sandbox.add_time(Duration::from_millis(sandbox.current_round_timeout()));
    assert!(sandbox.is_leader());
    sandbox.assert_state(Height(1), Round(3));

    // option: with transaction
    let tx = gen_timestamping_tx();
    sandbox.recv(&tx);
    sandbox.add_time(Duration::from_millis(PROPOSE_TIMEOUT));

    let propose = ProposeBuilder::new(&sandbox)
        .with_tx_hashes(&[tx.object_hash()]) //ordinary propose, but with this unreceived tx
        .build();
    let block = sandbox.create_block(&[tx.clone()]);

    let precommit_1 = sandbox.create_precommit(
        ValidatorId(1),
        Height(1),
        Round(3),
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(1)),
    );
    let precommit_2 = sandbox.create_precommit(
        ValidatorId(2),
        Height(1),
        Round(3),
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(2)),
    );

    // ok, we are leader

    sandbox.broadcast(&propose);
    sandbox.broadcast(&make_prevote_from_propose(&sandbox, &propose));

    // respond to RequestPropose
    sandbox.recv(&Sandbox::create_propose_request(
        sandbox.public_key(ValidatorId(3)),
        sandbox.public_key(ValidatorId(0)),
        Height(1),
        propose.object_hash(),
        sandbox.secret_key(ValidatorId(3)),
    ));

    sandbox.send(sandbox.public_key(ValidatorId(3)), &propose);

    // respond to RequestPrevotes
    let mut validators = BitVec::from_elem(sandbox.validators().len(), false);
    validators.set(ValidatorId(3).into(), true);

    sandbox.recv(&Sandbox::create_prevote_request(
        sandbox.public_key(ValidatorId(3)),
        sandbox.public_key(ValidatorId(0)),
        Height(1),
        Round(3),
        propose.object_hash(),
        validators,
        sandbox.secret_key(ValidatorId(3)),
    ));

    sandbox.send(
        sandbox.public_key(ValidatorId(3)),
        &make_prevote_from_propose(&sandbox, &propose),
    );

    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(1),
        Height(1),
        Round(3),
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(1)),
    ));
    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(2),
        Height(1),
        Round(3),
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(2)),
    ));

    sandbox.broadcast(&sandbox.create_precommit(
        ValidatorId(0),
        Height(1),
        Round(3),
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(0)),
    ));

    sandbox.recv(&precommit_1);
    sandbox.recv(&precommit_2);

    sandbox.assert_state(Height(2), Round(1));
    sandbox.check_broadcast_status(Height(2), block.object_hash());

    // respond to RequestTransactions
    sandbox.recv(&Sandbox::create_transactions_request(
        sandbox.public_key(ValidatorId(1)),
        sandbox.public_key(ValidatorId(0)),
        vec![tx.object_hash()],
        sandbox.secret_key(ValidatorId(1)),
    ));

    sandbox.send(
        sandbox.public_key(ValidatorId(1)),
        &Sandbox::create_transactions_response(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(1)),
            vec![tx],
            sandbox.secret_key(ValidatorId(0)),
        ),
    );

    // respond to RequestPropose negative
    sandbox.recv(&Sandbox::create_propose_request(
        sandbox.public_key(ValidatorId(3)),
        sandbox.public_key(ValidatorId(0)),
        Height(1),
        propose.object_hash(),
        sandbox.secret_key(ValidatorId(3)),
    ));

    // here state.height() is 1 already, so requestPropose with height zero is ignored

    // respond to RequestPrevotes negative
    let mut validators = BitVec::from_elem(sandbox.validators().len(), false);
    validators.set(ValidatorId(3).into(), true);

    sandbox.recv(&Sandbox::create_prevote_request(
        sandbox.public_key(ValidatorId(3)),
        sandbox.public_key(ValidatorId(0)),
        Height(1),
        Round(3),
        propose.object_hash(),
        validators,
        sandbox.secret_key(ValidatorId(3)),
    ));

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
        .with_tx_hashes(&[tx.object_hash()]) //ordinary propose, but with this unreceived tx
        .build();

    sandbox.recv(&propose);
    sandbox.broadcast(&make_prevote_from_propose(&sandbox, &propose));
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
        .with_tx_hashes(&[tx.object_hash()]) //ordinary propose, but with this unreceived tx
        .build();

    sandbox.recv(&propose);
    sandbox.add_time(Duration::from_millis(sandbox.current_round_timeout() - 1));

    sandbox.send(
        sandbox.public_key(ValidatorId(2)),
        &Sandbox::create_transactions_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(2)),
            vec![tx.object_hash()],
            sandbox.secret_key(ValidatorId(0)),
        ),
    );

    sandbox.add_time(Duration::from_millis(0));

    sandbox.recv(&sandbox.create_prevote(
        ValidatorId(3),
        Height(1),
        Round(1),
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(3)),
    ));

    sandbox.add_time(Duration::from_millis(sandbox.current_round_timeout() - 1));

    sandbox.send(
        sandbox.public_key(ValidatorId(3)),
        &Sandbox::create_transactions_request(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(3)),
            vec![tx.object_hash()],
            sandbox.secret_key(ValidatorId(0)),
        ),
    );

    sandbox.add_time(Duration::from_millis(0));
}

#[test]
fn regular_propose_when_no_transaction_pressure() {
    let sandbox = timestamping_sandbox_with_threshold();

    // Generate and receive some transactions (fewer than the threshold).
    let transactions = TimestampingTxGenerator::new(64)
        .take(PROPOSE_THRESHOLD as usize - 1)
        .collect::<Vec<_>>();

    for tx in &transactions {
        sandbox.recv(tx);
    }

    // Proposal is expected to arrive after maximum timeout as we're still not over the threshold.
    sandbox.add_time(Duration::from_millis(MAX_PROPOSE_TIMEOUT));

    let propose = ProposeBuilder::new(&sandbox)
        .with_tx_hashes(&tx_hashes(&transactions))
        .build();

    sandbox.broadcast(&propose);
    sandbox.broadcast(&make_prevote_from_propose(&sandbox, &propose));
}

#[test]
fn expedited_propose_on_transaction_pressure() {
    let sandbox = timestamping_sandbox_with_threshold();

    // Generate and receive some transactions (at the threshold).
    let transactions = TimestampingTxGenerator::new(64)
        .take(PROPOSE_THRESHOLD as usize)
        .collect::<Vec<_>>();

    for tx in &transactions {
        sandbox.recv(tx);
    }

    // Proposal should be expedited and is expected to arrive after minimum timeout.
    sandbox.add_time(Duration::from_millis(MIN_PROPOSE_TIMEOUT));

    let propose = ProposeBuilder::new(&sandbox)
        .with_tx_hashes(&tx_hashes(&transactions))
        .build();

    sandbox.broadcast(&propose);
    sandbox.broadcast(&make_prevote_from_propose(&sandbox, &propose));
}

#[test]
fn valid_txs_are_broadcast() {
    let sandbox = timestamping_sandbox();
    let tx = gen_timestamping_tx();
    sandbox
        .api_sender
        .broadcast_transaction_blocking(tx.clone())
        .unwrap();
    sandbox.process_events();
    sandbox.broadcast(&tx);
}

#[test]
fn incorrect_txs_are_not_broadcast() {
    let sandbox = timestamping_sandbox();
    let incorrect_tx = gen_incorrect_tx();
    sandbox
        .api_sender
        .broadcast_transaction_blocking(incorrect_tx)
        .unwrap();
    sandbox.process_events();
    // If the transaction is broadcast, the sandbox will panic on drop.
}

#[test]
fn executing_block_does_not_lead_to_amnesia() {
    let sandbox = timestamping_sandbox();
    let tx = gen_timestamping_tx();
    let tx_hash = tx.object_hash();
    sandbox.recv(&tx);

    let propose = ProposeBuilder::new(&sandbox)
        .with_tx_hashes(&[tx_hash])
        .build();
    let block = sandbox.create_block(&[tx.clone()]);

    let prevotes = (1_u16..3).map(|i| {
        let validator = ValidatorId(i);
        sandbox.create_prevote(
            validator,
            Height(1),
            Round(1),
            propose.object_hash(),
            NOT_LOCKED,
            sandbox.secret_key(validator),
        )
    });
    for prevote in prevotes {
        sandbox.recv(&prevote);
    }
    sandbox.recv(&propose);

    sandbox.broadcast(&sandbox.create_prevote(
        ValidatorId(0),
        Height(1),
        Round(1),
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(0)),
    ));

    // Node should execute the block by this point.
    assert!(sandbox.node_state().block(&block.object_hash()).is_some());
    sandbox.broadcast(&sandbox.create_precommit(
        ValidatorId(0),
        Height(1),
        Round(1),
        propose.object_hash(),
        block.object_hash(),
        sandbox.time().into(),
        sandbox.secret_key(ValidatorId(0)),
    ));

    // Node should not forget about the transaction.
    assert!(sandbox.node_state().tx_cache().contains_key(&tx_hash));
    sandbox.recv(&Sandbox::create_transactions_request(
        sandbox.public_key(ValidatorId(3)),
        sandbox.public_key(ValidatorId(0)),
        vec![tx_hash],
        sandbox.secret_key(ValidatorId(3)),
    ));
    sandbox.send(
        sandbox.public_key(ValidatorId(3)),
        &Sandbox::create_transactions_response(
            sandbox.public_key(ValidatorId(0)),
            sandbox.public_key(ValidatorId(3)),
            vec![tx],
            sandbox.secret_key(ValidatorId(0)),
        ),
    );
}

#[derive(Debug)]
struct WhitelistProposer {
    key: PublicKey,
}

impl WhitelistProposer {
    fn new(key: PublicKey) -> Self {
        Self { key }
    }
}

impl ProposeBlock for WhitelistProposer {
    fn propose_block(&mut self, pool: Pool<'_>, params: ProposeParams<'_>) -> ProposeTemplate {
        let tx_hashes = pool.transactions().filter_map(|(tx_hash, tx)| {
            if tx.author() == self.key {
                Some(tx_hash)
            } else {
                None
            }
        });
        let tx_limit = params.consensus_config().txs_block_limit;
        let tx_hashes = tx_hashes.take(tx_limit as usize);
        ProposeTemplate::ordinary(tx_hashes)
    }
}

#[test]
fn propose_with_custom_logic() {
    let keypair = KeyPair::random();
    let sandbox = timestamping_sandbox_builder()
        .with_proposer(WhitelistProposer::new(keypair.public_key()))
        .build();

    let good_tx = keypair.timestamp(TimestampingService::ID, vec![1, 2, 3]);
    let other_good_tx = keypair.timestamp(TimestampingService::ID, vec![1]);
    let bad_tx = KeyPair::random().timestamp(TimestampingService::ID, vec![1, 2, 3]);
    sandbox.recv(&good_tx);
    sandbox.recv(&other_good_tx);
    sandbox.recv(&bad_tx);

    while !sandbox.is_leader() {
        sandbox.add_time(Duration::from_millis(sandbox.current_round_timeout()));
    }
    assert!(sandbox.is_leader());
    sandbox.add_time(Duration::from_millis(sandbox.current_round_timeout()));

    let tx_hashes = BTreeSet::from_iter(vec![good_tx.object_hash(), other_good_tx.object_hash()]);
    let propose = sandbox.create_propose(
        ValidatorId(0),
        Height(1),
        Round(3),
        sandbox.last_hash(),
        tx_hashes,
        sandbox.secret_key(ValidatorId(0)),
    );
    sandbox.broadcast(&propose);
    let prevote = sandbox.create_prevote(
        ValidatorId(0),
        Height(1),
        Round(3),
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(0)),
    );
    sandbox.broadcast(&prevote);
}

#[test]
fn custom_proposer_does_not_influence_external_proposes() {
    let keypair = KeyPair::random();
    let sandbox = timestamping_sandbox_builder()
        .with_proposer(WhitelistProposer::new(keypair.public_key()))
        .build();

    let good_tx = keypair.timestamp(TimestampingService::ID, vec![1, 2, 3]);
    let bad_tx = KeyPair::random().timestamp(TimestampingService::ID, vec![1, 2, 3]);
    sandbox.recv(&good_tx);
    sandbox.recv(&bad_tx);

    let propose = sandbox.create_propose(
        ValidatorId(2),
        Height(1),
        Round(1),
        sandbox.last_hash(),
        vec![good_tx.object_hash(), bad_tx.object_hash()],
        sandbox.secret_key(ValidatorId(2)),
    );
    sandbox.recv(&propose);

    let prevote = sandbox.create_prevote(
        ValidatorId(0),
        Height(1),
        Round(1),
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(0)),
    );
    sandbox.broadcast(&prevote);

    let block = sandbox.create_block(&[good_tx, bad_tx]);
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
    sandbox.broadcast(&Sandbox::create_status(
        sandbox.public_key(ValidatorId(0)),
        Height(2),
        block.object_hash(),
        0,
        sandbox.secret_key(ValidatorId(0)),
    ));
}

#[test]
fn not_proposing_incorrect_transactions() {
    let sandbox = timestamping_sandbox();
    add_one_height(&sandbox, &SandboxState::new());

    let tx = gen_timestamping_tx();
    let stop_service_tx =
        KeyPair::random().stop_service(SupervisorService::ID, TimestampingService::ID);

    // Send both transactions to the sandbox.
    sandbox.recv(&tx);
    sandbox.recv(&stop_service_tx);

    // Accept the block with the transaction stopping the TS service, but not `tx`
    // (say, we didn't manage to broadcast it).
    let stop_propose = sandbox.create_propose(
        ValidatorId(3),
        Height(2),
        Round(1),
        sandbox.last_hash(),
        vec![stop_service_tx.object_hash()],
        sandbox.secret_key(ValidatorId(3)),
    );
    let block = sandbox.create_block(&[stop_service_tx]);

    let stop_precommits = (1..4).map(|i| {
        let validator = ValidatorId(i);
        sandbox.create_precommit(
            validator,
            Height(2),
            Round(1),
            stop_propose.object_hash(),
            block.object_hash(),
            sandbox.time().into(),
            sandbox.secret_key(validator),
        )
    });

    sandbox.recv(&stop_propose);
    sandbox.broadcast(&sandbox.create_prevote(
        ValidatorId(0),
        Height(2),
        Round(1),
        stop_propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(0)),
    ));

    for precommit in stop_precommits {
        sandbox.recv(&precommit);
    }
    sandbox.broadcast(&sandbox.create_our_status(Height(3), Height(3), 1));

    // The transaction is incorrect now.
    let snapshot = sandbox.blockchain().snapshot();
    assert!(Blockchain::check_tx(&snapshot, &tx).is_err());

    // Create a proposal.
    assert!(sandbox.is_leader());
    sandbox.add_time(Duration::from_millis(sandbox.current_round_timeout()));

    let propose = sandbox.create_propose(
        ValidatorId(0),
        Height(3),
        Round(1),
        sandbox.last_hash(),
        vec![], // `tx` is incorrect and should not be included into the proposal.
        sandbox.secret_key(ValidatorId(0)),
    );
    sandbox.broadcast(&propose);
    let prevote = sandbox.create_prevote(
        ValidatorId(0),
        Height(3),
        Round(1),
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(0)),
    );
    sandbox.broadcast(&prevote);
}
