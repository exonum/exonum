// Copyright 2017 The Exonum Team
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

extern crate exonum;
extern crate exonum_cryptocurrency as cryptocurrency;
#[macro_use]
extern crate exonum_testkit;
extern crate rand;

use exonum::blockchain::Transaction;
use exonum::crypto;
use exonum_testkit::{TestKit, TestKitBuilder};

// Import data types used in tests from the crate where the service is defined.
use cryptocurrency::{CurrencySchema, CurrencyService, TxCreateWallet, TxTransfer, Wallet};

fn init_testkit() -> TestKit {
    TestKitBuilder::validator()
        .with_service(CurrencyService)
        .create()
}

#[test]
fn test_create_wallet() {
    let mut testkit = init_testkit();
    let (pubkey, key) = crypto::gen_keypair();
    testkit.create_block_with_transactions(txvec![
        TxCreateWallet::new(&pubkey, "Alice", &key),
    ]);

    // Check that the user indeed is persisted by the service
    let wallet = {
        let snapshot = testkit.snapshot();
        CurrencySchema::new(&snapshot).wallet(&pubkey).expect(
            "No wallet persisted",
        )
    };
    assert_eq!(*wallet.pub_key(), pubkey);
    assert_eq!(wallet.name(), "Alice");
    assert_eq!(wallet.balance(), 100);
}

#[test]
fn test_transfer() {
    let mut testkit = init_testkit();
    let (alice_pubkey, alice_key) = crypto::gen_keypair();
    let (bob_pubkey, bob_key) = crypto::gen_keypair();
    testkit.create_block_with_transactions(txvec![
        TxCreateWallet::new(&alice_pubkey, "Alice", &alice_key),
        TxCreateWallet::new(&bob_pubkey, "Bob", &bob_key),
        TxTransfer::new(&alice_pubkey, &bob_pubkey, /* amount */ 10, /* seed */ 0, &alice_key),
    ]);

    let wallets = {
        let snapshot = testkit.snapshot();
        let schema = CurrencySchema::new(&snapshot);
        (schema.wallet(&alice_pubkey), schema.wallet(&bob_pubkey))
    };
    if let (Some(alice_wallet), Some(bob_wallet)) = wallets {
        assert_eq!(alice_wallet.balance(), 90);
        assert_eq!(bob_wallet.balance(), 110);
    } else {
        panic!("Wallets not persisted");
    }
}

#[test]
fn test_transfer_from_nonexisting_wallet() {
    let mut testkit = init_testkit();
    let (alice_pubkey, alice_key) = crypto::gen_keypair();
    let (bob_pubkey, bob_key) = crypto::gen_keypair();
    testkit.create_block_with_transactions(txvec![
        TxCreateWallet::new(&bob_pubkey, "Bob", &bob_key),
        TxTransfer::new(&alice_pubkey, &bob_pubkey, /* amount */ 10, /* seed */ 0, &alice_key),
    ]);

    let wallets = {
        let snapshot = testkit.snapshot();
        let schema = CurrencySchema::new(&snapshot);
        (schema.wallet(&alice_pubkey), schema.wallet(&bob_pubkey))
    };
    if let (None, Some(bob_wallet)) = wallets {
        assert_eq!(bob_wallet.balance(), 100);
    } else {
        panic!("Wallets not persisted");
    }
}

#[test]
fn test_transfer_to_nonexisting_wallet() {
    let mut testkit = init_testkit();
    let (alice_pubkey, alice_key) = crypto::gen_keypair();
    let (bob_pubkey, bob_key) = crypto::gen_keypair();
    testkit.create_block_with_transactions(txvec![
        TxCreateWallet::new(&alice_pubkey, "Alice", &alice_key),
        TxTransfer::new(&alice_pubkey, &bob_pubkey, /* amount */ 10, /* seed */ 0, &alice_key),
        // Although Bob's wallet is created, this occurs after the transfer is executed
        TxCreateWallet::new(&bob_pubkey, "Bob", &bob_key),
    ]);

    let wallets = {
        let snapshot = testkit.snapshot();
        let schema = CurrencySchema::new(&snapshot);
        (schema.wallet(&alice_pubkey), schema.wallet(&bob_pubkey))
    };
    if let (Some(alice_wallet), Some(bob_wallet)) = wallets {
        assert_eq!(alice_wallet.balance(), 100);
        assert_eq!(bob_wallet.balance(), 100);
    } else {
        panic!("Wallets not persisted");
    }
}

