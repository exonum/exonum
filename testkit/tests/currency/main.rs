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

//! Tests for sample currency service almost identical to one implemented in
//! the [`cryptocurrency`] tutorial.
//!
//! [`cryptocurrency`]: https://github.com/exonum/cryptocurrency/

#[macro_use]
extern crate exonum;
#[macro_use]
extern crate exonum_testkit;
#[macro_use]
extern crate serde_derive;

use std::collections::BTreeSet;
use std::iter::FromIterator;

use exonum::crypto::{self, PublicKey, SecretKey, CryptoHash};
use exonum_testkit::{ApiKind, ComparableSnapshot, TestKit, TestKitApi, TestKitBuilder};

mod cryptocurrency;
use cryptocurrency::{CurrencySchema, CurrencyService, TransactionResponse, TxCreateWallet,
                     TxTransfer, Wallet};

fn init_testkit() -> TestKit {
    TestKitBuilder::validator()
        .with_validators(4)
        .with_service(CurrencyService)
        .create()
}

fn create_wallet(api: &TestKitApi, name: &str) -> (TxCreateWallet, SecretKey) {
    let (pubkey, key) = crypto::gen_keypair();
    // Create a presigned transaction
    let tx = TxCreateWallet::new(&pubkey, name, &key);

    let tx_info: TransactionResponse = api.post(
        ApiKind::Service("cryptocurrency"),
        "v1/wallets/transaction",
        &tx,
    );
    assert_eq!(tx_info.tx_hash, tx.hash());

    (tx, key)
}

fn transfer(api: &TestKitApi, tx: &TxTransfer) {
    let tx_info: TransactionResponse = api.post(
        ApiKind::Service("cryptocurrency"),
        "v1/wallets/transaction",
        tx,
    );
    assert_eq!(tx_info.tx_hash, tx.hash());
}

fn get_wallet(api: &TestKitApi, pubkey: &PublicKey) -> Wallet {
    api.get(
        ApiKind::Service("cryptocurrency"),
        &format!("v1/wallet/{}", pubkey.to_string()),
    )
}

fn get_all_wallets(api: &TestKitApi) -> Vec<Wallet> {
    api.get(ApiKind::Service("cryptocurrency"), "v1/wallets")
}

#[test]
fn test_create_wallet() {
    let mut testkit = init_testkit();
    let api = testkit.api();
    let (tx, _) = create_wallet(&api, "Alice");

    testkit.create_block();

    // Check that the user indeed is persisted by the service
    let wallet = get_wallet(&api, tx.pub_key());
    assert_eq!(wallet.pub_key(), tx.pub_key());
    assert_eq!(wallet.name(), tx.name());
    assert_eq!(wallet.balance(), 100);
}

#[test]
fn test_transfer() {
    let mut testkit = init_testkit();
    let api = testkit.api();

    let (tx_alice, key_alice) = create_wallet(&api, "Alice");
    let (tx_bob, _) = create_wallet(&api, "Bob");
    // Commit creation transactions
    testkit.create_block();

    // Check that the initial Alice's and Bob's balances are persisted by the service
    let wallet = get_wallet(&api, tx_alice.pub_key());
    assert_eq!(wallet.balance(), 100);
    let wallet = get_wallet(&api, tx_bob.pub_key());
    assert_eq!(wallet.balance(), 100);

    // Transfer funds
    let tx = TxTransfer::new(
        tx_alice.pub_key(),
        tx_bob.pub_key(),
        10, // amount
        0, // seed
        &key_alice,
    );
    transfer(&api, &tx);
    testkit.create_block();

    let wallet = get_wallet(&api, tx_alice.pub_key());
    assert_eq!(wallet.balance(), 90);
    let wallet = get_wallet(&api, tx_bob.pub_key());
    assert_eq!(wallet.balance(), 110);
}

#[test]
fn test_snapshot_completeness() {
    let mut testkit = init_testkit();
    let api = testkit.api();

    let (tx_alice, _) = create_wallet(&api, "Alice");
    testkit.create_block();

    let (tx_bob, _) = create_wallet(&api, "Bob");
    // Check that Alice's wallet is in the snapshot
    testkit
        .probe(tx_bob)
        .compare(testkit.snapshot())
        .map(CurrencySchema::new)
        .map(|schema| schema.wallet(tx_alice.pub_key()))
        .assert_inv("Alice's wallet is there", Option::is_some)
        .map(|w| w.as_ref().unwrap().balance())
        .assert_eq("Alice's balance hasn't changed");
}

