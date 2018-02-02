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

//! Demo [Exonum][exonum] service implementing a simple cryptocurrency.
//! See [the documentation][docs] for a detailed step-by-step guide how to approach this demo,
//! and [the reporitory README][readme] on how to use, test, and contribute to it.
//!
//! **Note.** The service in this crate is intended for demo purposes only. It is not intended
//! for use in production.
//!
//! [exonum]: https://github.com/exonum/exonum
//! [docs]: https://exonum.com/doc/get-started/create-service
//! [readme]: https://github.com/exonum/cryptocurrency#readme

// Import necessary types from crates.

use bodyparser;
use exonum::blockchain::{Blockchain, Service, Transaction, ApiContext};
use exonum::encoding::serialize::FromHex;
use exonum::node::{TransactionSend, ApiSender};
use exonum::messages::{RawTransaction, Message};
use exonum::storage::{Fork, MapIndex, Snapshot};
use exonum::crypto::{Hash, PublicKey};
use exonum::encoding;
use exonum::api::{Api, ApiError};
use iron::prelude::*;
use iron::Handler;
use router::Router;
use serde::Deserialize;
use serde_json;

// // // // // // // // // // CONSTANTS // // // // // // // // // //

/// Service ID for the `Service` trait.
const SERVICE_ID: u16 = 1;

// Constants for transaction types within the service.
const TX_CREATE_WALLET_ID: u16 = 1;
const TX_TRANSFER_ID: u16 = 2;

/// Initial balance of a newly created wallet.
const INIT_BALANCE: u64 = 100;

// // // // // // // // // // PERSISTENT DATA // // // // // // // // // //

// Declare the data to be stored in the blockchain, namely wallets with balances.
// See [serialization docs][1] for details.
//
// [1]: https://exonum.com/doc/architecture/serialization

encoding_struct! {
    /// Wallet struct used to persist data within the service.
    struct Wallet {
        /// Public key of the wallet owner.
        pub_key: &PublicKey,
        /// Name of the wallet owner.
        name: &str,
        /// Current balance.
        balance: u64,
    }
}

/// Additional methods for managing balance of the wallet in an immutable fashion.
impl Wallet {
    /// Returns a copy of this wallet with the balance increased by the specified amount.
    pub fn increase(self, amount: u64) -> Self {
        let balance = self.balance() + amount;
        Self::new(self.pub_key(), self.name(), balance)
    }

    /// Returns a copy of this wallet with the balance decreased by the specified amount.
    pub fn decrease(self, amount: u64) -> Self {
        debug_assert!(self.balance() >= amount);
        let balance = self.balance() - amount;
        Self::new(self.pub_key(), self.name(), balance)
    }
}

// // // // // // // // // // DATA LAYOUT // // // // // // // // // //

/// Schema of the key-value storage used by the demo cryptocurrency service.
pub struct CurrencySchema<T> {
    view: T,
}

/// Declare the layout of data managed by the service. An instance of [`MapIndex`] is used
/// to keep wallets in the storage. Index values are serialized [`Wallet`] structs.
///
/// [`MapIndex`]: https://exonum.com/doc/architecture/storage#mapindex
/// [`Wallet`]: struct.Wallet.html
impl<T: AsRef<Snapshot>> CurrencySchema<T> {
    /// Creates a new schema instance.
    pub fn new(view: T) -> Self {
        CurrencySchema { view }
    }

    /// Returns an immutable version of the wallets table.
    pub fn wallets(&self) -> MapIndex<&Snapshot, PublicKey, Wallet> {
        MapIndex::new("cryptocurrency.wallets", self.view.as_ref())
    }

    /// Gets a specific wallet from the storage.
    pub fn wallet(&self, pub_key: &PublicKey) -> Option<Wallet> {
        self.wallets().get(pub_key)
    }
}

/// A mutable version of the schema with an additional method to persist wallets
/// to the storage.
impl<'a> CurrencySchema<&'a mut Fork> {
    /// Returns a mutable version of the wallets table.
    pub fn wallets_mut(&mut self) -> MapIndex<&mut Fork, PublicKey, Wallet> {
        MapIndex::new("cryptocurrency.wallets", &mut self.view)
    }
}

// // // // // // // // // // TRANSACTIONS // // // // // // // // // //

