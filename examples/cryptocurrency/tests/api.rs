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

//! These are tests concerning the API of the cryptocurrency service. See `tx_logic.rs`
//! for tests focused on the business logic of transactions.
//!
//! Note how API tests predominantly use `TestKitApi` to send transactions and make assertions
//! about the storage state.

#[macro_use]
extern crate assert_matches;
extern crate exonum;
extern crate exonum_cryptocurrency as cryptocurrency;
extern crate exonum_testkit;
#[macro_use]
extern crate serde_json;

use exonum::api::ApiError;
use exonum::crypto::{self, CryptoHash, Hash, PublicKey, SecretKey};
use exonum_testkit::{ApiKind, TestKit, TestKitApi, TestKitBuilder};

// Import data types used in tests from the crate where the service is defined.
use cryptocurrency::schema::Wallet;
use cryptocurrency::service::CurrencyService;
use cryptocurrency::transactions::{TxCreateWallet, TxTransfer};

// Imports shared test constants.
use constants::{ALICE_NAME, BOB_NAME};

mod constants;

/// Check that the wallet creation transaction works when invoked via API.
#[test]
fn test_create_wallet() {
    let (mut testkit, api) = create_testkit();
    // Create and send a transaction via API
    let (tx, _) = api.create_wallet(ALICE_NAME);
    testkit.create_block();
    api.assert_tx_status(&tx.hash(), &json!({ "type": "success" }));

    // Check that the user indeed is persisted by the service.
    let wallet = api.get_wallet(tx.pub_key());
    assert_eq!(wallet.pub_key(), tx.pub_key());
    assert_eq!(wallet.name(), tx.name());
    assert_eq!(wallet.balance(), 100);
}

/// Check that the transfer transaction works as intended.
#[test]
fn test_transfer() {
    // Create 2 wallets.
    let (mut testkit, api) = create_testkit();
    let (tx_alice, key_alice) = api.create_wallet(ALICE_NAME);
    let (tx_bob, _) = api.create_wallet(BOB_NAME);
    testkit.create_block();
    api.assert_tx_status(&tx_alice.hash(), &json!({ "type": "success" }));
    api.assert_tx_status(&tx_bob.hash(), &json!({ "type": "success" }));

    // Check that the initial Alice's and Bob's balances persisted by the service.
    let wallet = api.get_wallet(tx_alice.pub_key());
    assert_eq!(wallet.balance(), 100);
    let wallet = api.get_wallet(tx_bob.pub_key());
    assert_eq!(wallet.balance(), 100);

    // Transfer funds by invoking the corresponding API method.
    let tx = TxTransfer::new(
        tx_alice.pub_key(),
        tx_bob.pub_key(),
        10, // transferred amount
        0,  // seed
        &key_alice,
    );
    api.transfer(&tx);
    testkit.create_block();
    api.assert_tx_status(&tx.hash(), &json!({ "type": "success" }));

    // After the transfer transaction is included into a block, we may check new wallet
    // balances.
    let wallet = api.get_wallet(tx_alice.pub_key());
    assert_eq!(wallet.balance(), 90);
    let wallet = api.get_wallet(tx_bob.pub_key());
    assert_eq!(wallet.balance(), 110);
}

/// Check that a transfer from a non-existing wallet fails as expected.
#[test]
fn test_transfer_from_nonexisting_wallet() {
    let (mut testkit, api) = create_testkit();

    let (tx_alice, key_alice) = api.create_wallet(ALICE_NAME);
    let (tx_bob, _) = api.create_wallet(BOB_NAME);
    // Do not commit Alice's transaction, so Alice's wallet does not exist
    // when a transfer occurs.
    testkit.create_block_with_tx_hashes(&[tx_bob.hash()]);

    api.assert_no_wallet(tx_alice.pub_key());
    let wallet = api.get_wallet(tx_bob.pub_key());
    assert_eq!(wallet.balance(), 100);

    let tx = TxTransfer::new(
        tx_alice.pub_key(),
        tx_bob.pub_key(),
        10, // transfer amount
        0,  // seed
        &key_alice,
    );
    api.transfer(&tx);
    testkit.create_block_with_tx_hashes(&[tx.hash()]);
    api.assert_tx_status(
        &tx.hash(),
        &json!({ "type": "error", "code": 1, "description": "Sender doesn't exist" }),
    );

    // Check that Bob's balance doesn't change.
    let wallet = api.get_wallet(tx_bob.pub_key());
    assert_eq!(wallet.balance(), 100);
}

/// Check that a transfer to a non-existing wallet fails as expected.
#[test]
fn test_transfer_to_nonexisting_wallet() {
    let (mut testkit, api) = create_testkit();

    let (tx_alice, key_alice) = api.create_wallet(ALICE_NAME);
    let (tx_bob, _) = api.create_wallet(BOB_NAME);
    // Do not commit Bob's transaction, so Bob's wallet does not exist
    // when a transfer occurs.
    testkit.create_block_with_tx_hashes(&[tx_alice.hash()]);

    let wallet = api.get_wallet(tx_alice.pub_key());
    assert_eq!(wallet.balance(), 100);
    api.assert_no_wallet(tx_bob.pub_key());

    let tx = TxTransfer::new(
        tx_alice.pub_key(),
        tx_bob.pub_key(),
        10, // transfer amount
        0,  // seed
        &key_alice,
    );
    api.transfer(&tx);
    testkit.create_block_with_tx_hashes(&[tx.hash()]);
    api.assert_tx_status(
        &tx.hash(),
        &json!({ "type": "error", "code": 2, "description": "Receiver doesn't exist" }),
    );

    // Check that Alice's balance doesn't change.
    let wallet = api.get_wallet(tx_alice.pub_key());
    assert_eq!(wallet.balance(), 100);
}