#[test]
fn test_transfer_from_nonexisting_wallet() {
    let mut testkit = init_testkit();
    let api = testkit.api();

    let (tx_alice, key_alice) = create_wallet(&api, "Alice");
    let (tx_bob, _) = create_wallet(&api, "Bob");
    // Do not commit Alice's transaction
    testkit.create_block_with_tx_hashes(&[tx_bob.hash()]);

    let wallet = get_wallet(&api, tx_bob.pub_key());
    assert_eq!(wallet.balance(), 100);

    let tx = TxTransfer::new(
        tx_alice.pub_key(),
        tx_bob.pub_key(),
        10, // amount
        0, // seed
        &key_alice,
    );

    let comp = testkit.probe(tx).compare(testkit.snapshot());
    let comp = comp.map(CurrencySchema::new);
    comp.map(|s| s.wallet(tx_alice.pub_key())).assert_inv(
        "No Alice's wallet",
        Option::is_none,
    );
    comp.map(|s| {
        s.wallet(tx_bob.pub_key())
            .expect("No Bob's wallet!")
            .balance()
    }).assert_eq("Bob's balance hasn't changed");
}

#[test]
fn test_transfer_to_nonexisting_wallet() {
    let mut testkit = init_testkit();
    let api = testkit.api();

    let (tx_alice, key_alice) = create_wallet(&api, "Alice");
    let (tx_bob, _) = create_wallet(&api, "Bob");
    // Do not commit Bob's transaction
    testkit.create_block_with_tx_hashes(&[tx_alice.hash()]);

    let wallet = get_wallet(&api, tx_alice.pub_key());
    assert_eq!(wallet.balance(), 100);

    let tx = TxTransfer::new(
        tx_alice.pub_key(),
        tx_bob.pub_key(),
        10, // amount
        0, // seed
        &key_alice,
    );
    transfer(&api, &tx);

    let old_snapshot = testkit.snapshot();
    testkit.create_block_with_tx_hashes(&[tx.hash()]);

    let comp = testkit.snapshot().compare(old_snapshot);
    let comp = comp.map(CurrencySchema::new);
    comp.map(|s| s.wallet(tx_bob.pub_key())).assert_inv(
        "No Bob's wallet",
        Option::is_none,
    );
    comp.map(|s| {
        s.wallet(tx_alice.pub_key())
            .expect("No Alice's wallet!")
            .balance()
    }).assert_eq("Alice's balance hasn't changed");
}

#[test]
fn test_transfer_overcharge() {
    let mut testkit = init_testkit();
    let api = testkit.api();

    let (tx_alice, key_alice) = create_wallet(&api, "Alice");
    let (tx_bob, _) = create_wallet(&api, "Bob");
    testkit.create_block();

    // Transfer funds
    let tx = TxTransfer::new(
        tx_alice.pub_key(),
        tx_bob.pub_key(),
        110, // amount
        0, // seed
        &key_alice,
    );
    transfer(&api, &tx);
    testkit.create_block();

    let wallet = get_wallet(&api, tx_alice.pub_key());
    assert_eq!(wallet.balance(), 100);
    let wallet = get_wallet(&api, tx_bob.pub_key());
    assert_eq!(wallet.balance(), 100);
}

