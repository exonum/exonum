//! Tests for sample currency service almost identical to one implemented in
//! the [`cryptocurrency`] tutorial.
//!
//! [`cryptocurrency`]: https://github.com/exonum/cryptocurrency/

#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate exonum;
extern crate exonum_harness;

use exonum::crypto::{self, PublicKey, SecretKey};
use exonum::messages::Message;
use exonum_harness::{TestHarness, HarnessApi, ApiKind, ComparableSnapshot};

mod cryptocurrency {
    extern crate serde;
    extern crate serde_json;
    extern crate router;
    extern crate bodyparser;
    extern crate iron;

    use exonum::blockchain::{self, Blockchain, Service, Transaction, ApiContext};
    use exonum::node::{TransactionSend, ApiSender};
    use exonum::messages::{RawTransaction, FromRaw, Message};
    use exonum::storage::{Snapshot, Fork, MemoryDB, MapIndex};
    use exonum::crypto::{PublicKey, Hash, HexValue};
    use exonum::encoding;
    use exonum::api::{Api, ApiError};
    use self::iron::prelude::*;
    use self::iron::Handler;
    use self::router::Router;

    // // // // // // // // // // CONSTANTS // // // // // // // // // //

    const SERVICE_ID: u16 = 1;
    const TX_CREATE_WALLET_ID: u16 = 1;
    const TX_TRANSFER_ID: u16 = 2;

    /// Initial balance of newly created wallet.
    pub const INIT_BALANCE: u64 = 100;

    // // // // // // // // // // PERSISTENT DATA // // // // // // // // // //

    encoding_struct! {
        struct Wallet {
            const SIZE = 48;

            field pub_key:            &PublicKey  [00 => 32]
            field name:               &str        [32 => 40]
            field balance:            u64         [40 => 48]
        }
    }

    impl Wallet {
        pub fn increase(self, amount: u64) -> Self {
            let balance = self.balance() + amount;
            Self::new(self.pub_key(), self.name(), balance)
        }

        pub fn decrease(self, amount: u64) -> Self {
            let balance = self.balance() - amount;
            Self::new(self.pub_key(), self.name(), balance)
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
            let prefix = blockchain::gen_prefix(SERVICE_ID, 0, &());
            MapIndex::new(prefix, self.view.as_ref())
        }

        /// Get a separate wallet from the storage.
        pub fn wallet(&self, pub_key: &PublicKey) -> Option<Wallet> {
            self.wallets().get(pub_key)
        }
    }

    impl<'a> CurrencySchema<&'a mut Fork> {
        pub fn wallets_mut(&mut self) -> MapIndex<&mut Fork, PublicKey, Wallet> {
            let prefix = blockchain::gen_prefix(SERVICE_ID, 0, &());
            MapIndex::new(prefix, self.view)
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
            let mut schema = CurrencySchema { view };
            if schema.wallet(self.pub_key()).is_none() {
                let wallet = Wallet::new(self.pub_key(), self.name(), INIT_BALANCE);
                schema.wallets_mut().put(self.pub_key(), wallet)
            }
        }

        /// Provide information about the transaction to be used in the blockchain explorer.
        fn info(&self) -> serde_json::Value {
            serde_json::to_value(&self).expect("Cannot serialize transaction to JSON")
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
            let mut schema = CurrencySchema { view };
            let sender = schema.wallet(self.from());
            let receiver = schema.wallet(self.to());
            if let (Some(sender), Some(receiver)) = (sender, receiver) {
                let amount = self.amount();
                if sender.balance() >= amount {
                    let sender = sender.decrease(amount);
                    let receiver = receiver.increase(amount);
                    let mut wallets = schema.wallets_mut();
                    wallets.put(self.from(), sender);
                    wallets.put(self.to(), receiver);
                }
            }
        }

        /// Provide information about the transaction to be used in the blockchain explorer.
        fn info(&self) -> serde_json::Value {
            serde_json::to_value(&self).expect("Cannot serialize transaction to JSON")
        }
    }

    // // // // // // // // // // REST API // // // // // // // // // //

    #[derive(Clone)]
    struct CryptocurrencyApi {
        channel: ApiSender,
        blockchain: Blockchain,
    }

    /// Shortcut to get data on wallets.
    impl CryptocurrencyApi {
        fn get_wallet(&self, pub_key: &PublicKey) -> Option<Wallet> {
            let view = self.blockchain.snapshot();
            let schema = CurrencySchema::new(view);
            schema.wallet(pub_key)
        }

