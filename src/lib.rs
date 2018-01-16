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

// Import crates with necessary types into a new project.

extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate exonum;
extern crate router;
extern crate bodyparser;
extern crate iron;

// Import necessary types from crates.

use exonum::blockchain::{Blockchain, Service, Transaction, ApiContext};
use exonum::encoding::serialize::FromHex;
use exonum::node::{TransactionSend, ApiSender};
use exonum::messages::{RawTransaction, Message};
use exonum::storage::{Fork, MapIndex, Snapshot};
use exonum::crypto::{PublicKey, Hash};
use exonum::encoding;
use exonum::api::{Api, ApiError};
use iron::prelude::*;
use iron::Handler;
use router::Router;
use serde::Deserialize;

// // // // // // // // // // CONSTANTS // // // // // // // // // //

// Define service ID for the service trait.

const SERVICE_ID: u16 = 1;

// Define constants for transaction types within the service.

const TX_CREATE_WALLET_ID: u16 = 1;

const TX_TRANSFER_ID: u16 = 2;

// Define initial balance of a newly created wallet.

const INIT_BALANCE: u64 = 100;

// // // // // // // // // // PERSISTENT DATA // // // // // // // // // //

// Declare the data to be stored in the blockchain. In the present case,
// declare a type for storing information about the wallet and its balance.

/// Declare a [serializable][1] struct and determine bounds of its fields
/// with `encoding_struct!` macro.
///
/// [1]: https://exonum.com/doc/architecture/serialization
encoding_struct! {
    struct Wallet {
        const SIZE = 48;

        field pub_key:            &PublicKey  [00 => 32]
        field name:               &str        [32 => 40]
        field balance:            u64         [40 => 48]
    }
}

/// Add methods to the `Wallet` type for changing balance.
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

/// Schema of the key-value storage used by the demo cryptocurrency service.
pub struct CurrencySchema<T> {
    view: T,
}

/// Declare layout of the data. Use an instance of [`MapIndex`]
/// to keep wallets in storage. Index values are serialized `Wallet` structs.
///
/// [`MapIndex`]: https://exonum.com/doc/architecture/storage#mapindex
impl<T: AsRef<Snapshot>> CurrencySchema<T> {
    /// Creates the schema instance.
    pub fn new(view: T) -> Self {
        CurrencySchema { view }
    }

    /// Returns an immutable version of the wallets table.
    pub fn wallets(&self) -> MapIndex<&Snapshot, PublicKey, Wallet> {
        MapIndex::new("cryptocurrency.wallets", self.view.as_ref())
    }

    /// Gets a separate wallet from the storage.
    pub fn wallet(&self, pub_key: &PublicKey) -> Option<Wallet> {
        self.wallets().get(pub_key)
    }
}

/// A mutable version of the schema declaring an additional method to persist wallets
/// to the storage.
impl<'a> CurrencySchema<&'a mut Fork> {
    /// Returns a mutable version of the wallets table.
    pub fn wallets_mut(&mut self) -> MapIndex<&mut Fork, PublicKey, Wallet> {
        MapIndex::new("cryptocurrency.wallets", &mut self.view)
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

/// Execute a transaction.
impl Transaction for TxCreateWallet {
    /// Verify integrity of the transaction by checking the transaction
    /// signature.
    fn verify(&self) -> bool {
        self.verify_signature(self.pub_key())
    }

    /// Apply logic to the storage when executing the transaction.
    fn execute(&self, view: &mut Fork) {
        let mut schema = CurrencySchema::new(view);
        if schema.wallet(self.pub_key()).is_none() {
            let wallet = Wallet::new(self.pub_key(), self.name(), INIT_BALANCE);
            println!("Create the wallet: {:?}", wallet);
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

    /// Provide information about the transaction to be used in the blockchain explorer.
    fn info(&self) -> serde_json::Value {
        serde_json::to_value(&self).expect("Cannot serialize transaction to JSON")
    }
}

// // // // // // // // // // REST API // // // // // // // // // //

/// Implement the node API.
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
                let json = TransactionResponse { tx_hash };
                self.ok_response(&serde_json::to_value(&json).unwrap())
            }
            Ok(None) => Err(ApiError::IncorrectRequest("Empty request body".into()))?,
            Err(e) => Err(ApiError::IncorrectRequest(Box::new(e)))?,
        }
    }
}


/// Implement the `Api` trait.
/// `Api` facilitates conversion between transactions/read requests and REST
/// endpoints; for example, it parses `POSTed` JSON into the binary transaction
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
