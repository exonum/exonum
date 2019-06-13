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

//! Cryptocurrency implementation example using [exonum](http://exonum.com/).

#![deny(unsafe_code, bare_trait_objects)]
#![warn(missing_docs, missing_debug_implementations)]

#[macro_use]
extern crate exonum_derive;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate serde_derive;

pub use crate::schema::Schema;

pub mod api;
pub mod proto;
pub mod schema;
pub mod transactions;
pub mod wallet;

use exonum::{
    api::ServiceApiBuilder,
    blockchain::{ExecutionError, ExecutionResult},
    helpers::fabric::{self, Context},
    impl_service_dispatcher,
    runtime::rust::{
        RustArtifactSpec, Service, ServiceDescriptor, ServiceFactory, TransactionContext,
    },
};

use crate::{
    api::PublicApi as CryptocurrencyApi,
    transactions::{CreateWallet, Error, Issue, Transfer, ERROR_SENDER_SAME_AS_RECEIVER},
};

/// Unique service ID.
const CRYPTOCURRENCY_SERVICE_ID: u16 = 128;
/// Name of the service.
const SERVICE_NAME: &str = "cryptocurrency";
/// Initial balance of the wallet.
const INITIAL_BALANCE: u64 = 100;

#[service_interface]
pub trait Cryptocurrency {
    fn transfer(&self, ctx: TransactionContext, arg: Transfer) -> ExecutionResult;
    fn issue(&self, ctx: TransactionContext, arg: Issue) -> ExecutionResult;
    fn create_wallet(&self, ctx: TransactionContext, arg: CreateWallet) -> ExecutionResult;
}

#[derive(Debug)]
pub struct CryptocurrencyServiceImpl;

impl Cryptocurrency for CryptocurrencyServiceImpl {
    fn transfer(&self, context: TransactionContext, arg: Transfer) -> ExecutionResult {
        let from = &context.author();
        let hash = context.tx_hash();

        let mut schema = Schema::new(context.fork());

        let to = &arg.to;
        let amount = arg.amount;

        if from == to {
            return Err(ExecutionError::new(ERROR_SENDER_SAME_AS_RECEIVER));
        }

        let sender = schema.wallet(from).ok_or(Error::SenderNotFound)?;

        let receiver = schema.wallet(to).ok_or(Error::ReceiverNotFound)?;

        if sender.balance < amount {
            Err(Error::InsufficientCurrencyAmount)?
        }

        schema.decrease_wallet_balance(sender, amount, &hash);
        schema.increase_wallet_balance(receiver, amount, &hash);

        Ok(())
    }

    fn issue(&self, context: TransactionContext, arg: Issue) -> ExecutionResult {
        let pub_key = &context.author();
        let hash = context.tx_hash();

        let mut schema = Schema::new(context.fork());

        if let Some(wallet) = schema.wallet(pub_key) {
            let amount = arg.amount;
            schema.increase_wallet_balance(wallet, amount, &hash);
            Ok(())
        } else {
            Err(Error::ReceiverNotFound)?
        }
    }

    fn create_wallet(&self, context: TransactionContext, arg: CreateWallet) -> ExecutionResult {
        let pub_key = &context.author();
        let hash = context.tx_hash();

        let mut schema = Schema::new(context.fork());

        if schema.wallet(pub_key).is_none() {
            let name = &arg.name;
            schema.create_wallet(pub_key, name, &hash);
            Ok(())
        } else {
            Err(Error::WalletAlreadyExists)?
        }
    }
}

impl_service_dispatcher!(CryptocurrencyServiceImpl, Cryptocurrency);

impl Service for CryptocurrencyServiceImpl {
    fn wire_api(&self, _descriptor: ServiceDescriptor, builder: &mut ServiceApiBuilder) {
        CryptocurrencyApi::wire(builder);
    }
}

#[derive(Debug)]
pub struct CryptocurrencyServiceFactory;

impl ServiceFactory for CryptocurrencyServiceFactory {
    fn artifact(&self) -> RustArtifactSpec {
        RustArtifactSpec::new(SERVICE_NAME, 0, 1, 0)
    }

    fn new_instance(&self) -> Box<dyn Service> {
        Box::new(CryptocurrencyServiceImpl)
    }
}