message! {
    /// Transaction type for creating a new wallet.
    ///
    /// See [the `Transaction` trait implementation](#impl-Transaction) for details how
    /// `TxCreateWallet` transactions are processed.
    struct TxCreateWallet {
        const TYPE = SERVICE_ID;
        const ID = TX_CREATE_WALLET_ID;

        /// Public key of the wallet's owner.
        pub_key: &PublicKey,
        /// UTF-8 string with the owner's name.
        name: &str,
    }
}

message! {
    /// Transaction type for transferring tokens between two wallets.
    ///
    /// See [the `Transaction` trait implementation](#impl-Transaction) for details how
    /// `TxTransfer` transactions are processed.
    struct TxTransfer {
        const TYPE = SERVICE_ID;
        const ID = TX_TRANSFER_ID;

        /// Public key of the sender.
        from: &PublicKey,
        /// Public key of the receiver.
        to: &PublicKey,
        /// Number of tokens to transfer from sender's account to receiver's account.
        amount: u64,
        /// Auxiliary number to guarantee [non-idempotence][idempotence] of transactions.
        ///
        /// [idempotence]: https://en.wikipedia.org/wiki/Idempotence
        seed: u64,
    }
}

// // // // // // // // // // CONTRACTS // // // // // // // // // //

impl Transaction for TxCreateWallet {
    /// Verifies integrity of the transaction by checking the transaction
    /// signature.
    fn verify(&self) -> bool {
        self.verify_signature(self.pub_key())
    }

    /// If a wallet with the specified public key is not registered, then creates a new wallet
    /// with the specified public key and name, and an initial balance of 100.
    /// Otherwise, performs no op.
    fn execute(&self, view: &mut Fork) {
        let mut schema = CurrencySchema::new(view);
        if schema.wallet(self.pub_key()).is_none() {
            let wallet = Wallet::new(self.pub_key(), self.name(), INIT_BALANCE);
            println!("Create the wallet: {:?}", wallet);
            schema.wallets_mut().put(self.pub_key(), wallet);
        }
    }
}

impl Transaction for TxTransfer {
    /// Checks if the sender is not the receiver, and checks correctness of the
    /// sender's signature.
    fn verify(&self) -> bool {
        (*self.from() != *self.to()) && self.verify_signature(self.from())
    }

    /// Retrieves two wallets to apply the transfer; they should be previously registered
    /// with the help of [`TxCreateWallet`] transactions. Checks the sender's
    /// balance and applies changes to the balances of the wallets if the sender's balance
    /// is sufficient. Otherwise, performs no op.
    ///
    /// [`TxCreateWallet`]: struct.TxCreateWallet.html
    fn execute(&self, view: &mut Fork) {
        let mut schema = CurrencySchema::new(view);
        let sender = schema.wallet(self.from());
        let receiver = schema.wallet(self.to());
        if let (Some(sender), Some(receiver)) = (sender, receiver) {
            let amount = self.amount();
            if sender.balance() >= amount {
                let sender = sender.decrease(amount);
                let receiver = receiver.increase(amount);
                println!("Transfer between wallets: {:?} => {:?}", sender, receiver);
                let mut wallets = schema.wallets_mut();
                wallets.put(self.from(), sender);
                wallets.put(self.to(), receiver);
            }
        }
    }
}

// // // // // // // // // // REST API // // // // // // // // // //

/// Container for the service API.
#[derive(Clone)]
struct CryptocurrencyApi {
    channel: ApiSender,
    blockchain: Blockchain,
}

impl CryptocurrencyApi {
    /// Endpoint for getting a single wallet.
    fn get_wallet(&self, req: &mut Request) -> IronResult<Response> {
        let path = req.url.path();
        let wallet_key = path.last().unwrap();
        let public_key = PublicKey::from_hex(wallet_key).map_err(ApiError::FromHex)?;

        let wallet = {
            let snapshot = self.blockchain.snapshot();
            let schema = CurrencySchema::new(snapshot);
            schema.wallet(&public_key)
        };

        if let Some(wallet) = wallet {
            self.ok_response(&serde_json::to_value(wallet).unwrap())
        } else {
            self.not_found_response(&serde_json::to_value("Wallet not found").unwrap())
        }
    }

