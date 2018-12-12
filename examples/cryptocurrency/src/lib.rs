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

//! Demo [Exonum][exonum] service implementing a simple cryptocurrency.
//! See [the documentation][docs] for a detailed step-by-step guide how to approach this demo,
//! and [the repository README][readme] on how to use, test, and contribute to it.
//!
//! **Note.** The service in this crate is intended for demo purposes only. It is not intended
//! for use in production.
//!
//! [exonum]: https://github.com/exonum/exonum
//! [docs]: https://exonum.com/doc/get-started/create-service
//! [readme]: https://github.com/exonum/cryptocurrency#readme

#![deny(
    missing_debug_implementations,
    missing_docs,
    unsafe_code,
    bare_trait_objects
)]

extern crate exonum;
#[macro_use]
extern crate exonum_derive;
#[macro_use]
extern crate failure;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate protobuf;
extern crate serde_json;

pub mod proto;

/// Persistent data.
pub mod schema {
    use exonum::{
        crypto::PublicKey,
        storage::{Fork, MapIndex, Snapshot},
    };

    use super::proto;

    // Declare the data to be stored in the blockchain, namely wallets with balances.
    // See [serialization docs][1] for details.
    //
    // [1]: https://exonum.com/doc/architecture/serialization
    /// Wallet struct used to persist data within the service.
    #[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert)]
    #[exonum(pb = "proto::Wallet")]
    pub struct Wallet {
        /// Public key of the wallet owner.
        pub pub_key: PublicKey,
        /// Name of the wallet owner.
        pub name: String,
        /// Current balance.
        pub balance: u64,
    }

    /// Additional methods for managing balance of the wallet in an immutable fashion.
    impl Wallet {
        /// Create new Wallet.
        pub fn new(&pub_key: &PublicKey, name: &str, balance: u64) -> Self {
            Self {
                pub_key,
                name: name.to_owned(),
                balance,
            }
        }

        /// Returns a copy of this wallet with the balance increased by the specified amount.
        pub fn increase(self, amount: u64) -> Self {
            let balance = self.balance + amount;
            Self::new(&self.pub_key, &self.name, balance)
        }

        /// Returns a copy of this wallet with the balance decreased by the specified amount.
        pub fn decrease(self, amount: u64) -> Self {
            debug_assert!(self.balance >= amount);
            let balance = self.balance - amount;
            Self::new(&self.pub_key, &self.name, balance)
        }
    }

    /// Schema of the key-value storage used by the demo cryptocurrency service.
    #[derive(Debug)]
    pub struct CurrencySchema<T> {
        view: T,
    }

    /// Declare the layout of data managed by the service. An instance of [`MapIndex`] is used
    /// to keep wallets in the storage. Index values are serialized [`Wallet`] structs.
    ///
    /// [`MapIndex`]: https://exonum.com/doc/architecture/storage#mapindex
    /// [`Wallet`]: struct.Wallet.html
    impl<T: AsRef<dyn Snapshot>> CurrencySchema<T> {
        /// Creates a new schema instance.
        pub fn new(view: T) -> Self {
            CurrencySchema { view }
        }

        /// Returns an immutable version of the wallets table.
        pub fn wallets(&self) -> MapIndex<&dyn Snapshot, PublicKey, Wallet> {
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
}

/// Transactions.
pub mod transactions {
    use super::proto;
    use super::service::SERVICE_ID;
    use exonum::{
        crypto::{PublicKey, SecretKey},
        messages::{Message, RawTransaction, Signed},
    };
    /// Transaction type for creating a new wallet.
    ///
    /// See [the `Transaction` trait implementation](#impl-Transaction) for details how
    /// `TxCreateWallet` transactions are processed.
    #[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert)]
    #[exonum(pb = "proto::TxCreateWallet")]
    pub struct TxCreateWallet {
        /// UTF-8 string with the owner's name.
        pub name: String,
    }

    /// Transaction type for transferring tokens between two wallets.
    ///
    /// See [the `Transaction` trait implementation](#impl-Transaction) for details how
    /// `TxTransfer` transactions are processed.
    #[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert)]
    #[exonum(pb = "proto::TxTransfer")]
    pub struct TxTransfer {
        /// Public key of the receiver.
        pub to: PublicKey,
        /// Number of tokens to transfer from sender's account to receiver's account.
        pub amount: u64,
        /// Auxiliary number to guarantee [non-idempotence][idempotence] of transactions.
        ///
        /// [idempotence]: https://en.wikipedia.org/wiki/Idempotence
        pub seed: u64,
    }

    /// Transaction group.
    #[derive(Serialize, Deserialize, Clone, Debug, TransactionSet)]
    pub enum CurrencyTransactions {
        /// Create wallet transaction.
        CreateWallet(TxCreateWallet),
        /// Transfer tokens transaction.
        Transfer(TxTransfer),
    }

