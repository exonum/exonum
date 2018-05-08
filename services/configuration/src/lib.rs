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

//! This crate implements a *configuration service* for Exonum blockchain framework.
//!
//! Upon being plugged in, the service allows to modify Exonum blockchain configuration
//! using [proposals](struct.Propose.html) and [voting for proposal](struct.Vote.html),
//! both of which are implemented as transactions signed by blockchain validators.
//!
//! The service also provides HTTP API for public queries (get actual/following
//! configuration, etc.) and private queries, intended for use only by validator nodes' maintainers
//! (post configuration propose, post vote for a configuration propose).
//!
//! See [Exonum documentation][docs:config] for more details about the service.
//!
//! # Blockchain configuration
//!
//! Blockchain configuration corresponds to [`StoredConfiguration`][sc]
//! in the Exonum core library. The logic of the configuration service extensively uses
//! hashes of configuration, which are calculated as follows:
//!
//! 1. Parse a `StoredConfiguration` from JSON string if necessary.
//! 2. Convert a `StoredConfiguration` into bytes as per its `StorageValue` implementation.
//! 3. Use `exonum::crypto::hash()` on the obtained bytes.
//!
//! [sc]: https://docs.rs/exonum/0.5.1/exonum/blockchain/config/struct.StoredConfiguration.html
//! [docs:config]: https://exonum.com/doc/advanced/configuration-updater/
//!
//! # Examples
//!
//! ```rust,no_run
//! extern crate exonum;
//! extern crate exonum_configuration as configuration;
//!
//! use exonum::helpers::fabric::NodeBuilder;
//!
//! fn main() {
//!     exonum::helpers::init_logger().unwrap();
//!     NodeBuilder::new()
//!         .with_service(Box::new(configuration::ServiceFactory))
//!         .run();
//! }
//! ```

#![deny(missing_debug_implementations, missing_docs)]

extern crate bodyparser;
#[macro_use]
extern crate exonum;
#[macro_use]
extern crate failure;
extern crate iron;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate params;
extern crate router;
#[macro_use]
extern crate serde_derive;

#[cfg(test)]
#[macro_use]
extern crate assert_matches;
#[cfg(test)]
#[macro_use]
extern crate exonum_testkit;
#[cfg(test)]
#[macro_use]
extern crate pretty_assertions;

pub use errors::ErrorCode;
pub use schema::{MaybeVote, ProposeData, Schema, VotingDecision};
pub use transactions::{ConfigurationTransactions, Propose, Vote, VoteAgainst};

use exonum::api::Api;
use exonum::blockchain::{self, ApiContext, Transaction, TransactionSet};
use exonum::crypto::Hash;
use exonum::encoding::Error as EncodingError;
use exonum::helpers::fabric::{self, Context};
use exonum::messages::RawTransaction;
use exonum::storage::Snapshot;
use iron::Handler;
use router::Router;

mod api;
mod errors;
mod schema;
#[cfg(test)]
mod tests;
mod transactions;

/// Service identifier for the configuration service.
pub const SERVICE_ID: u16 = 1;

/// Configuration service.
#[derive(Debug, Default)]
pub struct Service {}

impl blockchain::Service for Service {
    fn service_name(&self) -> &'static str {
        "configuration"
    }

    fn service_id(&self) -> u16 {
        SERVICE_ID
    }

    fn state_hash(&self, snapshot: &Snapshot) -> Vec<Hash> {
        let schema = Schema::new(snapshot);
        schema.state_hash()
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, EncodingError> {
        ConfigurationTransactions::tx_from_raw(raw).map(Into::into)
    }

    fn public_api_handler(&self, ctx: &ApiContext) -> Option<Box<Handler>> {
        let mut router = Router::new();
        let api = api::PublicApi::new(ctx);
        api.wire(&mut router);
        Some(Box::new(router))
    }

    fn private_api_handler(&self, ctx: &ApiContext) -> Option<Box<Handler>> {
        let mut router = Router::new();
        let api = api::PrivateApi::new(ctx);
        api.wire(&mut router);
        Some(Box::new(router))
    }
}

/// A configuration service creator for the `NodeBuilder`.
#[derive(Debug)]
pub struct ServiceFactory;

impl fabric::ServiceFactory for ServiceFactory {
    fn make_service(&mut self, _: &Context) -> Box<blockchain::Service> {
        Box::new(Service {})
    }
}
