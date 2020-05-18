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

//! These are tests concerning the business logic of transactions in the cryptocurrency service.
//! See `api.rs` for tests focused on the API of the service.
//!
//! Note how business logic tests use `TestKit::create_block*` methods to send transactions,
//! the service schema to make assertions about the storage state.

use exonum::{
    crypto::{KeyPair, PublicKey},
    runtime::SnapshotExt,
};
use exonum_merkledb::{access::Access, Snapshot};
use exonum_testkit::TestKit;

// Import data types used in tests from the crate where the service is defined.
use crate::{
    contracts::{CryptocurrencyInterface, CryptocurrencyService},
    schema::{CurrencySchema, Wallet},
    transactions::{CreateWallet, TxTransfer},
};

/// Alice's wallets name.
const ALICE_NAME: &str = "Alice";
/// Bob's wallet name.
const BOB_NAME: &str = "Bob";
/// Service instance id.
const INSTANCE_ID: u32 = 1010;
/// Service instance name.
const INSTANCE_NAME: &str = "nnm-token";

fn get_schema<'a>(snapshot: &'a dyn Snapshot) -> CurrencySchema<impl Access + 'a> {
    CurrencySchema::new(snapshot.for_service(INSTANCE_NAME).unwrap())
}

#[test]
fn test_create_wallet() {
    let mut testkit = init_testkit();
    let tx = KeyPair::random().create_wallet(INSTANCE_ID, CreateWallet::new(ALICE_NAME));
    testkit.create_block_with_transaction(tx.clone());

    // Check that the user indeed is persisted by the service
    let wallet = get_wallet(&testkit, &tx.author());
    assert_eq!(wallet.pub_key, tx.author());
    assert_eq!(wallet.name, ALICE_NAME);
    assert_eq!(wallet.balance, 100);
}

#[test]
fn test_transfer() {
    let mut testkit = init_testkit();
    let alice = KeyPair::random();
    let bob = KeyPair::random();
    testkit.create_block_with_transactions(vec![
        alice.create_wallet(INSTANCE_ID, CreateWallet::new(ALICE_NAME)),
        bob.create_wallet(INSTANCE_ID, CreateWallet::new(BOB_NAME)),
        alice.transfer(
            INSTANCE_ID,
            TxTransfer {
                amount: 10,
                seed: 0,
                to: bob.public_key(),
            },
        ),
    ]);

    let alice_wallet = get_wallet(&testkit, &alice.public_key());
    assert_eq!(alice_wallet.balance, 90);
    let bob_wallet = get_wallet(&testkit, &bob.public_key());
    assert_eq!(bob_wallet.balance, 110);
}

#[test]
fn test_transfer_from_nonexisting_wallet() {
    let mut testkit = init_testkit();
    let alice = KeyPair::random();
    let bob = KeyPair::random();
    testkit.create_block_with_transactions(vec![
        bob.create_wallet(INSTANCE_ID, CreateWallet::new(BOB_NAME)),
        alice.transfer(
            INSTANCE_ID,
            TxTransfer {
                amount: 10,
                seed: 0,
                to: bob.public_key(),
            },
        ),
    ]);

    assert!(try_get_wallet(&testkit, &alice.public_key()).is_none());
    let bob_wallet = get_wallet(&testkit, &bob.public_key());
    assert_eq!(bob_wallet.balance, 100);
}

#[test]
fn test_transfer_to_nonexisting_wallet() {
    let mut testkit = init_testkit();
    let alice = KeyPair::random();
    let bob = KeyPair::random();
    testkit.create_block_with_transactions(vec![
        alice.create_wallet(INSTANCE_ID, CreateWallet::new(ALICE_NAME)),
        alice.transfer(
            INSTANCE_ID,
            TxTransfer {
                amount: 10,
                seed: 0,
                to: bob.public_key(),
            },
        ),
        // Although Bob's wallet is created, this occurs after the transfer is executed.
        bob.create_wallet(INSTANCE_ID, CreateWallet::new(BOB_NAME)),
    ]);

    let alice_wallet = get_wallet(&testkit, &alice.public_key());
    assert_eq!(alice_wallet.balance, 100);
    let bob_wallet = get_wallet(&testkit, &bob.public_key());
    assert_eq!(bob_wallet.balance, 100);
}

#[test]
fn test_transfer_overcharge() {
    let mut testkit = init_testkit();
    let alice = KeyPair::random();
    let bob = KeyPair::random();
    testkit.create_block_with_transactions(vec![
        alice.create_wallet(INSTANCE_ID, CreateWallet::new(ALICE_NAME)),
        bob.create_wallet(INSTANCE_ID, CreateWallet::new(BOB_NAME)),
        alice.transfer(
            INSTANCE_ID,
            TxTransfer {
                amount: 150,
                seed: 0,
                to: bob.public_key(),
            },
        ),
    ]);

    // The transfer amount is greater than what Alice has at her disposal, so
    // the transfer should fail.
    let alice_wallet = get_wallet(&testkit, &alice.public_key());
    assert_eq!(alice_wallet.balance, 100);
    let bob_wallet = get_wallet(&testkit, &bob.public_key());
    assert_eq!(bob_wallet.balance, 100);
}