    impl TxCreateWallet {
        #[doc(hidden)]
        pub fn sign(name: &str, pk: &PublicKey, sk: &SecretKey) -> Signed<RawTransaction> {
            Message::sign_transaction(
                Self {
                    name: name.to_owned(),
                },
                SERVICE_ID,
                *pk,
                sk,
            )
        }
    }

    impl TxTransfer {
        #[doc(hidden)]
        pub fn sign(
            to: &PublicKey,
            amount: u64,
            seed: u64,
            pk: &PublicKey,
            sk: &SecretKey,
        ) -> Signed<RawTransaction> {
            Message::sign_transaction(
                Self {
                    to: *to,
                    amount,
                    seed,
                },
                SERVICE_ID,
                *pk,
                sk,
            )
        }
    }
}

/// Contract errors.
pub mod errors {
    // Workaround for `failure` see https://github.com/rust-lang-nursery/failure/issues/223 and
    // ECR-1771 for the details.
    #![allow(bare_trait_objects)]

    use exonum::blockchain::ExecutionError;

    /// Error codes emitted by `TxCreateWallet` and/or `TxTransfer` transactions during execution.
    #[derive(Debug, Fail)]
    #[repr(u8)]
    pub enum Error {
        /// Wallet already exists.
        ///
        /// Can be emitted by `TxCreateWallet`.
        #[fail(display = "Wallet already exists")]
        WalletAlreadyExists = 0,

        /// Sender doesn't exist.
        ///
        /// Can be emitted by `TxTransfer`.
        #[fail(display = "Sender doesn't exist")]
        SenderNotFound = 1,

        /// Receiver doesn't exist.
        ///
        /// Can be emitted by `TxTransfer`.
        #[fail(display = "Receiver doesn't exist")]
        ReceiverNotFound = 2,

        /// Insufficient currency amount.
        ///
        /// Can be emitted by `TxTransfer`.
        #[fail(display = "Insufficient currency amount")]
        InsufficientCurrencyAmount = 3,

        /// Sender same as receiver.
        ///
        /// Can be emitted by `TxTransfer`.
        #[fail(display = "Sender same as receiver")]
        SenderSameAsReceiver = 4,
    }

    impl From<Error> for ExecutionError {
        fn from(value: Error) -> ExecutionError {
            let description = format!("{}", value);
            ExecutionError::with_description(value as u8, description)
        }
    }
}

/// Contracts.
pub mod contracts {
    use exonum::blockchain::{ExecutionResult, Transaction, TransactionContext};

    use errors::Error;
    use schema::{CurrencySchema, Wallet};
    use transactions::{TxCreateWallet, TxTransfer};

    /// Initial balance of a newly created wallet.
    const INIT_BALANCE: u64 = 100;

    impl Transaction for TxCreateWallet {
        /// If a wallet with the specified public key is not registered, then creates a new wallet
        /// with the specified public key and name, and an initial balance of 100.
        /// Otherwise, performs no op.
        fn execute(&self, mut context: TransactionContext) -> ExecutionResult {
            let author = context.author();
            let view = context.fork();
            let mut schema = CurrencySchema::new(view);
            if schema.wallet(&author).is_none() {
                let wallet = Wallet::new(&author, &self.name, INIT_BALANCE);
                println!("Create the wallet: {:?}", wallet);
                schema.wallets_mut().put(&author, wallet);
                Ok(())
            } else {
                Err(Error::WalletAlreadyExists)?
            }
        }
    }

    impl Transaction for TxTransfer {
        /// Retrieves two wallets to apply the transfer; they should be previously registered
        /// with the help of [`TxCreateWallet`] transactions. Checks the sender's
        /// balance and applies changes to the balances of the wallets if the sender's balance
        /// is sufficient. Otherwise, performs no op.
        ///
        /// [`TxCreateWallet`]: ../transactions/struct.TxCreateWallet.html
        fn execute(&self, mut context: TransactionContext) -> ExecutionResult {
            let author = context.author();
            let view = context.fork();

            if author == self.to {
                Err(Error::SenderSameAsReceiver)?
            }

            let mut schema = CurrencySchema::new(view);

            let sender = match schema.wallet(&author) {
                Some(val) => val,
                None => Err(Error::SenderNotFound)?,
            };

            let receiver = match schema.wallet(&self.to) {
                Some(val) => val,
                None => Err(Error::ReceiverNotFound)?,
            };

            let amount = self.amount;
            if sender.balance >= amount {
                let sender = sender.decrease(amount);
                let receiver = receiver.increase(amount);
                println!("Transfer between wallets: {:?} => {:?}", sender, receiver);
                let mut wallets = schema.wallets_mut();
                wallets.put(&author, sender);
                wallets.put(&self.to, receiver);
                Ok(())
            } else {
                Err(Error::InsufficientCurrencyAmount)?
            }
        }
    }
}

/// REST API.
pub mod api {
    use exonum::{
        api::{self, ServiceApiBuilder, ServiceApiState},
        crypto::PublicKey,
    };

