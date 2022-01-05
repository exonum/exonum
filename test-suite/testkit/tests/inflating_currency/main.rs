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

//! Tests for sample currency service with inflation. Similar to the `test_currency`
//! integration test, with the difference that the balance of each created wallet increases by 1
//! on each block. Correspondingly, the initial wallet balance is set to 0.

use exonum::{
    crypto::{KeyPair, PublicKey},
    helpers::Height,
};
use exonum_testkit::{ApiKind, Spec, TestKit, TestKitApi, TestKitBuilder};
use futures::{
    stream::{self, StreamExt},
    FutureExt,
};
use pretty_assertions::assert_eq;
use rand::Rng;

use crate::inflating_cryptocurrency::{
    CreateWallet, CurrencyInterface, CurrencyService, Transfer, SERVICE_ID,
};

mod inflating_cryptocurrency;

fn init_testkit() -> TestKit {
    TestKitBuilder::validator()
        .with_validators(4)
        .with(Spec::new(CurrencyService).with_default_instance())
        .build()
}

fn create_wallet(testkit: &mut TestKit, name: &str) -> KeyPair {
    let keypair = KeyPair::random();
    // Create a pre-signed transaction
    let tx = keypair.create_wallet(SERVICE_ID, CreateWallet::new(name));
    let block = testkit.create_block_with_transaction(tx);
    block[0].status().unwrap();
    keypair
}

async fn get_balance(api: &TestKitApi, pubkey: &PublicKey) -> u64 {
    api.public(ApiKind::Service("cryptocurrency"))
        .get(&format!("v1/balance?pub_key={}", pubkey.to_hex()))
        .await
        .unwrap()
}

#[tokio::test]
async fn test_inflation() {
    let mut testkit = init_testkit();
    let alice = create_wallet(&mut testkit, "Alice");

    let api = testkit.api();
    assert_eq!(get_balance(&api, &alice.public_key()).await, 1);
    testkit.create_blocks_until(Height(10));
    assert_eq!(get_balance(&api, &alice.public_key()).await, 10);
}

#[tokio::test]
async fn test_transfer_scenarios() {
    let mut testkit = init_testkit();
    let api = testkit.api();

    // Create 2 wallets
    let alice = KeyPair::random();
    let tx_alice = alice.create_wallet(SERVICE_ID, CreateWallet::new("alice"));
    let bob = KeyPair::random();
    let tx_bob = bob.create_wallet(SERVICE_ID, CreateWallet::new("Bob"));
    testkit.create_block_with_transactions(vec![tx_alice, tx_bob]);
    testkit.create_blocks_until(Height(9));

    // Check that the initial Alice's and Bob's balances are persisted by the service
    assert_eq!(get_balance(&api, &alice.public_key()).await, 9);
    assert_eq!(get_balance(&api, &bob.public_key()).await, 9);

    // Transfer funds
    let tx_a_to_b = alice.transfer(
        SERVICE_ID,
        Transfer {
            to: bob.public_key(),
            amount: 5,
            seed: 0,
        },
    );

    let next_tx_a_to_b = alice.transfer(
        SERVICE_ID,
        Transfer {
            to: bob.public_key(),
            amount: 6,
            seed: 1,
        },
    );

    // Put transactions from A to B in separate blocks, allowing them both to succeed.
    testkit.checkpoint();
    testkit.create_block_with_transactions(vec![tx_a_to_b.clone()]); // A: 4 + 1, B: 14 + 1
    testkit.create_block_with_transactions(vec![]); // A: 4 + 2, B: 14 + 2
    testkit.create_block_with_transactions(vec![next_tx_a_to_b.clone()]); // A: 0 + 1, B: 20 + 3
    assert_eq!(get_balance(&api, &alice.public_key()).await, 1); // 0 + 1
    assert_eq!(get_balance(&api, &bob.public_key()).await, 23); // 20 + 3
    testkit.rollback();

    // If there is no block separating transactions, Alice's balance is insufficient
    // to complete the second transaction.
    testkit.checkpoint();
    testkit.create_block_with_transactions(vec![tx_a_to_b.clone()]); // A: 4 + 1, B: 14 + 1
    testkit.create_block_with_transactions(vec![next_tx_a_to_b.clone()]); // fails
    assert_eq!(get_balance(&api, &alice.public_key()).await, 6); // 4 + 2
    assert_eq!(get_balance(&api, &bob.public_key()).await, 16); // 14 + 2
    testkit.rollback();

    testkit.checkpoint();
    testkit.create_block_with_transactions(vec![next_tx_a_to_b.clone()]); // A: 3 + 1, B: 15 + 1
    testkit.create_block_with_transactions(vec![tx_a_to_b.clone()]); // fails
    assert_eq!(get_balance(&api, &alice.public_key()).await, 5); // 3 + 2
    assert_eq!(get_balance(&api, &bob.public_key()).await, 17); // 15 + 2
    testkit.rollback();

    // If the transactions are put in the same block, only the first transaction should succeed
    testkit.checkpoint();
    testkit.create_block_with_transactions(vec![tx_a_to_b.clone(), next_tx_a_to_b.clone()]);
    assert_eq!(get_balance(&api, &alice.public_key()).await, 5); // 4 + 1
    assert_eq!(get_balance(&api, &bob.public_key()).await, 15); // 14 + 1
    testkit.rollback();

    // Same here
    testkit.checkpoint();
    testkit.create_block_with_transactions(vec![next_tx_a_to_b, tx_a_to_b]);
    assert_eq!(get_balance(&api, &alice.public_key()).await, 4); // 3 + 1
    assert_eq!(get_balance(&api, &bob.public_key()).await, 16); // 15 + 1
    testkit.rollback();
}

/// Test randomly generated transfers among users without blockchain rollbacks.
#[tokio::test]
async fn test_fuzz_transfers() {
    const USERS: usize = 10;

    let mut rng = rand::thread_rng();
    let mut testkit = init_testkit();
    let api = testkit.api();

    // First, create users
    let keys_and_txs: Vec<_> = (0..USERS)
        .map(|i| {
            let keypair = KeyPair::random();
            let tx = keypair.create_wallet(SERVICE_ID, CreateWallet::new(format!("User #{}", i)));
            (keypair, tx)
        })
        .collect();
    let pubkeys: Vec<_> = keys_and_txs.iter().map(|(_, tx)| tx.author()).collect();

    testkit.create_block_with_transactions(keys_and_txs.iter().map(|(_, tx)| tx.clone()));

    for _ in 0..64 {
        let total_balance: u64 = stream::iter(&pubkeys)
            .fold(0, |acc, key| get_balance(&api, key).map(move |x| x + acc))
            .await;
        assert_eq!(total_balance, (USERS as u64) * testkit.height().0);

        let tx_count = rng.gen::<u32>() & 15;
        let height = testkit.height().0;
        let txs = (0..tx_count).map(|_| {
            let sender_idx = rng.gen_range(0..USERS);
            let (sender, _) = &keys_and_txs[sender_idx];
            let receiver = &pubkeys[rng.gen_range(0..USERS)];
            let amount = rng.gen_range(1..2 * height);

            sender.transfer(
                SERVICE_ID,
                Transfer {
                    to: *receiver,
                    amount,
                    seed: rng.gen::<u64>(),
                },
            )
        });
        testkit.create_block_with_transactions(txs);
    }
}
