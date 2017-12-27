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

//! Tests for sample currency service with inflation. Similar to the `test_currency`
//! integration test, with the difference that the balance of each created wallet increases by 1
//! on each block. Correspondingly, the initial wallet balance is set to 0.

#[macro_use]
extern crate exonum;
#[macro_use]
extern crate exonum_testkit;
extern crate rand;
#[macro_use]
extern crate serde_derive;

use exonum::blockchain::Transaction;
use exonum::crypto::{self, PublicKey, SecretKey};
use exonum::helpers::Height;
use exonum::messages::Message;
use exonum_testkit::{ApiKind, TestKit, TestKitApi, TestKitBuilder};
use rand::Rng;

mod inflating_cryptocurrency {
    extern crate bodyparser;
    extern crate iron;
    extern crate router;
    extern crate serde;
    extern crate serde_json;

    use exonum::blockchain::{ApiContext, Blockchain, Schema as CoreSchema, Service, Transaction};
    use exonum::node::{ApiSender, TransactionSend};
    use exonum::messages::{Message, RawTransaction};
    use exonum::storage::{Fork, MapIndex, Snapshot};
    use exonum::crypto::{Hash, PublicKey};
    use exonum::encoding;
    use exonum::encoding::serialize::FromHex;
    use exonum::api::{Api, ApiError};
    use exonum::helpers::Height;
    use self::iron::prelude::*;
    use self::iron::headers::ContentType;
    use self::iron::{Handler, IronError};
    use self::iron::status::Status;
    use self::router::Router;

    // // // // // // // // // // CONSTANTS // // // // // // // // // //

    const SERVICE_ID: u16 = 1;
    const TX_CREATE_WALLET_ID: u16 = 1;
    const TX_TRANSFER_ID: u16 = 2;

    /// Initial balance of newly created wallet.
    pub const INIT_BALANCE: u64 = 0;

    // // // // // // // // // // PERSISTENT DATA // // // // // // // // // //

    encoding_struct! {
        struct Wallet {
            const SIZE = 56;

            field pub_key:            &PublicKey  [00 => 32]
            field name:               &str        [32 => 40]
            field balance:            u64         [40 => 48]
            field last_update_height: u64         [48 => 56]
        }
    }

    impl Wallet {
        pub fn actual_balance(&self, height: Height) -> u64 {
            assert!(height.0 >= self.last_update_height());
            self.balance() + height.0 - self.last_update_height()
        }

        pub fn increase(self, amount: u64, height: Height) -> Self {
            let balance = self.actual_balance(height) + amount;
            Self::new(self.pub_key(), self.name(), balance, height.0)
        }

        pub fn decrease(self, amount: u64, height: Height) -> Self {
            let balance = self.actual_balance(height) - amount;
            Self::new(self.pub_key(), self.name(), balance, height.0)
        }
    }

    // // // // // // // // // // DATA LAYOUT // // // // // // // // // //

    pub struct CurrencySchema<S> {
        view: S,
    }

    impl<S: AsRef<Snapshot>> CurrencySchema<S> {
        pub fn new(view: S) -> Self {
            CurrencySchema { view }
        }

        pub fn wallets(&self) -> MapIndex<&Snapshot, PublicKey, Wallet> {
            MapIndex::new("cryptocurrency.wallets", self.view.as_ref())
        }

        /// Get a separate wallet from the storage.
        pub fn wallet(&self, pub_key: &PublicKey) -> Option<Wallet> {
            self.wallets().get(pub_key)
        }
    }

    impl<'a> CurrencySchema<&'a mut Fork> {
        pub fn wallets_mut(&mut self) -> MapIndex<&mut Fork, PublicKey, Wallet> {
            MapIndex::new("cryptocurrency.wallets", self.view)
        }
    }

    // // // // // // // // // // TRANSACTIONS // // // // // // // // // //

    /// Create a new wallet.
    message! {
        struct TxCreateWallet {
            const TYPE = SERVICE_ID;
            const ID = TX_CREATE_WALLET_ID;
            const SIZE = 40;

            field pub_key:     &PublicKey  [00 => 32]
            field name:        &str        [32 => 40]
        }
    }

    /// Transfer coins between the wallets.
    message! {
        struct TxTransfer {
            const TYPE = SERVICE_ID;
            const ID = TX_TRANSFER_ID;
            const SIZE = 80;

            field from:        &PublicKey  [00 => 32]
            field to:          &PublicKey  [32 => 64]
            field amount:      u64         [64 => 72]
            field seed:        u64         [72 => 80]
        }
    }

    // // // // // // // // // // CONTRACTS // // // // // // // // // //

    impl Transaction for TxCreateWallet {
        /// Verify integrity of the transaction by checking the transaction
        /// signature.
        fn verify(&self) -> bool {
            self.verify_signature(self.pub_key())
        }

