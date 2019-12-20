// Copyright 2019 The Exonum Team
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

//! Tests for sample currency service with inflation. Similar to the `test_currency`
//! integration test, with the difference that the balance of each created wallet increases by 1
//! on each block. Correspondingly, the initial wallet balance is set to 0.

#[macro_use]
extern crate pretty_assertions;

use exonum::{
    api::node::public::explorer::TransactionResponse,
    crypto::{self, PublicKey, SecretKey},
    helpers::Height,
    messages::{AnyTx, Verified},
    runtime::rust::Transaction,
};
use exonum_merkledb::{BinaryValue, ObjectHash};
use exonum_testkit::{ApiKind, TestKit, TestKitApi, TestKitBuilder};
use rand::Rng;
use serde_json::json;

use crate::inflating_cryptocurrency::{CreateWallet, CurrencyService, Transfer, SERVICE_ID};

mod inflating_cryptocurrency;
mod proto;

fn init_testkit() -> TestKit {
    TestKitBuilder::validator()
        .with_validators(4)
        .with_default_rust_service(CurrencyService)
        .create()
}

fn create_wallet(api: &TestKitApi, name: &str) -> (Verified<AnyTx>, SecretKey) {
    let (pubkey, key) = crypto::gen_keypair();
    // Create a pre-signed transaction
    let tx = CreateWallet {
        name: name.to_owned(),
    }
    .sign(SERVICE_ID, pubkey, &key);

    let data = hex::encode(tx.to_bytes());
    let tx_info: TransactionResponse = api
        .public(ApiKind::Explorer)
        .query(&json!({ "tx_body": data }))
        .post("v1/transactions")
        .unwrap();
    assert_eq!(tx_info.tx_hash, tx.object_hash());

    (tx, key)
}

fn get_balance(api: &TestKitApi, pubkey: &PublicKey) -> u64 {
    api.public(ApiKind::Service("cryptocurrency"))
        .get(&format!("v1/balance?pub_key={}", pubkey.to_hex()))
        .unwrap()
}

#[test]
fn test_inflation() {
    let mut testkit = init_testkit();
    let api = testkit.api();
    let (tx, _) = create_wallet(&api, "Alice");

    testkit.create_block();
    assert_eq!(get_balance(&api, &tx.author()), 1);
    testkit.create_blocks_until(Height(10));
    assert_eq!(get_balance(&api, &tx.author()), 10);
}

#[test]
fn test_transfer_scenarios() {
    let mut testkit = init_testkit();
    let api = testkit.api();

    // Create 2 wallets
    let (tx_alice, key_alice) = create_wallet(&api, "Alice");
    let (tx_bob, _) = create_wallet(&api, "Bob");
    testkit.create_blocks_until(Height(9));

    // Check that the initial Alice's and Bob's balances are persisted by the service
    assert_eq!(get_balance(&api, &tx_alice.author()), 9);
    assert_eq!(get_balance(&api, &tx_bob.author()), 9);

    // Transfer funds
    let tx_a_to_b = Transfer {
        to: tx_bob.author(),
        amount: 5,
        seed: 0,
    }
    .sign(SERVICE_ID, tx_alice.author(), &key_alice);

    let next_tx_a_to_b = Transfer {
        to: tx_bob.author(),
        amount: 6,
        seed: 1,
    }
    .sign(SERVICE_ID, tx_alice.author(), &key_alice);

    // Put transactions from A to B in separate blocks, allowing them both to succeed.
    testkit.checkpoint();
    testkit.create_block_with_transactions(vec![tx_a_to_b.clone()]); // A: 4 + 1, B: 14 + 1
    testkit.create_block_with_transactions(vec![]); // A: 4 + 2, B: 14 + 2
    testkit.create_block_with_transactions(vec![next_tx_a_to_b.clone()]); // A: 0 + 1, B: 20 + 3
    assert_eq!(get_balance(&api, &tx_alice.author()), 1); // 0 + 1
    assert_eq!(get_balance(&api, &tx_bob.author()), 23); // 20 + 3
    testkit.rollback();

    // If there is no block separating transactions, Alice's balance is insufficient
    // to complete the second transaction.
    testkit.checkpoint();
    testkit.create_block_with_transactions(vec![tx_a_to_b.clone()]); // A: 4 + 1, B: 14 + 1
    testkit.create_block_with_transactions(vec![next_tx_a_to_b.clone()]); // fails
    assert_eq!(get_balance(&api, &tx_alice.author()), 6); // 4 + 2
    assert_eq!(get_balance(&api, &tx_bob.author()), 16); // 14 + 2
    testkit.rollback();

    testkit.checkpoint();
    testkit.create_block_with_transactions(vec![next_tx_a_to_b.clone()]); // A: 3 + 1, B: 15 + 1
    testkit.create_block_with_transactions(vec![tx_a_to_b.clone()]); // fails
    assert_eq!(get_balance(&api, &tx_alice.author()), 5); // 3 + 2
    assert_eq!(get_balance(&api, &tx_bob.author()), 17); // 15 + 2
    testkit.rollback();

    // If the transactions are put in the same block, only the first transaction should succeed
    testkit.checkpoint();
    testkit.create_block_with_transactions(vec![tx_a_to_b.clone(), next_tx_a_to_b.clone()]);
    assert_eq!(get_balance(&api, &tx_alice.author()), 5); // 4 + 1
    assert_eq!(get_balance(&api, &tx_bob.author()), 15); // 14 + 1
    testkit.rollback();

    // Same here
    testkit.checkpoint();
    testkit.create_block_with_transactions(vec![next_tx_a_to_b.clone(), tx_a_to_b.clone()]);
    assert_eq!(get_balance(&api, &tx_alice.author()), 4); // 3 + 1
    assert_eq!(get_balance(&api, &tx_bob.author()), 16); // 15 + 1
    testkit.rollback();
}

/// Test randomly generated transfers among users without blockchain rollbacks.
#[test]
fn test_fuzz_transfers() {
    const USERS: usize = 10;

    let mut rng = rand::thread_rng();
    let mut testkit = init_testkit();
    let api = testkit.api();

    // First, create users
    let keys_and_txs: Vec<_> = (0..USERS)
        .map(|i| {
            let (pubkey, key) = crypto::gen_keypair();
            let tx = CreateWallet {
                name: format!("User #{}", i),
            }
            .sign(SERVICE_ID, pubkey, &key);
            (key, tx)
        })
        .collect();
    let pubkeys: Vec<_> = keys_and_txs
        .iter()
        .map(|&(_, ref tx)| tx.author())
        .collect();

    testkit.create_block_with_transactions(keys_and_txs.iter().map(|&(_, ref tx)| tx.clone()));

    for _ in 0..64 {
        let total_balance: u64 = pubkeys.iter().map(|key| get_balance(&api, &key)).sum();
        assert_eq!(total_balance, (USERS as u64) * testkit.height().0);

        let tx_count = rng.gen::<u32>() & 15;
        let height = testkit.height().0;
        let txs = (0..tx_count).map(|_| {
            let sender_idx = rng.gen_range(0, USERS);
            let sender = &pubkeys[sender_idx];
            let sender_key = &keys_and_txs[sender_idx].0;
            let receiver = &pubkeys[rng.gen_range(0, USERS)];
            let amount = rng.gen_range(1, 2 * height);

            Transfer {
                to: *receiver,
                amount,
                seed: rng.gen::<u64>(),
            }
            .sign(SERVICE_ID, *sender, sender_key)
        });
        testkit.create_block_with_transactions(txs);
    }
}
