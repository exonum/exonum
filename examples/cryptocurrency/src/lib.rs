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

//! Demo [Exonum][exonum] service implementing a simple cryptocurrency.
//! See [the documentation][docs] for a detailed step-by-step guide how to approach this demo,
//! and [the repository README][readme] on how to use, test, and contribute to it.
//!
//! **Note.** The service in this crate is intended for demo purposes only. It is not intended
//! for use in production.
//!
//! [exonum]: https://github.com/exonum/exonum
//! [docs]: https://exonum.com/doc/version/latest/get-started/create-service
//! [readme]: https://github.com/exonum/cryptocurrency#readme

#![deny(
    missing_debug_implementations,
    missing_docs,
    unsafe_code,
    bare_trait_objects
)]

#[macro_use]
extern crate exonum_derive;
#[macro_use]
extern crate serde_derive;

pub mod proto;

/// Persistent data.
pub mod schema {
    use exonum::crypto::PublicKey;
    use exonum_merkledb::{IndexAccess, MapIndex};

    use super::proto;

    // Declare the data to be stored in the blockchain, namely wallets with balances.
    // See [serialization docs][1] for details.
    //
    // [1]: https://exonum.com/doc/version/latest/architecture/serialization
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
    pub struct CurrencySchema<'a, T> {
        name: &'a str,
        access: T,
    }

    /// Declare the layout of data managed by the service. An instance of [`MapIndex`] is used
    /// to keep wallets in the storage. Index values are serialized [`Wallet`] structs.
    ///
    /// [`MapIndex`]: https://exonum.com/doc/version/latest/architecture/storage#mapindex
    /// [`Wallet`]: struct.Wallet.html
    impl<'a, T: IndexAccess> CurrencySchema<'a, T> {
        /// Creates a new schema instance.
        pub fn new(name: &'a str, access: T) -> Self {
            CurrencySchema { name, access }
        }

        /// Returns an immutable version of the wallets table.
        pub fn wallets(&self) -> MapIndex<T, PublicKey, Wallet> {
            MapIndex::new([self.name, ".wallets"].concat(), self.access.clone())
        }

        /// Gets a specific wallet from the storage.
        pub fn wallet(&self, pub_key: &PublicKey) -> Option<Wallet> {
            self.wallets().get(pub_key)
        }
    }
}

/// Transactions.
pub mod transactions {
    use exonum::crypto::PublicKey;

    use super::proto;

    /// Service configuration parameters.
    #[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert)]
    #[exonum(pb = "proto::Config")]
    pub struct Config;

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
}

/// Contract errors.
pub mod errors {
    /// Error codes emitted by `TxCreateWallet` and/or `TxTransfer` transactions during execution.
    #[derive(Debug, IntoExecutionError)]
    pub enum Error {
        /// Wallet already exists.
        ///
        /// Can be emitted by `TxCreateWallet`.
        WalletAlreadyExists = 0,
        /// Sender doesn't exist.
        ///
        /// Can be emitted by `TxTransfer`.
        SenderNotFound = 1,
        /// Receiver doesn't exist.
        ///
        /// Can be emitted by `TxTransfer`.
        ReceiverNotFound = 2,
        /// Insufficient currency amount.
        ///
        /// Can be emitted by `TxTransfer`.
        InsufficientCurrencyAmount = 3,
        /// Sender same as receiver.
        ///
        /// Can be emitted by `TxTransfer`.
        SenderSameAsReceiver = 4,
    }
}

/// Contracts.
pub mod contracts {
    use exonum::runtime::{
        api::ServiceApiBuilder,
        rust::{Service, TransactionContext},
    };

    use crate::{
        api::CryptocurrencyApi,
        errors::Error,
        schema::{CurrencySchema, Wallet},
        transactions::{TxCreateWallet, TxTransfer},
    };

    /// Initial balance of a newly created wallet.
    const INIT_BALANCE: u64 = 100;

    /// Cryptocurrency service transactions.
    #[exonum_service]
    pub trait CryptocurrencyInterface {
        /// Creates wallet with the given `name`.
        fn create_wallet(&self, ctx: TransactionContext, arg: TxCreateWallet) -> Result<(), Error>;
        /// Transfers `amount` of the currency from one wallet to another.
        fn transfer(&self, ctx: TransactionContext, arg: TxTransfer) -> Result<(), Error>;
    }

