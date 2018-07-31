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

//! Tests in this module are designed to test details of transaction handling.

use bit_vec::BitVec;

use std::time::Duration;

use crypto::{gen_keypair, CryptoHash};
use helpers::Round;
use messages::{
    Message, Precommit, Prevote, PrevotesRequest, ProposeRequest, TransactionsRequest,
    TransactionsResponse,
};
use node::state::TRANSACTIONS_REQUEST_TIMEOUT;
use sandbox::{
    config_updater::TxConfig, sandbox::timestamping_sandbox, sandbox_tests_helper::*,
    timestamping::{TimestampTx, TimestampingTxGenerator},
};

/// idea of the test is to verify request transaction scenario: other node requests
/// transaction from our node
// TODO: Add scenario with manually created and signed tx. (ECR-1627)
#[test]
fn response_to_request_txs() {
    let sandbox = timestamping_sandbox();

    let tx = gen_timestamping_tx();
    sandbox.recv(&tx);

    sandbox.recv(&TransactionsRequest::new(
        &sandbox.p(VALIDATOR_1),
        &sandbox.p(VALIDATOR_0),
        &[tx.hash()],
        sandbox.s(VALIDATOR_1),
    ));

    sandbox.send(
        sandbox.a(VALIDATOR_1),
        &TransactionsResponse::new(
            &sandbox.p(VALIDATOR_0),
            &sandbox.p(VALIDATOR_1),
            vec![tx.raw().clone()],
            sandbox.s(VALIDATOR_0),
        ),
    );
}

#[test]
fn empty_tx_request() {
    let sandbox = timestamping_sandbox();

    sandbox.recv(&TransactionsRequest::new(
        &sandbox.p(VALIDATOR_1),
        &sandbox.p(VALIDATOR_0),
        &[],
        sandbox.s(VALIDATOR_1),
    ));
}

