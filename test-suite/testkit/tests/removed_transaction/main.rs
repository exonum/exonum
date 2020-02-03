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

use exonum::{
    crypto::KeyPair,
    messages::{AnyTx, Verified},
    runtime::{CallInfo, CommonError, ErrorMatch},
};
use exonum_merkledb::BinaryValue;
use exonum_testkit::{TestKit, TestKitApi};
use pretty_assertions::assert_eq;

use crate::service::{SampleService, SampleServiceInterface, SERVICE_ID, SERVICE_NAME};

mod service;

fn init_testkit() -> (TestKit, TestKitApi) {
    let mut testkit = TestKit::for_rust_service(SampleService, SERVICE_NAME, SERVICE_ID, ());
    let api = testkit.api();
    (testkit, api)
}

fn generate_tx() -> Verified<AnyTx> {
    KeyPair::random().method_b(SERVICE_ID, 0)
}

fn generate_txs_for_removed_methods() -> Vec<Verified<AnyTx>> {
    let keypair = KeyPair::random();

    let create_tx = |id| {
        let tx = AnyTx::new(CallInfo::new(SERVICE_ID, id), 0_u64.to_bytes());
        tx.sign_with_keypair(&keypair)
    };

    let tx1 = create_tx(0);
    let tx2 = create_tx(2);

    vec![tx1, tx2]
}

fn generate_tx_for_nonexistent_method() -> Verified<AnyTx> {
    let keypair = KeyPair::random();
    let tx = AnyTx::new(CallInfo::new(SERVICE_ID, 3), 0_u64.to_bytes());

    tx.sign_with_keypair(&keypair)
}

/// Checks that if method is marked as removed, attempt to invoke it
/// results in a `CommonError::MethodRemoved`.
#[test]
fn call_removed_method() {
    let (mut testkit, _) = init_testkit();
    let txs = generate_txs_for_removed_methods();

    let expected_error = ErrorMatch::from_fail(&CommonError::MethodRemoved).for_service(SERVICE_ID);

    let block = testkit.create_block_with_transactions(txs);

    for tx in block.iter() {
        let error = tx
            .status()
            .expect_err("Tx for `method_b` should be executed successfully");

        assert_eq!(*error, expected_error);
    }
}

/// Checks that attempt to call existing method from service in which one method was removed
/// still executed successfully.
/// In other words, we check that removing one method from interface doesn't break other methods.
#[test]
fn call_existing_method() {
    let (mut testkit, _) = init_testkit();
    let tx = generate_tx();

    testkit.create_block_with_transaction(tx).transactions[0]
        .status()
        .expect("Tx for `method_b` should be executed successfully");
}

/// Checks that for nonexistent method `CommonError::NoSuchMethod` is
/// returned.
#[test]
fn call_nonexisting_method() {
    let (mut testkit, _) = init_testkit();
    let tx = generate_tx_for_nonexistent_method();

    let block = testkit.create_block_with_transaction(tx);
    let error = block.transactions[0]
        .status()
        .expect_err("Tx for `method_b` should be executed successfully");

    let expected_error = ErrorMatch::from_fail(&CommonError::NoSuchMethod).for_service(SERVICE_ID);

    assert_eq!(*error, expected_error);
}
