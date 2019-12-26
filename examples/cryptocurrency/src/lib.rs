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
extern crate serde_derive; // Required for Protobuf.

pub mod proto;

/// Persistent data.
pub mod schema {
    use exonum_crypto::PublicKey;
    use exonum_derive::{BinaryValue, FromAccess, ObjectHash};
    use exonum_merkledb::{access::Access, MapIndex};
    use exonum_proto::ProtobufConvert;

    use super::proto;

    // Declare the data to be stored in the blockchain, namely wallets with balances.
    // See [serialization docs][1] for details.
    //
    // [1]: https://exonum.com/doc/version/latest/architecture/serialization
    /// Wallet struct used to persist data within the service.
    #[derive(Clone, Debug)]
    #[derive(Serialize, Deserialize)]
    #[derive(ProtobufConvert, BinaryValue, ObjectHash)]
    #[protobuf_convert(source = "proto::Wallet")]
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
    #[derive(Debug, FromAccess)]
    pub struct CurrencySchema<T: Access> {
        /// Correspondence of public keys of users to account information.
        pub wallets: MapIndex<T::Base, PublicKey, Wallet>,
    }
}

/// Transactions.
pub mod transactions {
    use exonum_crypto::PublicKey;
    use exonum_derive::{BinaryValue, ObjectHash};
    use exonum_proto::ProtobufConvert;

    use super::proto;

    /// Service configuration parameters.
    #[derive(Clone, Debug)]
    #[derive(Serialize, Deserialize)]
    #[derive(ProtobufConvert, BinaryValue, ObjectHash)]
    #[protobuf_convert(source = "proto::Config")]
    pub struct Config;

    /// Transaction type for creating a new wallet.
    ///
    /// See [the `Transaction` trait implementation](#impl-Transaction) for details how
    /// `TxCreateWallet` transactions are processed.
    #[derive(Clone, Debug)]
    #[derive(Serialize, Deserialize)]
    #[derive(ProtobufConvert, BinaryValue, ObjectHash)]
    #[protobuf_convert(source = "proto::TxCreateWallet")]
    pub struct CreateWallet {
        /// UTF-8 string with the owner's name.
        pub name: String,
    }

    impl CreateWallet {
        /// Creates a wallet with the specified name.
        pub fn new(name: impl Into<String>) -> Self {
            Self { name: name.into() }
        }
    }

    /// Transaction type for transferring tokens between two wallets.
    ///
    /// See [the `Transaction` trait implementation](#impl-Transaction) for details how
    /// `TxTransfer` transactions are processed.
    #[protobuf_convert(source = "proto::TxTransfer")]
    #[derive(Clone, Debug)]
    #[derive(Serialize, Deserialize)]
    #[derive(ProtobufConvert, BinaryValue, ObjectHash)]
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
    use exonum_derive::ExecutionFail;

    /// Error codes emitted by `TxCreateWallet` and/or `TxTransfer` transactions during execution.
    #[derive(Debug, ExecutionFail)]
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
        rust::{api::ServiceApiBuilder, CallContext, Service},
        ExecutionError,
    };
    use exonum_derive::{exonum_interface, ServiceDispatcher, ServiceFactory};

    use crate::{
        api::CryptocurrencyApi,
        errors::Error,
        schema::{CurrencySchema, Wallet},
        transactions::{CreateWallet, TxTransfer},
    };

    /// Initial balance of a newly created wallet.
    const INIT_BALANCE: u64 = 100;

    /// Cryptocurrency service transactions.
    #[exonum_interface]
    pub trait CryptocurrencyInterface<Ctx> {
        /// Output of the methods in this interface.
        type Output;

        /// Creates wallet with the given `name`.
        fn create_wallet(&self, ctx: Ctx, arg: CreateWallet) -> Self::Output;
        /// Transfers `amount` of the currency from one wallet to another.
        fn transfer(&self, ctx: Ctx, arg: TxTransfer) -> Self::Output;
    }

    /// Cryptocurrency service implementation.
    #[derive(Debug, ServiceFactory, ServiceDispatcher)]
    #[service_dispatcher(implements("CryptocurrencyInterface"))]
    #[service_factory(proto_sources = "crate::proto")]
    pub struct CryptocurrencyService;

    impl CryptocurrencyInterface<CallContext<'_>> for CryptocurrencyService {
        type Output = Result<(), ExecutionError>;

        fn create_wallet(&self, context: CallContext<'_>, arg: CreateWallet) -> Self::Output {
            let author = context
                .caller()
                .author()
                .expect("Wrong 'TxCreateWallet' initiator");

            let mut schema = CurrencySchema::new(context.service_data());
            if schema.wallets.get(&author).is_none() {
                let wallet = Wallet::new(&author, &arg.name, INIT_BALANCE);
                println!("Create the wallet: {:?}", wallet);
                schema.wallets.put(&author, wallet);
                Ok(())
            } else {
                Err(Error::WalletAlreadyExists.into())
            }
        }

        fn transfer(&self, context: CallContext<'_>, arg: TxTransfer) -> Self::Output {
            let author = context
                .caller()
                .author()
                .expect("Wrong 'TxTransfer' initiator");
            if author == arg.to {
                return Err(Error::SenderSameAsReceiver.into());
            }

            let mut schema = CurrencySchema::new(context.service_data());
            let sender = schema.wallets.get(&author).ok_or(Error::SenderNotFound)?;
            let receiver = schema.wallets.get(&arg.to).ok_or(Error::ReceiverNotFound)?;

            let amount = arg.amount;
            if sender.balance >= amount {
                let sender = sender.decrease(amount);
                let receiver = receiver.increase(amount);
                println!("Transfer between wallets: {:?} => {:?}", sender, receiver);
                schema.wallets.put(&author, sender);
                schema.wallets.put(&arg.to, receiver);
                Ok(())
            } else {
                Err(Error::InsufficientCurrencyAmount.into())
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
        runtime::rust::api::{self, ServiceApiBuilder, ServiceApiState},
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
            state: &ServiceApiState<'_>,
            pub_key: PublicKey,
        ) -> api::Result<Wallet> {
            let schema = CurrencySchema::new(state.service_data());
            schema
                .wallets
                .get(&pub_key)
                .ok_or_else(|| api::Error::NotFound("\"Wallet not found\"".to_owned()))
        }

        /// Endpoint for dumping all wallets from the storage.
        pub fn get_wallets(self, state: &ServiceApiState<'_>) -> api::Result<Vec<Wallet>> {
            let schema = CurrencySchema::new(state.service_data());
            Ok(schema.wallets.values().collect())
        }

        /// 'ServiceApiBuilder' facilitates conversion between read requests and REST
        /// endpoints.
        pub fn wire(self, builder: &mut ServiceApiBuilder) {
            // Binds handlers to specific routes.
            builder
                .public_scope()
                .endpoint("v1/wallet", {
                    move |state: &ServiceApiState<'_>, query: WalletQuery| {
                        self.get_wallet(state, query.pub_key)
                    }
                })
                .endpoint("v1/wallets", {
                    move |state: &ServiceApiState<'_>, _query: ()| self.get_wallets(state)
                });
        }
    }
}
