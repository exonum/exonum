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

//! # Introduction
//! This crate implements the standalone configuration service of `Exonum` blockchain,
//! which, upon being plugged in, allows modifying
//! `Exonum` blockchain configuration by means of [propose config](struct.Propose.html)
//! and [vote for proposed config](struct.Vote.html) transactions, signed by validators
//! - actual blockchain participants.
//!
//! It also contains http api implementation for public queries (get actual/following
//! configuration, etc.) and private queries, intended for use only by validator nodes' maintainers
//! (post configuration propose, post vote for a configuration propose).
//!
//! `Exonum` blockchain configuration is composed of:
//!
//! - consensus algorithm parameters
//! - list of validators' public keys - list of identities of consensus participants
//! - list of services public keys
//! - configuration of all services, plugged in for a specific blockchain instance.
//!
//! It also contains auxiliary fields:
//!
//! - `actual_from` - blockchain height, upon reaching which current config is to become actual.
//! - `previous_cfg_hash` - hash of previous configuration, which validators' set is allowed to cast
//! votes for current config.
//!
//! See [`StoredConfiguration`][sc] in exonum.
//!
//! [sc]: https://docs.rs/exonum/0.3.0/exonum/blockchain/config/struct.StoredConfiguration.html
//!
//! While using the service's transactions and/or api, it's important to understand, how [hash of a
//! configuration][sc] is calculated. It's calculated as a hash of normalized `String` bytes,
//! containing configuration json representation.
//! When a new propose is put via `Propose`:
//!
//! [sc]: https://docs.rs/exonum/0.3.0/exonum/blockchain/config/struct.StoredConfiguration.html
//!
//! 1. [bytes](struct.Propose.html#method.cfg) of a `String`, containing configuration
//! json ->
//! 2. `String` ->
//! 3. `StoredConfiguration` ->
//! 4. unique normalized `String` for a unique configuration ->
//! 5. bytes ->
//! 6. [hash](https://docs.rs/exonum/0.3.0/exonum/crypto/fn.hash.html)(bytes)
//!
//! The same hash of a configuration is referenced in
//! `Vote` in [`cfg_hash`](struct.Vote.html#method.cfg_hash).
//!
//!
//! # Examples
//!
//! Run `Exonum` blockchain testnet with single configuration service turned on for it in a
//! single process (2 threads per node: 1 - for exonum node and 1 - for http api listener)
//!
//! ```rust,no_run
//! extern crate exonum;
//! extern crate exonum_configuration;
//!
//! use exonum::helpers::fabric::NodeBuilder;
//!
//! use exonum_configuration::ConfigurationServiceFactory;
//!
//! fn main() {
//!     exonum::helpers::init_logger().unwrap();
//!     NodeBuilder::new()
//!         .with_service(Box::new(ConfigurationServiceFactory))
//!         .run();
//! }
//! ```

// spell-checker:ignore ZEROVOTE

#[macro_use]
extern crate exonum;
#[cfg(test)]
#[macro_use]
extern crate exonum_testkit;
#[cfg(test)]
#[macro_use]
extern crate pretty_assertions;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;
extern crate iron;
extern crate router;
extern crate bodyparser;
extern crate params;
#[macro_use]
extern crate lazy_static;

use router::Router;
use iron::Handler;
use exonum::api::Api;
use exonum::blockchain::{Service, Transaction, ApiContext};
use exonum::helpers::fabric::{ServiceFactory, Context};
use exonum::crypto::Hash;
use exonum::messages::RawTransaction;
use exonum::storage::Snapshot;
use exonum::encoding::{Error as EncodingError};

pub mod api;
pub mod schema;
pub mod transactions;

#[cfg(test)]
mod tests;

pub use schema::{ConfigurationSchema, ProposeData};
pub use transactions::{Propose, Vote, ZEROVOTE};

/// Value of [`service_id`](struct.Service.html#method.service_id) of
/// `ConfigurationService`.
pub const CONFIGURATION_SERVICE_ID: u16 = 1;

/// Structure, implementing [Service][1] trait template.
/// Most of the actual business logic of modifying `Exonum` blockchain configuration is inside of
/// [`Propose`](struct.Propose.html#method.execute) and
/// [`Vote`](struct.Vote.html#method.execute).
/// [1]: <https://docs.rs/exonum/0.3.0/exonum/blockchain/trait.Service.html>
#[derive(Default)]
pub struct ConfigurationService {}

impl ConfigurationService {
    pub fn new() -> ConfigurationService {
        ConfigurationService {}
    }
}

impl Service for ConfigurationService {
    fn service_name(&self) -> &'static str {
        "configuration"
    }

    fn service_id(&self) -> u16 {
        CONFIGURATION_SERVICE_ID
    }

    /// `ConfigurationService` returns a vector, containing the single [root_hash][1]
    /// of [all config proposes table]
    /// (struct.ConfigurationSchema.html#method.propose_data_by_config_hash).
    /// [1]: <https://docs.rs/exonum/0.3.0/exonum/storage/proof_list_index/
    ///struct.ProofListIndex.html#method.root_hash>
    ///
    /// Thus, `state_hash` is affected by any new valid propose and indirectly by
    /// any new vote for a propose.
    ///
    /// When a new vote for a config propose is added the [root_hash][1]
    ///  of corresponding
    /// [votes for a propose table](struct.ConfigurationSchema.html#method.votes_by_config_hash)
    /// is modified. Such hash is stored in each entry of [all config proposes table]
    /// (struct.ConfigurationSchema.html#method.propose_data_by_config_hash)
    /// - `ProposeData`.
    /// [1]: <https://docs.rs/exonum/0.3.0/exonum/storage/proof_map_index/
    ///struct.ProofMapIndex.html#method.root_hash>
    fn state_hash(&self, snapshot: &Snapshot) -> Vec<Hash> {
        let schema = ConfigurationSchema::new(snapshot);
        schema.state_hash()
    }

    /// Returns box ([Transaction][1]).
    /// [1]: https://docs.rs/exonum/0.3.0/exonum/blockchain/trait.Transaction.html
    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, EncodingError> {
        transactions::tx_from_raw(raw)
    }

    fn public_api_handler(&self, ctx: &ApiContext) -> Option<Box<Handler>> {
        let mut router = Router::new();
        let api = api::PublicConfigApi { blockchain: ctx.blockchain().clone() };
        api.wire(&mut router);
        Some(Box::new(router))
    }

    fn private_api_handler(&self, ctx: &ApiContext) -> Option<Box<Handler>> {
        let mut router = Router::new();
        let api = api::PrivateConfigApi {
            channel: ctx.node_channel().clone(),
            config: (*ctx.public_key(), ctx.secret_key().clone()),
        };
        api.wire(&mut router);
        Some(Box::new(router))
    }
}

/// A configuration service creator for the `NodeBuilder`.
#[derive(Debug)]
pub struct ConfigurationServiceFactory;

impl ServiceFactory for ConfigurationServiceFactory {
    fn make_service(&mut self, _: &Context) -> Box<Service> {
        Box::new(ConfigurationService::new())
    }
}
