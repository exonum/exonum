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

//! This module implements a *configuration service* for Exonum blockchain framework.
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
//! 3. Use `crate::crypto::hash()` on the obtained bytes.
//!
//! [sc]: https://docs.rs/exonum/0.10.3/exonum/blockchain/config/struct.StoredConfiguration.html
//! [docs:config]: https://exonum.com/doc/advanced/configuration-updater/
//!
//! # Examples
//!
//! ```rust,no_run
//! extern crate exonum;
//!
//! use exonum::helpers::fabric::NodeBuilder;
//!
//! fn main() {
//!     exonum::helpers::init_logger().unwrap();
//!     NodeBuilder::new()
//!         .run();
//! }
//! ```

pub use errors::ErrorCode;
pub use schema::{MaybeVote, ProposeData, Schema, VotingDecision};
pub use transactions::{ConfigurationTransactions, Propose, Vote, VoteAgainst};

use serde_json::{to_value, Value};

use crate::{
    api::ServiceApiBuilder,
    blockchain::{self, Transaction, TransactionSet},
    crypto::Hash,
    helpers::fabric::{self, keys, Command, CommandExtension, CommandName, Context},
    messages::RawTransaction,
    node::State,
    storage::{Fork, Snapshot},
};

use cmd::{Finalize, GenerateCommonConfig};
use config::ConfigurationServiceConfig;

pub mod api; // TODO: pub only for testing.
mod cmd;
pub mod config; // TODO: pub only for testing.
pub mod errors; // TODO: pub only for testing.
pub mod schema; // TODO: pub only for testing.
pub mod transactions; // TODO: pub only for testing.

/// Service identifier for the configuration service.
pub const SERVICE_ID: u16 = 1;
/// Configuration service name.
pub const SERVICE_NAME: &str = "configuration";

/// ConfigurationService config.
#[derive(Debug, Default)]
pub struct Service {
    config: ConfigurationServiceConfig,
}

impl Service {
    /// Create new instance of configuration service.
    pub fn new(config: ConfigurationServiceConfig) -> Self {
        Self { config }
    }
}

impl blockchain::Service for Service {
    fn service_id(&self) -> u16 {
        SERVICE_ID
    }

    fn service_name(&self) -> &'static str {
        SERVICE_NAME
    }

    fn state_hash(&self, snapshot: &dyn Snapshot) -> Vec<Hash> {
        let schema = Schema::new(snapshot);
        schema.state_hash()
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<dyn Transaction>, failure::Error> {
        ConfigurationTransactions::tx_from_raw(raw).map(Into::into)
    }

    fn initialize(&self, _fork: &mut Fork) -> Value {
        to_value(self.config.clone()).unwrap()
    }

    fn wire_api(&self, builder: &mut ServiceApiBuilder) {
        api::PublicApi::wire(builder);
        api::PrivateApi::wire(builder);
    }
}

/// A configuration service creator for the `NodeBuilder`.
#[derive(Debug)]
pub struct ServiceFactory;

impl fabric::ServiceFactory for ServiceFactory {
    fn service_name(&self) -> &str {
        SERVICE_NAME
    }

    fn command(&mut self, command: CommandName) -> Option<Box<dyn CommandExtension>> {
        use crate::helpers::fabric;
        Some(match command {
            v if v == fabric::GenerateCommonConfig.name() => Box::new(GenerateCommonConfig),
            v if v == fabric::Finalize.name() => Box::new(Finalize),
            _ => return None,
        })
    }

    fn make_service(&mut self, context: &Context) -> Box<dyn blockchain::Service> {
        let service_config: ConfigurationServiceConfig =
            context.get(keys::NODE_CONFIG).unwrap().services_configs["configuration_service"]
                .clone()
                .try_into()
                .unwrap();

        if let Some(majority_count) = service_config.majority_count {
            let validators_count = context
                .get(keys::NODE_CONFIG)
                .unwrap()
                .genesis
                .validator_keys
                .len() as u16;
            let byzantine_majority_count =
                State::byzantine_majority_count(validators_count as usize) as u16;
            if majority_count > validators_count || majority_count < byzantine_majority_count {
                panic!(
                    "Invalid majority count: {}, it should be >= {} and <= {}",
                    majority_count, byzantine_majority_count, validators_count
                );
            }
        }

        Box::new(Service {
            config: service_config,
        })
    }
}
