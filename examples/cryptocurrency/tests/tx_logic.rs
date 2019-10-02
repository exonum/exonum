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

//! These are tests concerning the business logic of transactions in the cryptocurrency service.
//! See `api.rs` for tests focused on the API of the service.
//!
//! Note how business logic tests use `TestKit::create_block*` methods to send transactions,
//! the service schema to make assertions about the storage state.

#[macro_use]
extern crate exonum_testkit;

use exonum::{
    crypto::{self, PublicKey, SecretKey},
    messages::{AnyTx, Verified},
    runtime::rust::Transaction,
};
use exonum_testkit::TestKit;

// Import data types used in tests from the crate where the service is defined.
use exonum_cryptocurrency::{
    contracts::CryptocurrencyService,
    schema::{CurrencySchema, Wallet},
    transactions::{Config, TxCreateWallet, TxTransfer},
};

// Imports shared test constants.
use crate::constants::{ALICE_NAME, BOB_NAME, INSTANCE_ID, INSTANCE_NAME};

mod constants;

#[test]
fn test_create_wallet() {
    let mut testkit = init_testkit();
    let (tx, _) = create_wallet(&mut testkit, ALICE_NAME.to_owned());

    // Check that the user indeed is persisted by the service
    let wallet = get_wallet(&testkit, &tx.author());
    assert_eq!(wallet.pub_key, tx.author());
    assert_eq!(wallet.name, ALICE_NAME);
    assert_eq!(wallet.balance, 100);
}

#[test]
fn test_transfer() {
    let mut testkit = init_testkit();
    let (alice_pubkey, alice_key) = crypto::gen_keypair();
    let (bob_pubkey, bob_key) = crypto::gen_keypair();
    testkit.create_block_with_transactions(txvec![
        TxCreateWallet {
            name: ALICE_NAME.to_owned()
        }
        .sign(INSTANCE_ID, alice_pubkey, &alice_key),
        TxCreateWallet {
            name: BOB_NAME.to_owned()
        }
        .sign(INSTANCE_ID, bob_pubkey, &bob_key),
        TxTransfer {
            amount: 10,
            seed: 0,
            to: bob_pubkey,
        }
        .sign(INSTANCE_ID, alice_pubkey, &alice_key),
    ]);

    let alice_wallet = get_wallet(&testkit, &alice_pubkey);
    assert_eq!(alice_wallet.balance, 90);

    let bob_wallet = get_wallet(&testkit, &bob_pubkey);
    assert_eq!(bob_wallet.balance, 110);
}

#[test]
fn test_transfer_from_nonexisting_wallet() {
    let mut testkit = init_testkit();
    let (alice_pubkey, alice_key) = crypto::gen_keypair();
    let (bob_pubkey, bob_key) = crypto::gen_keypair();
    testkit.create_block_with_transactions(txvec![
        TxCreateWallet {
            name: BOB_NAME.to_owned()
        }
        .sign(INSTANCE_ID, bob_pubkey, &bob_key),
        TxTransfer {
            amount: 10,
            seed: 0,
            to: bob_pubkey,
        }
        .sign(INSTANCE_ID, alice_pubkey, &alice_key),
    ]);

    assert!(try_get_wallet(&testkit, &alice_pubkey).is_none());

    let bob_wallet = get_wallet(&testkit, &bob_pubkey);
    assert_eq!(bob_wallet.balance, 100);
}

#[test]
fn test_transfer_to_nonexisting_wallet() {
    let mut testkit = init_testkit();
    let (alice_pubkey, alice_key) = crypto::gen_keypair();
    let (bob_pubkey, bob_key) = crypto::gen_keypair();
    testkit.create_block_with_transactions(txvec![
        TxCreateWallet {
            name: ALICE_NAME.to_owned()
        }
        .sign(INSTANCE_ID, alice_pubkey, &alice_key),
        TxTransfer {
            amount: 10,
            seed: 0,
            to: bob_pubkey,
        }
        .sign(INSTANCE_ID, alice_pubkey, &alice_key),
        // Although Bob's wallet is created, this occurs after the transfer is executed.
        TxCreateWallet {
            name: BOB_NAME.to_owned()
        }
        .sign(INSTANCE_ID, bob_pubkey, &bob_key),
    ]);

    let alice_wallet = get_wallet(&testkit, &alice_pubkey);
    assert_eq!(alice_wallet.balance, 100);

    let bob_wallet = get_wallet(&testkit, &bob_pubkey);
    assert_eq!(bob_wallet.balance, 100);
}

#[test]
fn test_transfer_overcharge() {
    let mut testkit = init_testkit();
    let (alice_pubkey, alice_key) = crypto::gen_keypair();
    let (bob_pubkey, bob_key) = crypto::gen_keypair();
    testkit.create_block_with_transactions(txvec![
        TxCreateWallet {
            name: ALICE_NAME.to_owned()
        }
        .sign(INSTANCE_ID, alice_pubkey, &alice_key),
        TxCreateWallet {
            name: BOB_NAME.to_owned()
        }
        .sign(INSTANCE_ID, bob_pubkey, &bob_key),
        TxTransfer {
            to: bob_pubkey,
            amount: 150,
            seed: 0
        }
        .sign(INSTANCE_ID, alice_pubkey, &alice_key),
    ]);

    // The transfer amount is greater than what Alice has at her disposal, so
    // the transfer should fail.
    let alice_wallet = get_wallet(&testkit, &alice_pubkey);
    assert_eq!(alice_wallet.balance, 100);

    let bob_wallet = get_wallet(&testkit, &bob_pubkey);
    assert_eq!(bob_wallet.balance, 100);
}