    /// Endpoint for dumping all wallets from the storage.
    fn get_wallets(&self, _: &mut Request) -> IronResult<Response> {
        let snapshot = self.blockchain.snapshot();
        let schema = CurrencySchema::new(snapshot);
        let idx = schema.wallets();
        let wallets: Vec<Wallet> = idx.values().collect();

        self.ok_response(&serde_json::to_value(&wallets).unwrap())
    }

    /// Common processing for transaction-accepting endpoints.
    fn post_transaction<T>(&self, req: &mut Request) -> IronResult<Response>
    where
        T: Transaction + Clone + for<'de> Deserialize<'de>,
    {
        match req.get::<bodyparser::Struct<T>>() {
            Ok(Some(transaction)) => {
                let transaction: Box<Transaction> = Box::new(transaction);
                let tx_hash = transaction.hash();
                self.channel.send(transaction).map_err(ApiError::from)?;
                self.ok_response(&json!({
                    "tx_hash": tx_hash
                }))
            }
            Ok(None) => Err(ApiError::IncorrectRequest("Empty request body".into()))?,
            Err(e) => Err(ApiError::IncorrectRequest(Box::new(e)))?,
        }
    }
}


/// `Api` trait implementation.
///
/// `Api` facilitates conversion between transactions/read requests and REST
/// endpoints; for example, it parses `POST`ed JSON into the binary transaction
/// representation used in Exonum internally.
impl Api for CryptocurrencyApi {
    fn wire(&self, router: &mut Router) {
        let self_ = self.clone();
        let post_create_wallet =
            move |req: &mut Request| self_.post_transaction::<TxCreateWallet>(req);
        let self_ = self.clone();
        let post_transfer = move |req: &mut Request| self_.post_transaction::<TxTransfer>(req);
        let self_ = self.clone();
        let get_wallets = move |req: &mut Request| self_.get_wallets(req);
        let self_ = self.clone();
        let get_wallet = move |req: &mut Request| self_.get_wallet(req);

        // Bind handlers to specific routes.
        router.post("/v1/wallets", post_create_wallet, "post_create_wallet");
        router.post("/v1/wallets/transfer", post_transfer, "post_transfer");
        router.get("/v1/wallets", get_wallets, "get_wallets");
        router.get("/v1/wallet/:pub_key", get_wallet, "get_wallet");
    }
}

// // // // // // // // // // SERVICE DECLARATION // // // // // // // // // //

/// Demo cryptocurrency service.
///
/// See [the crate documentation](index.html) for context.
///
/// # Public REST API
///
/// In all APIs, the request body (if applicable) and response are JSON-encoded.
///
/// ## Retrieve single wallet
///
/// GET `v1/wallet/:pub_key`
///
/// Returns information about a wallet with the specified public key (hex-encoded).
/// If a wallet with the specified pubkey is not in the storage, returns a string
/// `"Wallet not found"` with the HTTP 404 status.
///
/// ## Dump wallets
///
/// GET `v1/wallets`
///
/// Returns an array of all wallets in the storage.
///
/// ## Create new wallet
///
/// POST `v1/wallets`
///
/// Accepts a [`TxCreateWallet`] transaction from an external client. Returns the hex-encoded
/// hash of the transaction encumbered in an object: `{ "tx_hash": <hash> }`.
///
/// ## Transfer between wallets
///
/// POST `v1/wallets/transfer`
///
/// Accepts a [`TxTransfer`] transaction from an external client. Returns the hex-encoded
/// hash of the transaction encumbered in an object: `{ "tx_hash": <hash> }`.
///
/// [`TxCreateWallet`]: struct.TxCreateWallet.html
/// [`TxTransfer`]: struct.TxTransfer.html
pub struct CurrencyService;

impl Service for CurrencyService {
    fn service_name(&self) -> &'static str {
        "cryptocurrency"
    }

    fn service_id(&self) -> u16 {
        SERVICE_ID
    }

    // Implement a method to deserialize transactions coming to the node.
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

    // Hashes for the service tables that will be included into the state hash.
    // To simplify things, we don't have [Merkelized tables][merkle] in the service storage
    // for now, so we return an empty vector.
    //
    // [merkle]: https://exonum.com/doc/architecture/storage/#merklized-indices
    fn state_hash(&self, _: &Snapshot) -> Vec<Hash> {
        vec![]
    }

    // Create a REST `Handler` to process web requests to the node.
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
