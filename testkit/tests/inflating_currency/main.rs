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

//! Tests for sample currency service with inflation. Similar to the `test_currency`
//! integration test, with the difference that the balance of each created wallet increases by 1
//! on each block. Correspondingly, the initial wallet balance is set to 0.

#[macro_use]
extern crate exonum;
#[macro_use]
extern crate exonum_testkit;
#[macro_use]
extern crate pretty_assertions;
extern crate rand;
#[macro_use]
extern crate serde_derive;

use exonum::blockchain::Transaction;
use exonum::crypto::{self, CryptoHash, PublicKey, SecretKey};
use exonum::helpers::Height;
use exonum_testkit::{ApiKind, TestKit, TestKitApi, TestKitBuilder};
use rand::Rng;

use inflating_cryptocurrency::{CurrencyService, TransactionResponse, TxCreateWallet, TxTransfer};

mod inflating_cryptocurrency;

fn init_testkit() -> TestKit {
    TestKitBuilder::validator()
        .with_validators(4)
        .with_service(CurrencyService)
        .create()
}

fn create_wallet(api: &TestKitApi, name: &str) -> (TxCreateWallet, SecretKey) {
    let (pubkey, key) = crypto::gen_keypair();
    // Create a pre-signed transaction
    let tx = TxCreateWallet::new(&pubkey, name, &key);

    let tx_info: TransactionResponse = api.post(
        ApiKind::Service("cryptocurrency"),
        "v1/wallets/transaction",
        &tx,
    );
    assert_eq!(tx_info.tx_hash, tx.hash());

    (tx, key)
}

fn get_balance(api: &TestKitApi, pubkey: &PublicKey) -> u64 {
    api.get(
        ApiKind::Service("cryptocurrency"),
        &format!("v1/balance/{}", pubkey.to_string()),
    )
}

#[test]
fn test_inflation() {
    let mut testkit = init_testkit();
    let api = testkit.api();
    let (tx, _) = create_wallet(&api, "Alice");

    testkit.create_block();
    assert_eq!(get_balance(&api, tx.pub_key()), 1);
    testkit.create_blocks_until(Height(10));
    assert_eq!(get_balance(&api, tx.pub_key()), 10);
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
    assert_eq!(get_balance(&api, tx_alice.pub_key()), 9);
    assert_eq!(get_balance(&api, tx_bob.pub_key()), 9);

    // Transfer funds
    let tx_a_to_b = TxTransfer::new(
        tx_alice.pub_key(),
        tx_bob.pub_key(),
        5, // amount
        0, // seed
        &key_alice,
    );
    let next_tx_a_to_b = TxTransfer::new(
        tx_alice.pub_key(),
        tx_bob.pub_key(),
        6, // amount
        1, // seed
        &key_alice,
    );
    // Put transactions from A to B in separate blocks, allowing them both to succeed.
    testkit.checkpoint();
    testkit.create_block_with_transactions(txvec![tx_a_to_b.clone()]); // A: 4 + 1, B: 14 + 1
    testkit.create_block_with_transactions(txvec![]); // A: 4 + 2, B: 14 + 2
    testkit.create_block_with_transactions(txvec![next_tx_a_to_b.clone()]); // A: 0 + 1, B: 20 + 3
    assert_eq!(get_balance(&api, tx_alice.pub_key()), 1); // 0 + 1
    assert_eq!(get_balance(&api, tx_bob.pub_key()), 23); // 20 + 3
    testkit.rollback();

    // If there is no block separating transactions, Alice's balance is insufficient
    // to complete the second transaction.
    testkit.checkpoint();
    testkit.create_block_with_transactions(txvec![tx_a_to_b.clone()]); // A: 4 + 1, B: 14 + 1
    testkit.create_block_with_transactions(txvec![next_tx_a_to_b.clone()]); // fails
    assert_eq!(get_balance(&api, tx_alice.pub_key()), 6); // 4 + 2
    assert_eq!(get_balance(&api, tx_bob.pub_key()), 16); // 14 + 2
    testkit.rollback();

    testkit.checkpoint();
    testkit.create_block_with_transactions(txvec![next_tx_a_to_b.clone()]); // A: 3 + 1, B: 15 + 1
    testkit.create_block_with_transactions(txvec![tx_a_to_b.clone()]); // fails
    assert_eq!(get_balance(&api, tx_alice.pub_key()), 5); // 3 + 2
    assert_eq!(get_balance(&api, tx_bob.pub_key()), 17); // 15 + 2
    testkit.rollback();

    // If the transactions are put in the same block, only the first transaction should succeed
    testkit.checkpoint();
    testkit.create_block_with_transactions(txvec![tx_a_to_b.clone(), next_tx_a_to_b.clone()]);
    assert_eq!(get_balance(&api, tx_alice.pub_key()), 5); // 4 + 1
    assert_eq!(get_balance(&api, tx_bob.pub_key()), 15); // 14 + 1
    testkit.rollback();

    // Same here
    testkit.checkpoint();
    testkit.create_block_with_transactions(txvec![next_tx_a_to_b.clone(), tx_a_to_b.clone()]);
    assert_eq!(get_balance(&api, tx_alice.pub_key()), 4); // 3 + 1
    assert_eq!(get_balance(&api, tx_bob.pub_key()), 16); // 15 + 1
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
            let tx = TxCreateWallet::new(&pubkey, &format!("User #{}", i), &key);
            (key, tx)
        })
        .collect();
    let pubkeys: Vec<&_> = keys_and_txs
        .iter()
        .map(|&(_, ref tx)| tx.pub_key())
        .collect();

    testkit.create_block_with_transactions(
        keys_and_txs
            .iter()
            .map(|&(_, ref tx)| Box::new(tx.clone()) as Box<Transaction>),
    );

    for _ in 0..64 {
        let total_balance: u64 = pubkeys.iter().map(|key| get_balance(&api, key)).sum();
        assert_eq!(total_balance, (USERS as u64) * testkit.height().0);

        let tx_count = rng.next_u32() & 15;
        let height = testkit.height().0;
        let txs = (0..tx_count)
            .map(|_| {
                let sender_idx = rng.gen_range(0, USERS);
                let sender = pubkeys[sender_idx];
                let sender_key = &keys_and_txs[sender_idx].0;
                let receiver = pubkeys[rng.gen_range(0, USERS)];
                let amount = rng.gen_range(1, 2 * height);

                TxTransfer::new(sender, receiver, amount, rng.next_u64(), sender_key)
            })
            .map(Box::<Transaction>::from);
        testkit.create_block_with_transactions(txs);
    }
}