#[test]
fn duplicate_tx_in_pool() {
    let sandbox = timestamping_sandbox();

    let mut tx_gen = TimestampingTxGenerator::new(64);
    let tx1 = tx_gen.next().unwrap();
    let tx2 = tx_gen.next().unwrap();

    let propose = ProposeBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .with_tx_hashes(&[tx1.hash()])
        .build();

    sandbox.recv(&propose);
    sandbox.add_time(Duration::from_millis(TRANSACTIONS_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.a(VALIDATOR_2),
        &TransactionsRequest::new(
            &sandbox.p(VALIDATOR_0),
            &sandbox.p(VALIDATOR_2),
            &[tx1.hash()],
            &sandbox.s(VALIDATOR_0),
        ),
    );
    sandbox.recv(&tx1);
    sandbox.broadcast(&make_prevote_from_propose(&sandbox, &propose));

    sandbox.recv(&tx2);

    sandbox.recv(&TransactionsResponse::new(
        &sandbox.p(VALIDATOR_2),
        &sandbox.p(VALIDATOR_0),
        vec![tx1.raw().clone()],
        sandbox.s(VALIDATOR_2),
    ));
}

#[test]
#[should_panic(expected = "Send unexpected message Request(TransactionsRequest")]
fn incorrect_tx_in_request() {
    let sandbox = timestamping_sandbox();

    let (pub_key, _) = gen_keypair();
    let (_, sec_key) = gen_keypair();
    let data = vec![0; 64];
    let tx0 = TimestampTx::new(&pub_key, &data, &sec_key);

    let propose = ProposeBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .with_tx_hashes(&[tx0.hash()])
        .build();

    // Receive propose with unknown transaction `tx0`.
    sandbox.recv(&propose);
    // After `TRANSACTIONS_REQUEST_TIMEOUT` node send request with `tx0`.
    sandbox.add_time(Duration::from_millis(TRANSACTIONS_REQUEST_TIMEOUT));
    sandbox.send(
        sandbox.a(VALIDATOR_2),
        &TransactionsRequest::new(
            &sandbox.p(VALIDATOR_0),
            &sandbox.p(VALIDATOR_2),
            &[tx0.hash()],
            &sandbox.s(VALIDATOR_0),
        ),
    );

    // Receive response with invalid `tx0`.
    sandbox.recv(&TransactionsResponse::new(
        &sandbox.p(VALIDATOR_2),
        &sandbox.p(VALIDATOR_0),
        vec![tx0.raw().clone()],
        sandbox.s(VALIDATOR_2),
    ));

    add_one_height(&sandbox, &SandboxState::new());

    let tx1 = gen_timestamping_tx();
    let propose = ProposeBuilder::new(&sandbox)
        .with_tx_hashes(&[tx0.hash(), tx1.hash()])
        .build();

    sandbox.recv(&tx1);
    // Receive new propose with `tx0` and `tx1`.
    // `tx1` - valid and after receiving go to the pool.
    // `tx0` - invalid and after receiving should be dismissed.
    sandbox.recv(&propose);

    sandbox.add_time(Duration::from_millis(TRANSACTIONS_REQUEST_TIMEOUT));
}

#[test]
fn response_size_larger_than_max_message_len() {
    use crypto::{PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH};
    use messages::HEADER_LENGTH;
    use storage::StorageValue;

    const EMPTY_RESPONSE_SIZE: usize =
        (HEADER_LENGTH + SIGNATURE_LENGTH + 2 * PUBLIC_KEY_LENGTH + 8);

    let sandbox = timestamping_sandbox();
    let sandbox_state = SandboxState::new();

    // Create 4 transactions.
    // The size of the fourth transactions is 1 more than size of the first three.
    let tx1 = gen_timestamping_tx();
    let tx2 = gen_timestamping_tx();
    let tx3 = gen_timestamping_tx();
    let (pub_key, sec_key) = gen_keypair();
    let data = vec![0; 65];
    let tx4 = TimestampTx::new(&pub_key, &data, &sec_key);

    assert_eq!(
        tx1.raw().len() + tx2.raw().len() + 1,
        tx3.raw().len() + tx4.raw().len()
    );

    // Create new config. Set the size of the message to a size
    // that is exactly equal to the message to send the first two transactions.
    let tx_cfg = {
        let mut consensus_cfg = sandbox.cfg();
        consensus_cfg.consensus.max_message_len =
            (EMPTY_RESPONSE_SIZE + tx1.raw().len() + tx2.raw().len()) as u32;
        consensus_cfg.actual_from = sandbox.current_height().next();
        consensus_cfg.previous_cfg_hash = sandbox.cfg().hash();

        TxConfig::new(
            &sandbox.p(VALIDATOR_0),
            &consensus_cfg.clone().into_bytes(),
            consensus_cfg.actual_from,
            sandbox.s(VALIDATOR_0),
        )
    };

    add_one_height_with_transactions(&sandbox, &sandbox_state, &[tx_cfg.raw().clone()]);

    sandbox.recv(&tx1);
    sandbox.recv(&tx2);

    // Send request with `tx1` and `tx2`.
    sandbox.recv(&TransactionsRequest::new(
        &sandbox.p(VALIDATOR_1),
        &sandbox.p(VALIDATOR_0),
        &[tx1.hash(), tx2.hash()],
        sandbox.s(VALIDATOR_1),
    ));

    // Receive response with `tx1` and `tx2`.
    sandbox.send(
        sandbox.a(VALIDATOR_1),
        &TransactionsResponse::new(
            &sandbox.p(VALIDATOR_0),
            &sandbox.p(VALIDATOR_1),
            vec![tx1.raw().clone(), tx2.raw().clone()],
            sandbox.s(VALIDATOR_0),
        ),
    );

    sandbox.recv(&tx3);
    sandbox.recv(&tx4);

    // Send request with `tx3` and `tx4`.
    sandbox.recv(&TransactionsRequest::new(
        &sandbox.p(VALIDATOR_1),
        &sandbox.p(VALIDATOR_0),
        &[tx3.hash(), tx4.hash()],
        sandbox.s(VALIDATOR_1),
    ));

    // Receive separate responses with `tx3` and `tx4`.
    sandbox.send(
        sandbox.a(VALIDATOR_1),
        &TransactionsResponse::new(
            &sandbox.p(VALIDATOR_0),
            &sandbox.p(VALIDATOR_1),
            vec![tx3.raw().clone()],
            sandbox.s(VALIDATOR_0),
        ),
    );

    sandbox.send(
        sandbox.a(VALIDATOR_1),
        &TransactionsResponse::new(
            &sandbox.p(VALIDATOR_0),
            &sandbox.p(VALIDATOR_1),
            vec![tx4.raw().clone()],
            sandbox.s(VALIDATOR_0),
        ),
    );
}

/// idea of the test is to
///  - become e leader
///  - receive tx
///  - broadcast propose & prevote
///  - receive 2 prevotes
///  - broadcast precommit
///  - receive requests for tx, propose, prevote & precommit
///   - send responses to above requests
#[test]
fn respond_to_request_tx_propose_prevotes_precommits() {
    let sandbox = timestamping_sandbox();

    {
        // round happens to make us a leader
        sandbox.add_time(Duration::from_millis(sandbox.round_timeout()));
        sandbox.add_time(Duration::from_millis(sandbox.round_timeout()));
        assert!(sandbox.is_leader());
        sandbox.assert_state(HEIGHT_ONE, ROUND_THREE);
    }

    // option: with transaction
    let tx = gen_timestamping_tx();
    sandbox.recv(&tx);

    sandbox.add_time(Duration::from_millis(PROPOSE_TIMEOUT));

    let propose = ProposeBuilder::new(&sandbox)
        .with_tx_hashes(&[tx.hash()]) //ordinary propose, but with this unreceived tx
        .build();

    let block = BlockBuilder::new(&sandbox)
        .with_state_hash(&sandbox.compute_state_hash(&[tx.raw().clone()]))
        .with_tx_hash(&tx.hash())
        .build();

    let precommit_1 = Precommit::new(
        VALIDATOR_1,
        HEIGHT_ONE,
        ROUND_THREE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_1),
    );
    let precommit_2 = Precommit::new(
        VALIDATOR_2,
        HEIGHT_ONE,
        ROUND_THREE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_2),
    );

    // ok, we are leader

    sandbox.broadcast(&propose);
    sandbox.broadcast(&make_prevote_from_propose(&sandbox, &propose));

    {
        // respond to RequestPropose
        sandbox.recv(&ProposeRequest::new(
            &sandbox.p(VALIDATOR_3),
            &sandbox.p(VALIDATOR_0),
            HEIGHT_ONE,
            &propose.hash(),
            sandbox.s(VALIDATOR_3),
        ));

        sandbox.send(sandbox.a(VALIDATOR_3), &propose);
    }

    {
        // respond to RequestPrevotes
        let mut validators = BitVec::from_elem(sandbox.n_validators(), false);
        validators.set(VALIDATOR_3.into(), true);

        sandbox.recv(&PrevotesRequest::new(
            &sandbox.p(VALIDATOR_3),
            &sandbox.p(VALIDATOR_0),
            HEIGHT_ONE,
            ROUND_THREE,
            &propose.hash(),
            validators,
            sandbox.s(VALIDATOR_3),
        ));

        sandbox.send(
            sandbox.a(VALIDATOR_3),
            &make_prevote_from_propose(&sandbox, &propose),
        );
    }

    sandbox.recv(&Prevote::new(
        VALIDATOR_1,
        HEIGHT_ONE,
        ROUND_THREE,
        &propose.hash(),
        Round::zero(),
        sandbox.s(VALIDATOR_1),
    ));
    sandbox.recv(&Prevote::new(
        VALIDATOR_2,
        HEIGHT_ONE,
        ROUND_THREE,
        &propose.hash(),
        Round::zero(),
        sandbox.s(VALIDATOR_2),
    ));

    sandbox.broadcast(&Precommit::new(
        VALIDATOR_0,
        HEIGHT_ONE,
        ROUND_THREE,
        &propose.hash(),
        &block.hash(),
        sandbox.time().into(),
        sandbox.s(VALIDATOR_0),
    ));

    sandbox.recv(&precommit_1);
    sandbox.recv(&precommit_2);

    sandbox.assert_state(HEIGHT_TWO, ROUND_ONE);
    sandbox.check_broadcast_status(HEIGHT_TWO, &block.hash());

    {
        // respond to RequestTransactions
        sandbox.recv(&TransactionsRequest::new(
            &sandbox.p(VALIDATOR_1),
            &sandbox.p(VALIDATOR_0),
            &[tx.hash()],
            sandbox.s(VALIDATOR_1),
        ));

        sandbox.send(
            sandbox.a(VALIDATOR_1),
            &TransactionsResponse::new(
                &sandbox.p(VALIDATOR_0),
                &sandbox.p(VALIDATOR_1),
                vec![tx.raw().clone()],
                sandbox.s(VALIDATOR_0),
            ),
        );
    }

    {
        // respond to RequestPropose negative
        sandbox.recv(&ProposeRequest::new(
            &sandbox.p(VALIDATOR_3),
            &sandbox.p(VALIDATOR_0),
            HEIGHT_ONE,
            &propose.hash(),
            sandbox.s(VALIDATOR_3),
        ));

        // here state.height() is 1 already, so requestPropose with height zero is ignored
        //        sandbox.send(
        //            sandbox.a(VALIDATOR_3),
        //            propose.clone()
        //        );
    }

    {
        // respond to RequestPrevotes negative
        let mut validators = BitVec::from_elem(sandbox.n_validators(), false);
        validators.set(VALIDATOR_3.into(), true);

        sandbox.recv(&PrevotesRequest::new(
            &sandbox.p(VALIDATOR_3),
            &sandbox.p(VALIDATOR_0),
            HEIGHT_ONE,
            ROUND_THREE,
            &propose.hash(),
            validators,
            sandbox.s(VALIDATOR_3),
        ));

        // here state.height() is 1 already, so RequestPrevotes with height zero is ignored
        //        sandbox.send(
        //            sandbox.a(VALIDATOR_3),
        //            make_prevote_from_propose(&sandbox, &propose)
        //        );
    }

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
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .with_tx_hashes(&[tx.hash()]) //ordinary propose, but with this unreceived tx
        .build();

    sandbox.recv(&propose);
    sandbox.broadcast(&make_prevote_from_propose(&sandbox, &propose));
    sandbox.add_time(Duration::from_millis(TRANSACTIONS_REQUEST_TIMEOUT));
}