        /// Apply logic to the storage when executing the transaction.
        fn execute(&self, view: &mut Fork) {
            let height = CoreSchema::new(&view).height();
            let mut schema = CurrencySchema { view };
            if schema.wallet(self.pub_key()).is_none() {
                let wallet = Wallet::new(self.pub_key(), self.name(), INIT_BALANCE, height.0);
                schema.wallets_mut().put(self.pub_key(), wallet)
            }
        }
    }

    impl Transaction for TxTransfer {
        /// Check if the sender is not the receiver. Check correctness of the
        /// sender's signature.
        fn verify(&self) -> bool {
            (*self.from() != *self.to()) && self.verify_signature(self.from())
        }

        /// Retrieve two wallets to apply the transfer. Check the sender's
        /// balance and apply changes to the balances of the wallets.
        fn execute(&self, view: &mut Fork) {
            let height = CoreSchema::new(&view).height();
            let mut schema = CurrencySchema { view };
            let sender = schema.wallet(self.from());
            let receiver = schema.wallet(self.to());
            if let (Some(sender), Some(receiver)) = (sender, receiver) {
                let amount = self.amount();
                if sender.actual_balance(height) >= amount {
                    let sender = sender.decrease(amount, height);
                    let receiver = receiver.increase(amount, height);
                    let mut wallets = schema.wallets_mut();
                    wallets.put(self.from(), sender);
                    wallets.put(self.to(), receiver);
                }
            }
        }
    }

    // // // // // // // // // // REST API // // // // // // // // // //

    #[derive(Clone)]
    struct CryptocurrencyApi {
        channel: ApiSender,
        blockchain: Blockchain,
    }

    /// The structure returned by the REST API.
    #[derive(Serialize, Deserialize)]
    pub struct TransactionResponse {
        pub tx_hash: Hash,
    }

    /// Shortcut to get data on wallets.
    impl CryptocurrencyApi {
        fn wallet(&self, pub_key: &PublicKey) -> Option<Wallet> {
            let view = self.blockchain.snapshot();
            let schema = CurrencySchema::new(view);
            schema.wallet(pub_key)
        }

        /// Endpoint for transactions.
        fn post_transaction(&self, req: &mut Request) -> IronResult<Response> {
            /// Add an enum which joins transactions of both types to simplify request
            /// processing.
            #[serde(untagged)]
            #[derive(Clone, Serialize, Deserialize)]
            enum TransactionRequest {
                CreateWallet(TxCreateWallet),
                Transfer(TxTransfer),
            }

            /// Implement a trait for the enum for deserialized `TransactionRequest`s
            /// to fit into the node channel.
            impl Into<Box<Transaction>> for TransactionRequest {
                fn into(self) -> Box<Transaction> {
                    match self {
                        TransactionRequest::CreateWallet(trans) => Box::new(trans),
                        TransactionRequest::Transfer(trans) => Box::new(trans),
                    }
                }
            }

            match req.get::<bodyparser::Struct<TransactionRequest>>() {
                Ok(Some(transaction)) => {
                    let transaction: Box<Transaction> = transaction.into();
                    let tx_hash = transaction.hash();
                    self.channel.send(transaction).map_err(ApiError::from)?;
                    let json = TransactionResponse { tx_hash };
                    self.ok_response(&serde_json::to_value(&json).unwrap())
                }
                Ok(None) => Err(ApiError::IncorrectRequest("Empty request body".into()))?,
                Err(e) => Err(ApiError::IncorrectRequest(Box::new(e)))?,
            }
        }

        /// Endpoint for retrieving a single wallet.
        fn balance(&self, req: &mut Request) -> IronResult<Response> {
            use self::iron::modifiers::Header;

            let path = req.url.path();
            let wallet_key = path.last().unwrap();
            let public_key = PublicKey::from_hex(wallet_key).map_err(|e| {
                IronError::new(ApiError::FromHex(e), (
                    Status::BadRequest,
                    Header(ContentType::json()),
                    "\"Invalid request param: `pub_key`\"",
                ))
            })?;
            if let Some(wallet) = self.wallet(&public_key) {
                let height = CoreSchema::new(self.blockchain.snapshot()).height();
                self.ok_response(&serde_json::to_value(wallet.actual_balance(height))
                    .unwrap())
            } else {
                Err(IronError::new(ApiError::NotFound, (
                    Status::NotFound,
                    Header(ContentType::json()),
                    "\"Wallet not found\"",
                )))
            }
        }
    }

    impl Api for CryptocurrencyApi {
        fn wire(&self, router: &mut Router) {
            let self_ = self.clone();
            let post_transaction = move |req: &mut Request| self_.post_transaction(req);
            let self_ = self.clone();
            let balance = move |req: &mut Request| self_.balance(req);

            // Bind the transaction handler to a specific route.
            router.post(
                "/v1/wallets/transaction",
                post_transaction,
                "post_transaction",
            );
            router.get("/v1/balance/:pub_key", balance, "balance");
        }
    }

    // // // // // // // // // // SERVICE DECLARATION // // // // // // // // // //

    /// Define the service.
    pub struct CurrencyService;