#[test]
fn test_transfers_in_single_block() {
    let mut testkit = init_testkit();
    let (alice_pubkey, alice_key) = crypto::gen_keypair();
    let (bob_pubkey, bob_key) = crypto::gen_keypair();
    testkit.create_block_with_transactions(txvec![
        TxCreateWallet {
            name: ALICE_NAME.to_owned()
        }
        .sign(INSTANCE_ID, alice_pubkey, &alice_key),
        TxCreateWallet {
            name: BOB_NAME.to_owned()
        }
        .sign(INSTANCE_ID, bob_pubkey, &bob_key),
    ]);

    let tx_a_to_b = TxTransfer {
        to: bob_pubkey,
        amount: 90,
        seed: 0,
    }
    .sign(INSTANCE_ID, alice_pubkey, &alice_key);
    let tx_b_to_a = TxTransfer {
        to: alice_pubkey,
        amount: 120,
        seed: 0,
    }
    .sign(INSTANCE_ID, bob_pubkey, &bob_key);

    {
        // See what happens if transactions are applied in an "incorrect" order.
        // We use `TestKit::probe_all()` method for this.

        let snapshot = testkit.probe_all(txvec![tx_b_to_a.clone(), tx_a_to_b.clone()]);
        let schema = CurrencySchema::new(INSTANCE_NAME, &snapshot);
        assert_eq!(schema.wallet(&alice_pubkey).map(|w| w.balance), Some(10));
        assert_eq!(schema.wallet(&bob_pubkey).map(|w| w.balance), Some(190));
    }

    testkit.create_block_with_transactions(txvec![tx_a_to_b, tx_b_to_a]);

    let alice_wallet = get_wallet(&testkit, &alice_pubkey);
    assert_eq!(alice_wallet.balance, 130);

    let bob_wallet = get_wallet(&testkit, &bob_pubkey);
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
    use std::collections::BTreeSet;
    use std::iter::FromIterator;

    const BLOCKS: usize = 50; // number of blocks to create
    const MAX_TRANSACTIONS: usize = 20; // maximum number of transactions in a block

    let mut rng = rand::thread_rng();

    let mut testkit = init_testkit();
    let alice_keys = crypto::gen_keypair();
    let bob_keys = crypto::gen_keypair();
    let keys = &[alice_keys.clone(), bob_keys.clone()];
    testkit.create_block_with_transactions(txvec![
        TxCreateWallet {
            name: ALICE_NAME.to_owned()
        }
        .sign(INSTANCE_ID, alice_keys.0, &alice_keys.1),
        TxCreateWallet {
            name: BOB_NAME.to_owned()
        }
        .sign(INSTANCE_ID, bob_keys.0, &bob_keys.1),
    ]);

    for _ in 0..BLOCKS {
        let n_txs = rng.gen_range(0, MAX_TRANSACTIONS); // number of transactions in the block

        let txs: Vec<Verified<AnyTx>> = (0..n_txs)
            .map(|_| {
                let (sender, receiver) = (
                    keys.choose(&mut rng).unwrap(),
                    keys.choose(&mut rng).unwrap(),
                );
                let amount = rng.gen_range(0, 250);

                TxTransfer {
                    to: receiver.0,
                    amount,
                    seed: rng.gen::<u64>(),
                }
                .sign(INSTANCE_ID, sender.0, &sender.1)
            })
            .collect();

        testkit.create_block_with_transactions(txs);

        // Test invariants that should be maintained during fuzz testing.
        let snapshot = testkit.snapshot();
        let schema = CurrencySchema::new(INSTANCE_NAME, &snapshot);
        let wallets = schema.wallets();
        let wallets: Vec<_> = wallets.values().collect();
        // There must be 2 wallets in the storage.
        assert_eq!(wallets.len(), 2);
        // These wallets should belong to Alice and Bob.
        assert_eq!(
            BTreeSet::from_iter(wallets.iter().map(|w| &w.pub_key)),
            BTreeSet::from_iter(vec![&alice_keys.0, &bob_keys.0])
        );
        // The total amount of funds should equal 200, no matter which transactions were executed.
        assert_eq!(wallets.iter().map(|w| w.balance).sum::<u64>(), 200);
    }
}

/// Initializes testkit with `CurrencyService`.
fn init_testkit() -> TestKit {
    TestKit::for_rust_service(CryptocurrencyService, INSTANCE_NAME, INSTANCE_ID, Config)
}

/// Creates a wallet with the given name and a random key.
fn create_wallet(testkit: &mut TestKit, name: String) -> (Verified<AnyTx>, SecretKey) {
    let (pubkey, key) = crypto::gen_keypair();
    let tx = TxCreateWallet { name }.sign(INSTANCE_ID, pubkey, &key);
    testkit.create_block_with_transaction(tx.clone());
    (tx, key)
}

/// Returns the wallet identified by the given public key or `None` such wallet doesn't exist.
fn try_get_wallet(testkit: &TestKit, pubkey: &PublicKey) -> Option<Wallet> {
    let snapshot = testkit.snapshot();
    CurrencySchema::new(INSTANCE_NAME, &snapshot).wallet(pubkey)
}

/// Returns the wallet identified by the given public key.
fn get_wallet(testkit: &TestKit, pubkey: &PublicKey) -> Wallet {
    try_get_wallet(testkit, pubkey).expect("No wallet persisted")
}