    /// Cryptocurrency service implementation.
    #[derive(Debug, ServiceFactory)]
    #[exonum(
        proto_sources = "crate::proto",
        interfaces(default = "CryptocurrencyInterface")
    )]
    pub struct CryptocurrencyService;

    impl CryptocurrencyInterface for CryptocurrencyService {
        fn create_wallet(
            &self,
            context: TransactionContext,
            arg: TxCreateWallet,
        ) -> Result<(), Error> {
            let author = context
                .caller()
                .author()
                .expect("Wrong 'TxCreateWallet' initiator");

            let view = context.fork();
            let schema = CurrencySchema::new(context.instance.name, view);
            if schema.wallet(&author).is_none() {
                let wallet = Wallet::new(&author, &arg.name, INIT_BALANCE);
                println!("Create the wallet: {:?}", wallet);
                schema.wallets().put(&author, wallet);
                Ok(())
            } else {
                Err(Error::WalletAlreadyExists)?
            }
        }

        fn transfer(&self, context: TransactionContext, arg: TxTransfer) -> Result<(), Error> {
            let author = context
                .caller()
                .author()
                .expect("Wrong 'TxTransfer' initiator");

            let view = context.fork();

            if author == arg.to {
                Err(Error::SenderSameAsReceiver)?
            }

            let schema = CurrencySchema::new(context.instance.name, view);

            let sender = match schema.wallet(&author) {
                Some(val) => val,
                None => Err(Error::SenderNotFound)?,
            };

            let receiver = match schema.wallet(&arg.to) {
                Some(val) => val,
                None => Err(Error::ReceiverNotFound)?,
            };

            let amount = arg.amount;
            if sender.balance >= amount {
                let sender = sender.decrease(amount);
                let receiver = receiver.increase(amount);
                println!("Transfer between wallets: {:?} => {:?}", sender, receiver);
                let mut wallets = schema.wallets();
                wallets.put(&author, sender);
                wallets.put(&arg.to, receiver);
                Ok(())
            } else {
                Err(Error::InsufficientCurrencyAmount)?
            }
        }
    }

    impl Service for CryptocurrencyService {
        fn wire_api(&self, builder: &mut ServiceApiBuilder) {
            CryptocurrencyApi.wire(builder);
        }
    }
}

/// Cryptocurrency API implementation.
pub mod api {
    use exonum::{
        crypto::PublicKey,
        runtime::api::{self, ServiceApiBuilder, ServiceApiState},
    };

    use crate::schema::{CurrencySchema, Wallet};

    /// Public service API description.
    #[derive(Debug, Clone, Copy)]
    pub struct CryptocurrencyApi;

    /// The structure describes the query parameters for the `get_wallet` endpoint.
    #[derive(Debug, Serialize, Deserialize, Clone, Copy)]
    pub struct WalletQuery {
        /// Public key of the queried wallet.
        pub pub_key: PublicKey,
    }

    impl CryptocurrencyApi {
        /// Endpoint for getting a single wallet.
        pub fn get_wallet(
            self,
            state: &ServiceApiState,
            pub_key: PublicKey,
        ) -> api::Result<Wallet> {
            let snapshot = state.snapshot();
            let schema = CurrencySchema::new(state.instance.name, snapshot);
            schema
                .wallet(&pub_key)
                .ok_or_else(|| api::Error::NotFound("\"Wallet not found\"".to_owned()))
        }

        /// Endpoint for dumping all wallets from the storage.
        pub fn get_wallets(self, state: &ServiceApiState) -> api::Result<Vec<Wallet>> {
            let snapshot = state.snapshot();
            let schema = CurrencySchema::new(state.instance.name, snapshot);
            let idx = schema.wallets();
            let wallets = idx.values().collect();
            Ok(wallets)
        }

        /// 'ServiceApiBuilder' facilitates conversion between read requests and REST
        /// endpoints.
        pub fn wire(self, builder: &mut ServiceApiBuilder) {
            // Binds handlers to specific routes.
            builder
                .public_scope()
                .endpoint("v1/wallet", {
                    move |state: &ServiceApiState, query: WalletQuery| {
                        self.get_wallet(state, query.pub_key)
                    }
                })
                .endpoint("v1/wallets", {
                    move |state: &ServiceApiState, _query: ()| self.get_wallets(state)
                });
        }
    }
}