#[test]
fn test_transfers_in_single_block() {
    let mut testkit = init_testkit();
    let api = testkit.api();

    let (tx_alice, key_alice) = create_wallet(&api, "Alice");
    let (tx_bob, key_bob) = create_wallet(&api, "Bob");
    testkit.create_block();

    // Transfer funds from Alice to Bob.
    let tx_a_to_b = TxTransfer::new(
        tx_alice.pub_key(),
        tx_bob.pub_key(),
        90, // amount
        0, // seed
        &key_alice,
    );

    // Transfer funds back from Bob to Alice.
    let tx_b_to_a = TxTransfer::new(
        tx_bob.pub_key(),
        tx_alice.pub_key(),
        120, // amount
        0, // seed
        &key_bob,
    );

    {
        // See what happens if transactions are applied in an "incorrect" order.
        let comp = testkit
            .probe_all(txvec![tx_b_to_a.clone(), tx_a_to_b.clone()])
            .compare(testkit.snapshot());
        let comp = comp.map(CurrencySchema::new);
        comp.map(|s| s.wallet(tx_alice.pub_key()).unwrap().balance())
            .assert("Alice's balance decreases", |&old, &new| new == old - 90);
        comp.map(|s| s.wallet(tx_bob.pub_key()).unwrap().balance())
            .assert("Bob's balance increases", |&old, &new| new == old + 90);
    }

    transfer(&api, &tx_a_to_b);
    transfer(&api, &tx_b_to_a);
    testkit.create_block_with_tx_hashes(&[tx_a_to_b.hash(), tx_b_to_a.hash()]);

    let wallet = get_wallet(&api, tx_alice.pub_key());
    assert_eq!(wallet.balance(), 130);
    let wallet = get_wallet(&api, tx_bob.pub_key());
    assert_eq!(wallet.balance(), 70);

    let wallets = get_all_wallets(&api);
    assert_eq!(wallets.len(), 2);
    assert_eq!(wallets.iter().fold(0, |acc, w| acc + w.balance()), 200);
    assert_eq!(
        BTreeSet::from_iter(wallets.iter().map(|w| *w.pub_key())),
        BTreeSet::from_iter(vec![*tx_alice.pub_key(), *tx_bob.pub_key()])
    );
}

#[test]
fn test_malformed_wallet_request() {
    let testkit = TestKitBuilder::validator()
        .with_validators(4)
        .with_service(CurrencyService)
        .create();
    let api = testkit.api();
    let info: String = api.get_err(ApiKind::Service("cryptocurrency"), "v1/wallet/c0ffee");
    assert!(info.starts_with("Invalid request param"));
}

#[test]
fn test_unknown_wallet_request() {
    let testkit = init_testkit();
    let api = testkit.api();

    // transaction is sent by API, but isn't committed
    let (tx_alice, _) = create_wallet(&api, "Alice");

    let info: String = api.get_err(
        ApiKind::Service("cryptocurrency"),
        &format!("v1/wallet/{}", tx_alice.pub_key().to_string()),
    );
    assert_eq!(info, "Wallet not found".to_string());
}

#[test]
fn test_nonverified_transaction_in_create_block() {
    let mut testkit = init_testkit();
    let api = testkit.api();

    let (create_tx, key) = create_wallet(&api, "Alice");
    testkit.create_block();

    // Send transaction to self. As this transaction fails `verify()`, it should not
    // be executed and increase sender's balance.
    let transfer_tx = TxTransfer::new(create_tx.pub_key(), create_tx.pub_key(), 10, 0, &key);
    testkit.create_block_with_transactions(txvec![transfer_tx]);

    let wallet = get_wallet(&api, create_tx.pub_key());
    assert_eq!(wallet.balance(), 100);
}

#[test]
fn test_nonsigned_transaction_in_create_block() {
    let mut testkit = init_testkit();
    let api = testkit.api();

    let (tx_alice, key) = create_wallet(&api, "Alice");
    let (tx_bob, _) = create_wallet(&api, "Bob");
    testkit.create_block();

    let transfer_tx = TxTransfer::new(tx_alice.pub_key(), tx_bob.pub_key(), 10, 0, &key);
    // Transaction with an incorrect signature (all zeros)
    let bogus_transfer_tx = TxTransfer::new_with_signature(
        tx_alice.pub_key(),
        tx_bob.pub_key(),
        20,
        0,
        &crypto::Signature::new([0; 64]),
    );
    // Check execution of a mix of correct and incorrect transactions
    testkit.create_block_with_transactions(txvec![bogus_transfer_tx, transfer_tx]);

    let wallet = get_wallet(&api, tx_alice.pub_key());
    assert_eq!(wallet.balance(), 90);
    let wallet = get_wallet(&api, tx_bob.pub_key());
    assert_eq!(wallet.balance(), 110);
}