    /// Implement a `Service` trait for the service.
    impl Service for CurrencyService {
        fn service_name(&self) -> &'static str {
            "cryptocurrency"
        }

        fn state_hash(&self, _: &Snapshot) -> Vec<Hash> {
            Vec::new()
        }

        fn service_id(&self) -> u16 {
            SERVICE_ID
        }

        /// Implement a method to deserialize transactions coming to the node.
        fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, encoding::Error> {
            let trans: Box<Transaction> = match raw.message_type() {
                TX_TRANSFER_ID => Box::new(TxTransfer::from_raw(raw)?),
                TX_CREATE_WALLET_ID => Box::new(TxCreateWallet::from_raw(raw)?),
                _ => {
                    return Err(encoding::Error::IncorrectMessageType {
                        message_type: raw.message_type(),
                    });
                }
            };
            Ok(trans)
        }

        /// Create a REST `Handler` to process web requests to the node.
        fn public_api_handler(&self, ctx: &ApiContext) -> Option<Box<Handler>> {
            let mut router = Router::new();
            let api = CryptocurrencyApi {
                channel: ctx.node_channel().clone(),
                blockchain: ctx.blockchain().clone(),
            };
            api.wire(&mut router);
            Some(Box::new(router))
        }
    }
}

use inflating_cryptocurrency::{CurrencyService, TransactionResponse, TxCreateWallet, TxTransfer};

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
    testkit.create_block_with_transactions(txvec![tx_a_to_b.clone()]); // A: 4 + 1, B: 14 + 1
    testkit.create_block_with_transactions(txvec![]); // A: 4 + 2, B: 14 + 2
    testkit.create_block_with_transactions(txvec![next_tx_a_to_b.clone()]); // A: 0 + 1, B: 20 + 3
    assert_eq!(get_balance(&api, tx_alice.pub_key()), 1); // 0 + 1
    assert_eq!(get_balance(&api, tx_bob.pub_key()), 23); // 20 + 3
    testkit.rollback(3);

    // If there is no block separating transactions, Alice's balance is insufficent
    // to complete the second transaction.
    testkit.create_block_with_transactions(txvec![tx_a_to_b.clone()]); // A: 4 + 1, B: 14 + 1
    testkit.create_block_with_transactions(txvec![next_tx_a_to_b.clone()]); // fails
    assert_eq!(get_balance(&api, tx_alice.pub_key()), 6); // 4 + 2
    assert_eq!(get_balance(&api, tx_bob.pub_key()), 16); // 14 + 2
    testkit.rollback(2);

    testkit.create_block_with_transactions(txvec![next_tx_a_to_b.clone()]); // A: 3 + 1, B: 15 + 1
    testkit.create_block_with_transactions(txvec![tx_a_to_b.clone()]); // fails
    assert_eq!(get_balance(&api, tx_alice.pub_key()), 5); // 3 + 2
    assert_eq!(get_balance(&api, tx_bob.pub_key()), 17); // 15 + 2
    testkit.rollback(2);

    // If the transactions are put in the same block, only the first transaction should succeed
    testkit.create_block_with_transactions(txvec![tx_a_to_b.clone(), next_tx_a_to_b.clone()]);
    assert_eq!(get_balance(&api, tx_alice.pub_key()), 5); // 4 + 1
    assert_eq!(get_balance(&api, tx_bob.pub_key()), 15); // 14 + 1
    testkit.rollback(1);

    // Same here
    testkit.create_block_with_transactions(txvec![next_tx_a_to_b.clone(), tx_a_to_b.clone()]);
    assert_eq!(get_balance(&api, tx_alice.pub_key()), 4); // 3 + 1
    assert_eq!(get_balance(&api, tx_bob.pub_key()), 16); // 15 + 1
    testkit.rollback(1);
}

fn fuzz_transfers_and_maybe_rollbacks(use_rollbacks: bool) {
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

    testkit.create_block_with_transactions(keys_and_txs.iter().map(|&(_, ref tx)| {
        Box::new(tx.clone()) as Box<Transaction>
    }));

    for _ in 0..64 {
        let total_balance: u64 = pubkeys.iter().map(|key| get_balance(&api, key)).sum();
        assert_eq!(total_balance, (USERS as u64) * testkit.height().0);

        if use_rollbacks {
            let rollback_blocks = rng.choose(&[0usize, 0, 0, 1, 2, 3]);
            match rollback_blocks {
                Some(&blocks) if testkit.height() > Height(blocks as u64) => {
                    testkit.rollback(blocks)
                }
                _ => {}
            }
        }

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

/// Test randomly generated transfers among users without blockchain rollbacks.
#[test]
fn test_fuzz_transfers() {
    fuzz_transfers_and_maybe_rollbacks(false);
}

/// Test randomly generated transfers among users with blockchain rollbacks.
/// This mostly tests `TestKit::rollback()` method rather than the service,
/// because in practice rollbacks are impossible.
#[test]
fn test_fuzz_transfers_and_rollbacks() {
    fuzz_transfers_and_maybe_rollbacks(true);
}
