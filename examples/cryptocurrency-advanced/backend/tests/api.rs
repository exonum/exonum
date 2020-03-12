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

//! These are tests concerning the API of the cryptocurrency service.
//!
//! Note how API tests predominantly use `TestKitApi` to send transactions and make assertions
//! about the storage state.

use exonum::{
    blockchain::IndexProof,
    crypto::{Hash, KeyPair, PublicKey},
    merkledb::ObjectHash,
    messages::{AnyTx, Verified},
    runtime::{Caller, CallerAddress, SnapshotExt},
};
use exonum_explorer_service::ExplorerFactory;
use exonum_testkit::{
    explorer::api::{TransactionQuery, TransactionResponse},
    ApiKind, Spec, TestKit, TestKitApi, TestKitBuilder,
};
use serde_json::json;

// Import data types used in tests from the crate where the service is defined.
use exonum_cryptocurrency_advanced::{
    api::{WalletInfo, WalletQuery},
    schema::Schema,
    transactions::{CreateWallet, Transfer},
    wallet::Wallet,
    CryptocurrencyInterface, CryptocurrencyService,
};

/// Alice's wallets name.
const ALICE_NAME: &str = "Alice";
/// Bob's wallet name.
const BOB_NAME: &str = "Bob";
/// Service instance ID.
const SERVICE_ID: u32 = 120;
/// Service instance name.
const SERVICE_NAME: &str = "tst-token";

fn author_address(tx: &Verified<AnyTx>) -> CallerAddress {
    CallerAddress::from_key(tx.author())
}

/// Check that the wallet creation transaction works when invoked via API.
#[tokio::test]
async fn test_create_wallet() {
    let (mut testkit, api) = create_testkit();
    // Create and send a transaction via API
    let (tx, _) = api.create_wallet(ALICE_NAME).await;
    testkit.create_block();
    api.assert_tx_status(tx.object_hash(), &json!({ "type": "success" }))
        .await;

    // Check that the user indeed is persisted by the service.
    let wallet = api.get_wallet(tx.author()).await.unwrap();
    assert_eq!(wallet.owner, author_address(&tx));
    assert_eq!(wallet.name, ALICE_NAME);
    assert_eq!(wallet.balance, 100);
}

/// Check that the transfer transaction works as intended.
#[tokio::test]
async fn test_transfer() {
    // Create 2 wallets.
    let (mut testkit, api) = create_testkit();
    let (tx_alice, alice) = api.create_wallet(ALICE_NAME).await;
    let (tx_bob, _) = api.create_wallet(BOB_NAME).await;
    testkit.create_block();
    api.assert_tx_status(tx_alice.object_hash(), &json!({ "type": "success" }))
        .await;
    api.assert_tx_status(tx_bob.object_hash(), &json!({ "type": "success" }))
        .await;

    // Check that the initial Alice's and Bob's balances persisted by the service.
    let wallet = api.get_wallet(tx_alice.author()).await.unwrap();
    assert_eq!(wallet.balance, 100);
    let wallet = api.get_wallet(tx_bob.author()).await.unwrap();
    assert_eq!(wallet.balance, 100);

    // Transfer funds by invoking the corresponding API method.
    let tx = alice.transfer(
        SERVICE_ID,
        Transfer {
            to: author_address(&tx_bob),
            amount: 10,
            seed: 10,
        },
    );

    api.transfer(&tx).await;
    testkit.create_block();
    api.assert_tx_status(tx.object_hash(), &json!({ "type": "success" }))
        .await;

    // After the transfer transaction is included into a block, we may check new wallet
    // balances.
    let wallet = api.get_wallet(tx_alice.author()).await.unwrap();
    assert_eq!(wallet.balance, 90);
    let wallet = api.get_wallet(tx_bob.author()).await.unwrap();
    assert_eq!(wallet.balance, 110);

    // Check the balances via public schema.
    let snapshot = testkit.snapshot();
    let schema: Schema<_> = snapshot.service_schema(SERVICE_ID).unwrap();
    let alice_wallet = schema.wallets.get(&author_address(&tx_alice)).unwrap();
    assert_eq!(alice_wallet.balance, 90);
    let bob_wallet = schema.wallets.get(&author_address(&tx_bob)).unwrap();
    assert_eq!(bob_wallet.balance, 110);
}