#[test]
fn test_transfer_overcharge() {
    let mut testkit = init_testkit();
    let (alice_pubkey, alice_key) = crypto::gen_keypair();
    let (bob_pubkey, bob_key) = crypto::gen_keypair();
    testkit.create_block_with_transactions(txvec![
        TxCreateWallet::new(&alice_pubkey, "Alice", &alice_key),
        TxCreateWallet::new(&bob_pubkey, "Bob", &bob_key),
        TxTransfer::new(&alice_pubkey, &bob_pubkey, /* amount */ 150, /* seed */ 0, &alice_key),
    ]);

    // The transfer amount is greater than what Alice has at her disposal, so
    // the transfer should fail.
    let wallets = {
        let snapshot = testkit.snapshot();
        let schema = CurrencySchema::new(&snapshot);
        (schema.wallet(&alice_pubkey), schema.wallet(&bob_pubkey))
    };
    if let (Some(alice_wallet), Some(bob_wallet)) = wallets {
        assert_eq!(alice_wallet.balance(), 100);
        assert_eq!(bob_wallet.balance(), 100);
    } else {
        panic!("Wallets not persisted");
    }
}

#[test]
fn test_transfers_in_single_block() {
    let mut testkit = init_testkit();
    let (alice_pubkey, alice_key) = crypto::gen_keypair();
    let (bob_pubkey, bob_key) = crypto::gen_keypair();
    testkit.create_block_with_transactions(txvec![
        TxCreateWallet::new(&alice_pubkey, "Alice", &alice_key),
        TxCreateWallet::new(&bob_pubkey, "Bob", &bob_key),
    ]);

    let tx_a_to_b = TxTransfer::new(
        &alice_pubkey,
        &bob_pubkey,
        90, // amount
        0, // seed
        &alice_key,
    );
    let tx_b_to_a = TxTransfer::new(
        &bob_pubkey,
        &alice_pubkey,
        120, // amount
        0, // seed
        &bob_key,
    );

    {
        // See what happens if transactions are applied in an "incorrect" order.
        // We use `TestKit::probe_all()` method for this.

        let snapshot = testkit.probe_all(txvec![tx_b_to_a.clone(), tx_a_to_b.clone()]);
        let schema = CurrencySchema::new(&snapshot);
        assert_eq!(schema.wallet(&alice_pubkey).map(|w| w.balance()), Some(10));
        assert_eq!(schema.wallet(&bob_pubkey).map(|w| w.balance()), Some(190));
    }

    testkit.create_block_with_transactions(txvec![tx_a_to_b, tx_b_to_a]);
    let snapshot = testkit.snapshot();
    let schema = CurrencySchema::new(&snapshot);
    assert_eq!(schema.wallet(&alice_pubkey).map(|w| w.balance()), Some(130));
    assert_eq!(schema.wallet(&bob_pubkey).map(|w| w.balance()), Some(70));
}

/// Generate random transactions to perform [fuzz testing][fuzz] of the service. The service
/// should maintain invariants under all circumstances; e.g., the total amount of tokens
/// in existence should depend only on the number of registered wallets.
///
/// [fuzz]: https://en.wikipedia.org/wiki/Fuzzing
#[test]
fn test_fuzz_transfers() {
    use std::collections::BTreeSet;
    use std::iter::FromIterator;
    use rand::Rng;

    const BLOCKS: usize = 50; // number of blocks to create
    const MAX_TRANSACTIONS: usize = 20; // maximum number of transactions in a block

    let mut rng = rand::thread_rng();

    let mut testkit = init_testkit();
    let alice_keys = crypto::gen_keypair();
    let bob_keys = crypto::gen_keypair();
    let keys = &[alice_keys.clone(), bob_keys.clone()];
    testkit.create_block_with_transactions(txvec![
        TxCreateWallet::new(&alice_keys.0, "Alice", &alice_keys.1),
        TxCreateWallet::new(&bob_keys.0, "Bob", &bob_keys.1),
    ]);

    for _ in 0..BLOCKS {
        let n_txs = rng.gen_range(0, MAX_TRANSACTIONS); // number of transactions in the block

        let txs: Vec<Box<Transaction>> = (0..n_txs)
            .map(|_| {
                let (sender, receiver) = (rng.choose(keys).unwrap(), rng.choose(keys).unwrap());
                let amount = rng.gen_range(0, 250);
                TxTransfer::new(&sender.0, &receiver.0, amount, rng.next_u64(), &sender.1).into()
            })
            .collect();

        testkit.create_block_with_transactions(txs);

        // Test invariants that should be maintained during fuzz testing
        let schema = CurrencySchema::new(testkit.snapshot());
        let wallets = schema.wallets();
        let wallets: Vec<_> = wallets.values().collect();
        // There must be 2 wallets in the storage
        assert_eq!(wallets.len(), 2);
        // These wallets should belong to Alice and Bob
        assert_eq!(
            BTreeSet::from_iter(wallets.iter().map(Wallet::pub_key)),
            BTreeSet::from_iter(vec![&alice_keys.0, &bob_keys.0])
        );
        // The total amount of funds should equal 200, no matter which transactions were executed
        assert_eq!(wallets.iter().fold(0, |acc, w| acc + w.balance()), 200);
    }
}