        fn get_wallets(&self) -> Option<Vec<Wallet>> {
            let view = self.blockchain.snapshot();
            let schema = CurrencySchema::new(view);
            let idx = schema.wallets();
            let wallets: Vec<Wallet> = idx.values().collect();
            if wallets.is_empty() {
                None
            } else {
                Some(wallets)
            }
        }
    }

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

    /// The structure returned by the REST API.
    #[derive(Serialize, Deserialize)]
    pub struct TransactionResponse {
        pub tx_hash: Hash,
    }

    impl Api for CryptocurrencyApi {
        fn wire(&self, router: &mut Router) {
            let self_ = self.clone();
            let transaction = move |req: &mut Request| -> IronResult<Response> {
                match req.get::<bodyparser::Struct<TransactionRequest>>() {
                    Ok(Some(transaction)) => {
                        let transaction: Box<Transaction> = transaction.into();
                        let tx_hash = transaction.hash();
                        self_.channel.send(transaction).map_err(ApiError::from)?;
                        let json = TransactionResponse { tx_hash };
                        self_.ok_response(&serde_json::to_value(&json).unwrap())
                    }
                    Ok(None) => Err(ApiError::IncorrectRequest("Empty request body".into()))?,
                    Err(e) => Err(ApiError::IncorrectRequest(Box::new(e)))?,
                }
            };

            // Gets status of all wallets.
            let self_ = self.clone();
            let wallets_info = move |_: &mut Request| -> IronResult<Response> {
                if let Some(wallets) = self_.get_wallets() {
                    self_.ok_response(&serde_json::to_value(wallets).unwrap())
                } else {
                    self_.not_found_response(
                        &serde_json::to_value("Wallets database is empty").unwrap(),
                    )
                }
            };

            // Gets status of the wallet corresponding to the public key.
            let self_ = self.clone();
            let wallet_info = move |req: &mut Request| -> IronResult<Response> {
                let path = req.url.path();
                let wallet_key = path.last().unwrap();
                let public_key = PublicKey::from_hex(wallet_key).map_err(ApiError::FromHex)?;
                if let Some(wallet) = self_.get_wallet(&public_key) {
                    self_.ok_response(&serde_json::to_value(wallet).unwrap())
                } else {
                    self_.not_found_response(&serde_json::to_value("Wallet not found").unwrap())
                }
            };

            // Bind the transaction handler to a specific route.
            router.post("/v1/wallets/transaction", transaction, "transaction");
            router.get("/v1/wallets", wallets_info, "wallets_info");
            router.get("/v1/wallet/:pub_key", wallet_info, "wallet_info");
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

    pub fn blockchain() -> Blockchain {
        let db = MemoryDB::new();
        let services: Vec<Box<Service>> = vec![Box::new(CurrencyService)];
        Blockchain::new(Box::new(db), services)
    }
}

use cryptocurrency::{CurrencySchema, TxCreateWallet, TxTransfer, TransactionResponse, Wallet,
                     blockchain};

fn create_wallet(api: &HarnessApi, name: &str) -> (TxCreateWallet, SecretKey) {
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

fn transfer(api: &HarnessApi, tx: &TxTransfer) {
    let tx_info: TransactionResponse = api.post(
        ApiKind::Service("cryptocurrency"),
        "v1/wallets/transaction",
        tx,
    );
    assert_eq!(tx_info.tx_hash, tx.hash());
}

fn get_wallet(api: &HarnessApi, pubkey: &PublicKey) -> Wallet {
    api.get(
        ApiKind::Service("cryptocurrency"),
        &format!("v1/wallet/{}", pubkey.to_string()),
    )
}

#[test]
fn test_create_wallet() {
    let mut harness = TestHarness::new(blockchain());
    let api = harness.api();
    let (tx, _) = create_wallet(&api, "Alice");

    harness.create_block();

    // Check that the user indeed is persisted by the service
    let wallet = get_wallet(&api, tx.pub_key());
    assert_eq!(wallet.pub_key(), tx.pub_key());
    assert_eq!(wallet.name(), tx.name());
    assert_eq!(wallet.balance(), 100);
}

#[test]
fn test_transfer() {
    let mut harness = TestHarness::new(blockchain());
    let api = harness.api();

    let (tx_alice, key_alice) = create_wallet(&api, "Alice");
    let (tx_bob, _) = create_wallet(&api, "Bob");
    // Commit creation transactions
    harness.create_block();

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
    harness.create_block();

    let wallet = get_wallet(&api, tx_alice.pub_key());
    assert_eq!(wallet.balance(), 90);
    let wallet = get_wallet(&api, tx_bob.pub_key());
    assert_eq!(wallet.balance(), 110);
}

#[test]
fn test_snapshot_completeness() {
    let mut harness = TestHarness::new(blockchain());
    let api = harness.api();

    let (tx_alice, _) = create_wallet(&api, "Alice");
    harness.create_block();

    let (tx_bob, _) = create_wallet(&api, "Bob");
    // Check that Alice's wallet is in the snapshot
    harness
        .probe(tx_bob)
        .compare(harness.snapshot())
        .map(CurrencySchema::new)
        .map(|schema| schema.wallet(tx_alice.pub_key()))
        .assert_inv("Alice's wallet is there", Option::is_some)
        .map(|w| w.as_ref().unwrap().balance())
        .assert_eq("Alice's balance hasn't changed");
}

#[test]
fn test_transfer_from_nonexisting_wallet() {
    let mut harness = TestHarness::new(blockchain());
    let api = harness.api();

    let (tx_alice, key_alice) = create_wallet(&api, "Alice");
    let (tx_bob, _) = create_wallet(&api, "Bob");
    // Do not commit Alice's transaction
    harness.create_block_with_transactions(&[tx_bob.hash()]);

    let wallet = get_wallet(&api, tx_bob.pub_key());
    assert_eq!(wallet.balance(), 100);

    let tx = TxTransfer::new(
        tx_alice.pub_key(),
        tx_bob.pub_key(),
        10, // amount
        0, // seed
        &key_alice,
    );

    let comp = harness.probe(tx).compare(harness.snapshot());
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
    let mut harness = TestHarness::new(blockchain());
    let api = harness.api();

    let (tx_alice, key_alice) = create_wallet(&api, "Alice");
    let (tx_bob, _) = create_wallet(&api, "Bob");
    // Do not commit Bob's transaction
    harness.create_block_with_transactions(&[tx_alice.hash()]);

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

    let old_snapshot = harness.snapshot();
    harness.create_block_with_transactions(&[tx.hash()]);

    let comp = harness.snapshot().compare(old_snapshot);
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
    let mut harness = TestHarness::new(blockchain());
    let api = harness.api();

    let (tx_alice, key_alice) = create_wallet(&api, "Alice");
    let (tx_bob, _) = create_wallet(&api, "Bob");
    harness.create_block();

    // Transfer funds
    let tx = TxTransfer::new(
        tx_alice.pub_key(),
        tx_bob.pub_key(),
        110, // amount
        0, // seed
        &key_alice,
    );
    transfer(&api, &tx);
    harness.create_block();

    let wallet = get_wallet(&api, tx_alice.pub_key());
    assert_eq!(wallet.balance(), 100);
    let wallet = get_wallet(&api, tx_bob.pub_key());
    assert_eq!(wallet.balance(), 100);
}

#[test]
fn test_transfers_in_single_block() {
    let mut harness = TestHarness::new(blockchain());
    let api = harness.api();

    let (tx_alice, key_alice) = create_wallet(&api, "Alice");
    let (tx_bob, key_bob) = create_wallet(&api, "Bob");
    harness.create_block();

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
        let comp = harness
            .probe_all(vec![
                Box::new(tx_b_to_a.clone()),
                Box::new(tx_a_to_b.clone()),
            ])
            .compare(harness.snapshot());
        let comp = comp.map(CurrencySchema::new);
        comp.map(|s| s.wallet(tx_alice.pub_key()).unwrap().balance())
            .assert("Alice's balance decreases", |&old, &new| new == old - 90);
        comp.map(|s| s.wallet(tx_bob.pub_key()).unwrap().balance())
            .assert("Bob's balance increases", |&old, &new| new == old + 90);
    }

    transfer(&api, &tx_a_to_b);
    transfer(&api, &tx_b_to_a);
    harness.create_block_with_transactions(&[tx_a_to_b.hash(), tx_b_to_a.hash()]);

    let wallet = get_wallet(&api, tx_alice.pub_key());
    assert_eq!(wallet.balance(), 130);
    let wallet = get_wallet(&api, tx_bob.pub_key());
    assert_eq!(wallet.balance(), 70);
}
