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

//! The time oracle service for Exonum.
//!
//! See [the Exonum documentation][docs:time] for a high-level overview of the service,
//! in particular, its design rationale and the proof of correctness.
//!
//! [docs:time]: https://exonum.com/doc/advanced/time

#![deny(missing_debug_implementations, missing_docs)]

extern crate bodyparser;
extern crate chrono;
#[macro_use]
extern crate exonum;
#[macro_use]
extern crate failure;
extern crate iron;
extern crate router;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;

/// Node API.
pub mod api;
/// Database schema.
pub mod schema;
/// System time provider.
pub mod time_provider;
/// Node transactions.
pub mod transactions;

use exonum::api::Api;
use exonum::blockchain::{ApiContext, Service, ServiceContext, Transaction, TransactionSet};
use exonum::crypto::Hash;
use exonum::encoding;
use exonum::encoding::serialize::json::reexport::Value;
use exonum::helpers::fabric::{Context, ServiceFactory};
use exonum::messages::RawTransaction;
use exonum::storage::{Fork, Snapshot};

use iron::Handler;
use router::Router;
use schema::TimeSchema;
use time_provider::{SystemTimeProvider, TimeProvider};
use transactions::*;

/// Time service id.
pub const SERVICE_ID: u16 = 4;
/// Time service name.
pub const SERVICE_NAME: &str = "exonum_time";

/// Define the service.
#[derive(Debug)]
pub struct TimeService {
    /// Current time.
    time: Box<TimeProvider>,
}

impl Default for TimeService {
    fn default() -> TimeService {
        TimeService {
            time: Box::new(SystemTimeProvider) as Box<TimeProvider>,
        }
    }
}

impl TimeService {
    /// Create a new `TimeService`.
    pub fn new() -> TimeService {
        TimeService::default()
    }

    /// Create a new `TimeService` with time provider `T`.
    pub fn with_provider<T: Into<Box<TimeProvider>>>(time_provider: T) -> TimeService {
        TimeService {
            time: time_provider.into(),
        }
    }
}

impl Service for TimeService {
    fn service_name(&self) -> &str {
        SERVICE_NAME
    }

    fn state_hash(&self, snapshot: &Snapshot) -> Vec<Hash> {
        let schema = TimeSchema::new(snapshot);
        schema.state_hash()
    }

    fn service_id(&self) -> u16 {
        SERVICE_ID
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, encoding::Error> {
        TimeTransactions::tx_from_raw(raw).map(Into::into)
    }

    fn initialize(&self, _fork: &mut Fork) -> Value {
        Value::Null
    }

    /// Creates transaction after commit of the block.
    fn after_commit(&self, context: &ServiceContext) {
        // The transaction must be created by the validator.
        if context.validator_id().is_none() {
            return;
        }
        let (pub_key, sec_key) = (*context.public_key(), context.secret_key().clone());
        context
            .transaction_sender()
            .send(Box::new(TxTime::new(
                self.time.current_time(),
                &pub_key,
                &sec_key,
            )))
            .unwrap();
    }

    fn private_api_handler(&self, ctx: &ApiContext) -> Option<Box<Handler>> {
        let mut router = Router::new();
        let api = api::PrivateApi {
            blockchain: ctx.blockchain().clone(),
        };
        api.wire(&mut router);
        Some(Box::new(router))
    }

    fn public_api_handler(&self, ctx: &ApiContext) -> Option<Box<Handler>> {
        let mut router = Router::new();
        let api = api::PublicApi {
            blockchain: ctx.blockchain().clone(),
        };
        api.wire(&mut router);
        Some(Box::new(router))
    }
}

/// A time service creator for the `NodeBuilder`.
#[derive(Debug)]
pub struct TimeServiceFactory;

impl ServiceFactory for TimeServiceFactory {
    fn make_service(&mut self, _: &Context) -> Box<Service> {
        Box::new(TimeService::new())
    }
}