#[test]
fn test_transfers_in_single_block() {
    let mut testkit = init_testkit();
    let alice = KeyPair::random();
    let bob = KeyPair::random();
    testkit.create_block_with_transactions(vec![
        alice.create_wallet(INSTANCE_ID, CreateWallet::new(ALICE_NAME)),
        bob.create_wallet(INSTANCE_ID, CreateWallet::new(BOB_NAME)),
    ]);

    let tx_a_to_b = alice.transfer(
        INSTANCE_ID,
        TxTransfer {
            to: bob.public_key(),
            amount: 90,
            seed: 0,
        },
    );
    let tx_b_to_a = bob.transfer(
        INSTANCE_ID,
        TxTransfer {
            to: alice.public_key(),
            amount: 120,
            seed: 0,
        },
    );

    testkit.create_block_with_transactions(vec![tx_a_to_b, tx_b_to_a]);

    let alice_wallet = get_wallet(&testkit, &alice.public_key());
    assert_eq!(alice_wallet.balance, 130);
    let bob_wallet = get_wallet(&testkit, &bob.public_key());
    assert_eq!(bob_wallet.balance, 70);
}

/// Generate random transactions to perform [fuzz testing][fuzz] of the service. The service
/// should maintain invariants under all circumstances; e.g., the total amount of tokens
/// in existence should depend only on the number of registered wallets.
///
/// [fuzz]: https://en.wikipedia.org/wiki/Fuzzing
#[test]
fn test_fuzz_transfers() {
    use rand::{seq::SliceRandom, Rng};
    use std::{collections::BTreeSet, iter::FromIterator};

    const BLOCKS: usize = 50; // number of blocks to create
    const MAX_TRANSACTIONS: usize = 20; // maximum number of transactions in a block

    let mut rng = rand::thread_rng();

    let mut testkit = init_testkit();
    let alice = KeyPair::random();
    let bob = KeyPair::random();
    let keys = &[alice.clone(), bob.clone()];
    testkit.create_block_with_transactions(vec![
        alice.create_wallet(INSTANCE_ID, CreateWallet::new(ALICE_NAME)),
        bob.create_wallet(INSTANCE_ID, CreateWallet::new(BOB_NAME)),
    ]);

    for _ in 0..BLOCKS {
        let n_txs = rng.gen_range(0, MAX_TRANSACTIONS); // number of transactions in the block

        let txs = (0..n_txs).map(|_| {
            let (sender, receiver) = (
                keys.choose(&mut rng).unwrap(),
                keys.choose(&mut rng).unwrap(),
            );
            let amount = rng.gen_range(0, 250);

            sender.transfer(
                INSTANCE_ID,
                TxTransfer {
                    to: receiver.public_key(),
                    amount,
                    seed: rng.gen::<u64>(),
                },
            )
        });
        testkit.create_block_with_transactions(txs);

        // Test invariants that should be maintained during fuzz testing.
        let snapshot = testkit.snapshot();
        let schema = get_schema(&snapshot);
        let wallets: Vec<_> = schema.wallets.values().collect();
        // There must be 2 wallets in the storage.
        assert_eq!(wallets.len(), 2);
        // These wallets should belong to Alice and Bob.
        assert_eq!(
            BTreeSet::from_iter(wallets.iter().map(|w| &w.pub_key)),
            BTreeSet::from_iter(vec![&alice.public_key(), &bob.public_key()])
        );
        // The total amount of funds should equal 200, no matter which transactions were executed.
        assert_eq!(wallets.iter().map(|w| w.balance).sum::<u64>(), 200);
    }
}

/// Initializes testkit with `CurrencyService`.
fn init_testkit() -> TestKit {
    TestKit::for_rust_service(CryptocurrencyService, INSTANCE_NAME, INSTANCE_ID, ())
}

/// Returns the wallet identified by the given public key or `None` if such a wallet doesn't exist.
fn try_get_wallet(testkit: &TestKit, pubkey: &PublicKey) -> Option<Wallet> {
    let snapshot = testkit.snapshot();
    let schema = get_schema(&snapshot);
    schema.wallets.get(pubkey)
}

/// Returns the wallet identified by the given public key.
fn get_wallet(testkit: &TestKit, pubkey: &PublicKey) -> Wallet {
    try_get_wallet(testkit, pubkey).expect("No wallet persisted")
}