/// Check that an overcharge does not lead to changes in sender's and receiver's balances.
#[test]
fn test_transfer_overcharge() {
    let (mut testkit, api) = create_testkit();

    let (tx_alice, key_alice) = api.create_wallet(ALICE_NAME);
    let (tx_bob, _) = api.create_wallet(BOB_NAME);
    testkit.create_block();

    // Transfer funds. The transfer amount (110) is more than Alice has (100).
    let tx = TxTransfer::new(
        tx_alice.pub_key(),
        tx_bob.pub_key(),
        110, // transfer amount
        0,   // seed
        &key_alice,
    );
    api.transfer(&tx);
    testkit.create_block();
    api.assert_tx_status(
        &tx.hash(),
        &json!({ "type": "error", "code": 3, "description": "Insufficient currency amount" }),
    );

    let wallet = api.get_wallet(tx_alice.pub_key());
    assert_eq!(wallet.balance(), 100);
    let wallet = api.get_wallet(tx_bob.pub_key());
    assert_eq!(wallet.balance(), 100);
}

#[test]
fn test_malformed_wallet_request() {
    let (_testkit, api) = create_testkit();

    let info = api.inner
        .get_err(ApiKind::Service("cryptocurrency"), "v1/wallet/c0ffee");
    assert_matches!(
        info,
        ApiError::BadRequest(ref body) if body.starts_with("Invalid request param")
    );
}

#[test]
fn test_unknown_wallet_request() {
    let (_testkit, api) = create_testkit();

    // Transaction is sent by API, but isn't committed.
    let (tx, _) = api.create_wallet(ALICE_NAME);

    let info = api.inner.get_err(
        ApiKind::Service("cryptocurrency"),
        &format!("v1/wallet/{}", tx.pub_key().to_string()),
    );
    assert_matches!(
        info,
        ApiError::NotFound(ref body) if body == "Wallet not found"
    );
}

/// Wrapper for the cryptocurrency service API allowing to easily use it
/// (compared to `TestKitApi` calls).
struct CryptocurrencyApi {
    pub inner: TestKitApi,
}

impl CryptocurrencyApi {
    /// Generates a wallet creation transaction with a random key pair, sends it over HTTP,
    /// and checks the synchronous result (i.e., the hash of the transaction returned
    /// within the response).
    /// Note that the transaction is not immediately added to the blockchain, but rather is put
    /// to the pool of unconfirmed transactions.
    fn create_wallet(&self, name: &str) -> (TxCreateWallet, SecretKey) {
        let (pubkey, key) = crypto::gen_keypair();
        // Create a pre-signed transaction
        let tx = TxCreateWallet::new(&pubkey, name, &key);

        let tx_info: serde_json::Value =
            self.inner
                .post(ApiKind::Service("cryptocurrency"), "v1/wallets", &tx);
        assert_eq!(tx_info, json!({ "tx_hash": tx.hash() }));
        (tx, key)
    }

    /// Sends a transfer transaction over HTTP and checks the synchronous result.
    fn transfer(&self, tx: &TxTransfer) {
        let tx_info: serde_json::Value = self.inner.post(
            ApiKind::Service("cryptocurrency"),
            "v1/wallets/transfer",
            tx,
        );
        assert_eq!(tx_info, json!({ "tx_hash": tx.hash() }));
    }

    /// Gets the state of a particular wallet using an HTTP request.
    fn get_wallet(&self, pubkey: &PublicKey) -> Wallet {
        self.inner.get(
            ApiKind::Service("cryptocurrency"),
            &format!("v1/wallet/{}", pubkey.to_string()),
        )
    }

    /// Asserts that a wallet with the specified public key is not known to the blockchain.
    fn assert_no_wallet(&self, pubkey: &PublicKey) {
        let err = self.inner.get_err(
            ApiKind::Service("cryptocurrency"),
            &format!("v1/wallet/{}", pubkey.to_string()),
        );

        assert_matches!(
            err,
            ApiError::NotFound(ref body) if body == "Wallet not found"
        );
    }

    /// Asserts that the transaction with the given hash has a specified status.
    fn assert_tx_status(&self, tx_hash: &Hash, expected_status: &serde_json::Value) {
        let info: serde_json::Value = self.inner.get(
            ApiKind::Explorer,
            &format!("v1/transactions/{}", tx_hash.to_string()),
        );
        if let serde_json::Value::Object(mut info) = info {
            let tx_status = info.remove("status").unwrap();
            assert_eq!(tx_status, *expected_status);
        } else {
            panic!("Invalid transaction info format, object expected");
        }
    }
}

/// Creates a testkit together with the API wrapper defined above.
fn create_testkit() -> (TestKit, CryptocurrencyApi) {
    let testkit = TestKitBuilder::validator()
        .with_service(CurrencyService)
        .create();
    let api = CryptocurrencyApi {
        inner: testkit.api(),
    };
    (testkit, api)
}