/// Check that a transfer from a non-existing wallet fails as expected.
#[tokio::test]
async fn test_transfer_from_nonexisting_wallet() {
    let (mut testkit, api) = create_testkit();

    let (tx_alice, alice) = api.create_wallet(ALICE_NAME).await;
    let (tx_bob, _) = api.create_wallet(BOB_NAME).await;
    // Do not commit Alice's transaction, so Alice's wallet does not exist
    // when a transfer occurs.
    testkit.create_block_with_tx_hashes(&[tx_bob.object_hash()]);

    api.assert_no_wallet(tx_alice.author()).await;
    let wallet = api.get_wallet(tx_bob.author()).await.unwrap();
    assert_eq!(wallet.balance, 100);

    let tx = alice.transfer(
        SERVICE_ID,
        Transfer {
            to: author_address(&tx_bob),
            amount: 10,
            seed: 0,
        },
    );

    api.transfer(&tx).await;
    testkit.create_block_with_tx_hashes(&[tx.object_hash()]);
    let expected_status = json!({
        "type": "service_error",
        "code": 1,
        "description": "Sender doesn\'t exist.\n\nCan be emitted by `Transfer`.",
        "runtime_id": 0,
        "call_site": {
            "call_type": "method",
            "instance_id": SERVICE_ID,
            "method_id": 0,
        },
    });
    api.assert_tx_status(tx.object_hash(), &expected_status)
        .await;

    // Check that Bob's balance doesn't change.
    let wallet = api.get_wallet(tx_bob.author()).await.unwrap();
    assert_eq!(wallet.balance, 100);

    // Same check via schema.
    let snapshot = testkit.snapshot();
    let schema: Schema<_> = snapshot.service_schema(SERVICE_ID).unwrap();
    let wallet = schema.wallets.get(&author_address(&tx_bob)).unwrap();
    assert_eq!(wallet.balance, 100);
}

/// Check that a transfer to a non-existing wallet fails as expected.
#[tokio::test]
async fn test_transfer_to_nonexisting_wallet() {
    let (mut testkit, api) = create_testkit();

    let (tx_alice, alice) = api.create_wallet(ALICE_NAME).await;
    let (tx_bob, _) = api.create_wallet(BOB_NAME).await;
    // Do not commit Bob's transaction, so Bob's wallet does not exist
    // when a transfer occurs.
    testkit.create_block_with_tx_hashes(&[tx_alice.object_hash()]);

    let wallet = api.get_wallet(tx_alice.author()).await.unwrap();
    assert_eq!(wallet.balance, 100);
    api.assert_no_wallet(tx_bob.author()).await;

    let tx = alice.transfer(
        SERVICE_ID,
        Transfer {
            to: author_address(&tx_bob),
            amount: 10,
            seed: 0,
        },
    );

    api.transfer(&tx).await;
    testkit.create_block_with_tx_hashes(&[tx.object_hash()]);
    let expected_status = json!({
        "type": "service_error",
        "code": 2,
        "description": "Receiver doesn\'t exist.\n\nCan be emitted by `Transfer` or `Issue`.",
        "runtime_id": 0,
        "call_site": {
            "call_type": "method",
            "instance_id": SERVICE_ID,
            "method_id": 0,
        },
    });
    api.assert_tx_status(tx.object_hash(), &expected_status)
        .await;

    // Check that Alice's balance doesn't change.
    let wallet = api.get_wallet(tx_alice.author()).await.unwrap();
    assert_eq!(wallet.balance, 100);
}

/// Check that an overcharge does not lead to changes in sender's and receiver's balances.
#[tokio::test]
async fn test_transfer_overcharge() {
    let (mut testkit, api) = create_testkit();

    let (tx_alice, alice) = api.create_wallet(ALICE_NAME).await;
    let (tx_bob, _) = api.create_wallet(BOB_NAME).await;
    testkit.create_block();

    // Transfer funds. The transfer amount (110) is more than Alice has (100).
    let tx = alice.transfer(
        SERVICE_ID,
        Transfer {
            to: author_address(&tx_bob),
            amount: 110,
            seed: 0,
        },
    );

    api.transfer(&tx).await;
    testkit.create_block();
    let expected_status = json!({
        "type": "service_error",
        "code": 3,
        "description": "Insufficient currency amount.\n\nCan be emitted by `Transfer`.",
        "runtime_id": 0,
        "call_site": {
            "call_type": "method",
            "instance_id": SERVICE_ID,
            "method_id": 0,
        },
    });
    api.assert_tx_status(tx.object_hash(), &expected_status)
        .await;

    let wallet = api.get_wallet(tx_alice.author()).await.unwrap();
    assert_eq!(wallet.balance, 100);
    let wallet = api.get_wallet(tx_bob.author()).await.unwrap();
    assert_eq!(wallet.balance, 100);
}

