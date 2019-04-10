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

//! The time oracle service for Exonum.
//!
//! See [the Exonum documentation][docs:time] for a high-level overview of the service,
//! in particular, its design rationale and the proof of correctness.
//!
//! [docs:time]: https://exonum.com/doc/version/latest/advanced/time

#![deny(
    missing_debug_implementations,
    missing_docs,
    unsafe_code,
    bare_trait_objects
)]

#[macro_use]
extern crate failure;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate exonum_derive;

/// Node API.
pub mod api;
/// Protobuf generated structs.
pub mod proto;
/// Database schema.
pub mod schema;
/// System time provider.
pub mod time_provider;
/// Node transactions.
pub mod transactions;

use exonum_merkledb::{Fork, Snapshot};

use exonum::{
    api::ServiceApiBuilder,
    blockchain::{Service, ServiceContext, Transaction, TransactionSet},
    crypto::Hash,
    helpers::fabric::{Context, ServiceFactory},
    messages::RawTransaction,
};
use serde_json::Value;

use crate::{
    schema::TimeSchema,
    time_provider::{SystemTimeProvider, TimeProvider},
    transactions::*,
};

/// Time service id.
pub const SERVICE_ID: u16 = 4;
/// Time service name.
pub const SERVICE_NAME: &str = "exonum_time";

/// Define the service.
#[derive(Debug)]
pub struct TimeService {
    /// Current time.
    time: Box<dyn TimeProvider>,
}

impl Default for TimeService {
    fn default() -> TimeService {
        TimeService {
            time: Box::new(SystemTimeProvider) as Box<dyn TimeProvider>,
        }
    }
}

impl TimeService {
    /// Create a new `TimeService`.
    pub fn new() -> TimeService {
        TimeService::default()
    }

    /// Create a new `TimeService` with time provider `T`.
    pub fn with_provider<T: Into<Box<dyn TimeProvider>>>(time_provider: T) -> TimeService {
        TimeService {
            time: time_provider.into(),
        }
    }
}

impl Service for TimeService {
    fn service_name(&self) -> &str {
        SERVICE_NAME
    }

    fn state_hash(&self, snapshot: &dyn Snapshot) -> Vec<Hash> {
        let schema = TimeSchema::new(snapshot);
        schema.state_hash()
    }

    fn service_id(&self) -> u16 {
        SERVICE_ID
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<dyn Transaction>, failure::Error> {
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
        context.broadcast_transaction(TxTime::new(self.time.current_time()));
    }

    fn wire_api(&self, builder: &mut ServiceApiBuilder) {
        api::PublicApi::wire(builder);
        api::PrivateApi::wire(builder);
    }
}

/// A time service creator for the `NodeBuilder`.
#[derive(Debug)]
pub struct TimeServiceFactory;

impl ServiceFactory for TimeServiceFactory {
    fn service_name(&self) -> &str {
        SERVICE_NAME
    }

    fn make_service(&mut self, _: &Context) -> Box<dyn Service> {
        Box::new(TimeService::new())
    }
}
