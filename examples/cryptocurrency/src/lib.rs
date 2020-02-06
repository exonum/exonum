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
#[cfg(test)]
mod tx_tests;

/// Persistent data.
pub mod schema {
    use exonum::{
        crypto::PublicKey,
        merkledb::{
            access::{Access, FromAccess},
            MapIndex,
        },
    };
    use exonum_derive::{BinaryValue, FromAccess, ObjectHash};
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
    ///
    /// Note that the schema is fully private; it is exposed to the clients via service HTTP API.
    #[derive(Debug, FromAccess)]
    pub(crate) struct CurrencySchema<T: Access> {
        /// Correspondence of public keys of users to the account information.
        pub wallets: MapIndex<T::Base, PublicKey, Wallet>,
    }

    impl<T: Access> CurrencySchema<T> {
        pub fn new(access: T) -> Self {
            Self::from_root(access).unwrap()
        }
    }
}

/// Transactions.
pub mod transactions {
    use exonum::crypto::PublicKey;
    use exonum_derive::{BinaryValue, ObjectHash};
    use exonum_proto::ProtobufConvert;

    use super::proto;

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
    #[derive(Clone, Debug)]
    #[derive(Serialize, Deserialize)]
    #[derive(ProtobufConvert, BinaryValue, ObjectHash)]
    #[protobuf_convert(source = "proto::TxTransfer")]
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
    use exonum::runtime::{ExecutionContext, ExecutionError};
    use exonum_derive::{exonum_interface, interface_method, ServiceDispatcher, ServiceFactory};
    use exonum_rust_runtime::{api::ServiceApiBuilder, DefaultInstance, Service};

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
        #[interface_method(id = 0)]
        fn create_wallet(&self, ctx: Ctx, arg: CreateWallet) -> Self::Output;
        /// Transfers `amount` of the currency from one wallet to another.
        #[interface_method(id = 1)]
        fn transfer(&self, ctx: Ctx, arg: TxTransfer) -> Self::Output;
    }

    /// Cryptocurrency service implementation.
    #[derive(Debug, ServiceFactory, ServiceDispatcher)]
    #[service_dispatcher(implements("CryptocurrencyInterface"))]
    #[service_factory(proto_sources = "crate::proto")]
    pub struct CryptocurrencyService;

    impl CryptocurrencyInterface<ExecutionContext<'_>> for CryptocurrencyService {
        type Output = Result<(), ExecutionError>;

        fn create_wallet(&self, context: ExecutionContext<'_>, arg: CreateWallet) -> Self::Output {
            let author = context
                .caller()
                .author()
                .expect("Wrong `TxCreateWallet` initiator");

            let mut schema = CurrencySchema::new(context.service_data());
            if schema.wallets.get(&author).is_none() {
                let wallet = Wallet::new(&author, &arg.name, INIT_BALANCE);
                println!("Created wallet: {:?}", wallet);
                schema.wallets.put(&author, wallet);
                Ok(())
            } else {
                Err(Error::WalletAlreadyExists.into())
            }
        }

        fn transfer(&self, context: ExecutionContext<'_>, arg: TxTransfer) -> Self::Output {
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
            CryptocurrencyApi::wire(builder);
        }
    }

    // Specify default instantiation parameters for the service.
    impl DefaultInstance for CryptocurrencyService {
        const INSTANCE_ID: u32 = 101;
        const INSTANCE_NAME: &'static str = "cryptocurrency";
    }
}

/// Cryptocurrency API implementation.
pub mod api {
    use exonum::crypto::PublicKey;
    use exonum_rust_runtime::api::{self, ServiceApiBuilder, ServiceApiState};

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
        pub fn get_wallet(state: &ServiceApiState<'_>, query: WalletQuery) -> api::Result<Wallet> {
            let schema = CurrencySchema::new(state.service_data());
            schema
                .wallets
                .get(&query.pub_key)
                .ok_or_else(|| api::Error::not_found().title("Wallet not found"))
        }

        /// Endpoint for dumping all wallets from the storage.
        pub fn get_wallets(state: &ServiceApiState<'_>, _query: ()) -> api::Result<Vec<Wallet>> {
            let schema = CurrencySchema::new(state.service_data());
            Ok(schema.wallets.values().collect())
        }

        /// `ServiceApiBuilder` facilitates conversion between read requests and REST
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