#[tokio::test]
async fn test_unknown_wallet_request() {
    let (_testkit, api) = create_testkit();
    // Transaction is sent by API, but isn't committed.
    let (tx, _) = api.create_wallet(ALICE_NAME).await;
    api.assert_no_wallet(tx.author()).await;
}

/// Wrapper for the cryptocurrency service API allowing to easily use it
/// (compared to `TestKitApi` calls).
struct CryptocurrencyApi {
    validator_keys: Vec<PublicKey>,
    inner: TestKitApi,
}

impl CryptocurrencyApi {
    /// Generates a wallet creation transaction with a random key pair, sends it over HTTP,
    /// and checks the synchronous result (i.e., the hash of the transaction returned
    /// within the response).
    /// Note that the transaction is not immediately added to the blockchain, but rather is put
    /// to the pool of unconfirmed transactions.
    async fn create_wallet(&self, name: &str) -> (Verified<AnyTx>, KeyPair) {
        let keypair = KeyPair::random();
        // Create a pre-signed transaction.
        let tx = keypair.create_wallet(SERVICE_ID, CreateWallet::new(name));

        let tx_info: TransactionResponse = self
            .inner
            .public(ApiKind::Explorer)
            .query(&json!({ "tx_body": tx }))
            .post("v1/transactions")
            .await
            .unwrap();
        assert_eq!(tx_info.tx_hash, tx.object_hash());
        (tx, keypair)
    }

    async fn get_wallet(&self, pub_key: PublicKey) -> Option<Wallet> {
        let wallet_info = self
            .inner
            .public(ApiKind::Service(SERVICE_NAME))
            .query(&WalletQuery { pub_key })
            .get::<WalletInfo>("v1/wallets/info")
            .await
            .unwrap();

        // Check parts of the proof returned together with the wallet.
        let index_proof =
            IndexProof::new(wallet_info.block_proof, wallet_info.wallet_proof.to_table);
        let (index_name, index_hash) = index_proof.verify(&self.validator_keys).unwrap();
        assert_eq!(index_name, format!("{}.wallets", SERVICE_NAME));

        let to_wallet = wallet_info
            .wallet_proof
            .to_wallet
            .check_against_hash(index_hash)
            .unwrap();
        let address = Caller::Transaction { author: pub_key }.address();
        let (_, wallet) = to_wallet.all_entries().find(|(&key, _)| key == address)?;
        wallet.cloned()
    }

    /// Sends a transfer transaction over HTTP and checks the synchronous result.
    async fn transfer(&self, tx: &Verified<AnyTx>) {
        let tx_info: TransactionResponse = self
            .inner
            .public(ApiKind::Explorer)
            .query(&json!({ "tx_body": tx }))
            .post("v1/transactions")
            .await
            .unwrap();
        assert_eq!(tx_info.tx_hash, tx.object_hash());
    }

    /// Asserts that a wallet with the specified public key is not known to the blockchain.
    async fn assert_no_wallet(&self, pub_key: PublicKey) {
        let wallet_info: WalletInfo = self
            .inner
            .public(ApiKind::Service(SERVICE_NAME))
            .query(&WalletQuery { pub_key })
            .get("v1/wallets/info")
            .await
            .unwrap();

        let to_wallet = wallet_info.wallet_proof.to_wallet.check().unwrap();
        let address = Caller::Transaction { author: pub_key }.address();
        assert!(to_wallet.missing_keys().any(|&key| key == address))
    }

    /// Asserts that the transaction with the given hash has a specified status.
    async fn assert_tx_status(&self, tx_hash: Hash, expected_status: &serde_json::Value) {
        let info: serde_json::Value = self
            .inner
            .public(ApiKind::Explorer)
            .query(&TransactionQuery::new(tx_hash))
            .get("v1/transactions")
            .await
            .unwrap();

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
    let mut testkit = TestKitBuilder::validator()
        .with(Spec::new(ExplorerFactory).with_default_instance())
        .with(Spec::new(CryptocurrencyService).with_instance(SERVICE_ID, SERVICE_NAME, ()))
        .build();

    let api = CryptocurrencyApi {
        validator_keys: vec![testkit.us().public_keys().consensus_key],
        inner: testkit.api(),
    };
    (testkit, api)
}
