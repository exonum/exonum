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

#![deny(missing_debug_implementations, missing_docs, unsafe_code, bare_trait_objects)]

#[macro_use]
extern crate exonum;
#[macro_use]
extern crate failure;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

/// Persistent data.
pub mod schema {
    use exonum::{
        crypto::PublicKey, storage::{Fork, MapIndex, Snapshot},
    };

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
    use exonum::crypto::PublicKey;

    use service::SERVICE_ID;

    transactions! {
        /// Transaction group.
        pub CurrencyTransactions {
            const SERVICE_ID = SERVICE_ID;

            /// Transaction type for creating a new wallet.
            ///
            /// See [the `Transaction` trait implementation](#impl-Transaction) for details how
            /// `TxCreateWallet` transactions are processed.
            struct TxCreateWallet {
                /// Public key of the wallet's owner.
                pub_key: &PublicKey,
                /// UTF-8 string with the owner's name.
                name: &str,
            }

            /// Transaction type for transferring tokens between two wallets.
            ///
            /// See [the `Transaction` trait implementation](#impl-Transaction) for details how
            /// `TxTransfer` transactions are processed.
            struct TxTransfer {
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
    use exonum::{
        blockchain::{ExecutionResult, Transaction}, messages::Message, storage::Fork,
    };

    use errors::Error;
    use schema::{CurrencySchema, Wallet};
    use transactions::{TxCreateWallet, TxTransfer};

    /// Initial balance of a newly created wallet.
    const INIT_BALANCE: u64 = 100;

    impl Transaction for TxCreateWallet {
        /// Verifies integrity of the transaction by checking the transaction
        /// signature.
        fn verify(&self) -> bool {
            self.verify_signature(self.pub_key())
        }

        /// If a wallet with the specified public key is not registered, then creates a new wallet
        /// with the specified public key and name, and an initial balance of 100.
        /// Otherwise, performs no op.
        fn execute(&self, view: &mut Fork) -> ExecutionResult {
            let mut schema = CurrencySchema::new(view);
            if schema.wallet(self.pub_key()).is_none() {
                let wallet = Wallet::new(self.pub_key(), self.name(), INIT_BALANCE);
                println!("Create the wallet: {:?}", wallet);
                schema.wallets_mut().put(self.pub_key(), wallet);
                Ok(())
            } else {
                Err(Error::WalletAlreadyExists)?
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
        /// [`TxCreateWallet`]: ../transactions/struct.TxCreateWallet.html
        fn execute(&self, view: &mut Fork) -> ExecutionResult {
            let mut schema = CurrencySchema::new(view);

            let sender = match schema.wallet(self.from()) {
                Some(val) => val,
                None => Err(Error::SenderNotFound)?,
            };

            let receiver = match schema.wallet(self.to()) {
                Some(val) => val,
                None => Err(Error::ReceiverNotFound)?,
            };

            let amount = self.amount();
            if sender.balance() >= amount {
                let sender = sender.decrease(amount);
                let receiver = receiver.increase(amount);
                println!("Transfer between wallets: {:?} => {:?}", sender, receiver);
                let mut wallets = schema.wallets_mut();
                wallets.put(self.from(), sender);
                wallets.put(self.to(), receiver);
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
        api::{self, ServiceApiBuilder, ServiceApiState}, blockchain::Transaction,
        crypto::{Hash, PublicKey}, node::TransactionSend,
    };

    use schema::{CurrencySchema, Wallet};
    use transactions::CurrencyTransactions;

    /// Public service API description.
    #[derive(Debug, Clone)]
    pub struct CryptocurrencyApi;

    /// The structure describes the query parameters for the `get_wallet` endpoint.
    #[derive(Debug, Serialize, Deserialize, Clone, Copy)]
    pub struct WalletQuery {
        /// Public key of the queried wallet.
        pub pub_key: PublicKey,
    }

    /// The structure returned by the REST API.
    #[derive(Debug, Serialize, Deserialize)]
    pub struct TransactionResponse {
        /// Hash of the transaction.
        pub tx_hash: Hash,
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

        /// Common processing for transaction-accepting endpoints.
        pub fn post_transaction(
            state: &ServiceApiState,
            query: CurrencyTransactions,
        ) -> api::Result<TransactionResponse> {
            let transaction: Box<dyn Transaction> = query.into();
            let tx_hash = transaction.hash();
            state.sender().send(transaction)?;
            Ok(TransactionResponse { tx_hash })
        }

        /// 'ServiceApiBuilder' facilitates conversion between transactions/read requests and REST
        /// endpoints; for example, it parses `POST`ed JSON into the binary transaction
        /// representation used in Exonum internally.
        pub fn wire(builder: &mut ServiceApiBuilder) {
            // Binds handlers to specific routes.
            builder
                .public_scope()
                .endpoint("v1/wallet", Self::get_wallet)
                .endpoint("v1/wallets", Self::get_wallets)
                .endpoint_mut("v1/wallets", Self::post_transaction)
                .endpoint_mut("v1/wallets/transfer", Self::post_transaction);
        }
    }
}

/// Service declaration.
pub mod service {
    use exonum::{
        api::ServiceApiBuilder, blockchain::{Service, Transaction, TransactionSet}, crypto::Hash,
        encoding, messages::RawTransaction, storage::Snapshot,
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
    /// GET `v1/wallet/?pub_key={hash}`
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
        fn tx_from_raw(
            &self,
            raw: RawTransaction,
        ) -> Result<Box<dyn Transaction>, encoding::Error> {
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