    use schema::{CurrencySchema, Wallet};

    /// Public service API description.
    #[derive(Debug, Clone)]
    pub struct CryptocurrencyApi;

    /// The structure describes the query parameters for the `get_wallet` endpoint.
    #[derive(Debug, Serialize, Deserialize, Clone, Copy)]
    pub struct WalletQuery {
        /// Public key of the queried wallet.
        pub pub_key: PublicKey,
    }

    impl CryptocurrencyApi {
        /// Endpoint for getting a single wallet.
        pub fn get_wallet(state: &ServiceApiState, query: WalletQuery) -> api::Result<Wallet> {
            let snapshot = state.snapshot();
            let schema = CurrencySchema::new(snapshot);
            schema
                .wallet(&query.pub_key)
                .ok_or_else(|| api::Error::NotFound("\"Wallet not found\"".to_owned()))
        }

        /// Endpoint for dumping all wallets from the storage.
        pub fn get_wallets(state: &ServiceApiState, _query: ()) -> api::Result<Vec<Wallet>> {
            let snapshot = state.snapshot();
            let schema = CurrencySchema::new(snapshot);
            let idx = schema.wallets();
            let wallets = idx.values().collect();
            Ok(wallets)
        }

        /// 'ServiceApiBuilder' facilitates conversion between read requests and REST
        /// endpoints.
        pub fn wire(builder: &mut ServiceApiBuilder) {
            // Binds handlers to specific routes.
            builder
                .public_scope()
                .endpoint("v1/wallet", Self::get_wallet)
                .endpoint("v1/wallets", Self::get_wallets);
        }
    }
}

/// Service declaration.
pub mod service {
    use exonum::{
        api::ServiceApiBuilder,
        blockchain::{Service, Transaction, TransactionSet},
        crypto::Hash,
        messages::RawTransaction,
        storage::Snapshot,
    };

    use api::CryptocurrencyApi;
    use transactions::CurrencyTransactions;

    /// Service ID for the `Service` trait.
    pub const SERVICE_ID: u16 = 1;

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
    /// GET `api/services/cryptocurrency/v1/wallet/?pub_key={hash}`
    ///
    /// Returns information about a wallet with the specified public key (hex-encoded).
    /// If a wallet with the specified pubkey is not in the storage, returns a string
    /// `"Wallet not found"` with the HTTP 404 status.
    ///
    /// ## Dump wallets
    ///
    /// GET `api/services/cryptocurrency/v1/wallets`
    ///
    /// Returns an array of all wallets in the storage.
    ///
    /// ## Transactions endpoint
    ///
    /// POST `api/explorer/v1/transactions`
    ///
    /// Accepts a [`TxTransfer`] and [`TxCreateWallet`] transaction from an external client.
    /// Transaction should be serialized into protobuf binary form and placed into signed
    /// transaction message according to specification, endpoint accepts hex of this signed
    /// transaction message as an object: `{ "tx_body": <hex> }`.
    ///
    /// Returns the hex-encoded hash of the transaction
    /// encumbered in an object: `{ "tx_hash": <hash> }`.
    ///
    /// [`TxCreateWallet`]: ../transactions/struct.TxCreateWallet.html
    /// [`TxTransfer`]: ../transactions/struct.TxTransfer.html
    #[derive(Debug)]
    pub struct CurrencyService;

    impl Service for CurrencyService {
        fn service_name(&self) -> &'static str {
            "cryptocurrency"
        }

        fn service_id(&self) -> u16 {
            SERVICE_ID
        }

        // Implement a method to deserialize transactions coming to the node.
        fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<dyn Transaction>, failure::Error> {
            let tx = CurrencyTransactions::tx_from_raw(raw)?;
            Ok(tx.into())
        }

        // Hashes for the service tables that will be included into the state hash.
        // To simplify things, we don't have [Merkelized tables][merkle] in the service storage
        // for now, so we return an empty vector.
        //
        // [merkle]: https://exonum.com/doc/architecture/storage/#merklized-indices
        fn state_hash(&self, _: &dyn Snapshot) -> Vec<Hash> {
            vec![]
        }

        // Links the service api implementation to the Exonum.
        fn wire_api(&self, builder: &mut ServiceApiBuilder) {
            CryptocurrencyApi::wire(builder);
        }
    }
}