/// HANDLE TX
/// - verify signature
/// - should panic because tx has wrong signature and is not considered
#[cfg_attr(rustfmt, rustfmt_skip)]
#[test]
#[should_panic(expected = "Send unexpected message Request(TransactionsRequest")]
fn handle_tx_verify_signature() {
    let sandbox = timestamping_sandbox();

    // generate incorrect tx
    let (public_key1, _) = gen_keypair();
    let (_, secret_key2) = gen_keypair();

    let data = vec![0; 64]; // TODO: Find the way how to get rid of hard-coded value. (ECR-1627)
    let tx = TimestampTx::new(&public_key1, &data, &secret_key2);
    sandbox.recv(&tx);

    let propose = ProposeBuilder::new(&sandbox)
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .with_tx_hashes(&[tx.hash()]) //ordinary propose, but with this unreceived tx
        .build();

    sandbox.recv(&propose);
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
        .with_duration_since_sandbox_time(PROPOSE_TIMEOUT)
        .with_tx_hashes(&[tx.hash()]) //ordinary propose, but with this unreceived tx
        .build();

    sandbox.recv(&propose);
    sandbox.add_time(Duration::from_millis(sandbox.round_timeout() - 1));

    sandbox.send(
        sandbox.a(VALIDATOR_2),
        &TransactionsRequest::new(
            &sandbox.p(VALIDATOR_0),
            &sandbox.p(VALIDATOR_2),
            &[tx.hash()],
            sandbox.s(VALIDATOR_0),
        ),
    );

    sandbox.add_time(Duration::from_millis(0));

    sandbox.recv(&Prevote::new(
        VALIDATOR_3,
        HEIGHT_ONE,
        ROUND_ONE,
        &propose.hash(),
        Round::zero(),
        sandbox.s(VALIDATOR_3),
    ));

    sandbox.add_time(Duration::from_millis(sandbox.round_timeout() - 1));

    sandbox.send(
        sandbox.a(VALIDATOR_3),
        &TransactionsRequest::new(
            &sandbox.p(VALIDATOR_0),
            &sandbox.p(VALIDATOR_3),
            &[tx.hash()],
            sandbox.s(VALIDATOR_0),
        ),
    );

    sandbox.add_time(Duration::from_millis(0));
}
