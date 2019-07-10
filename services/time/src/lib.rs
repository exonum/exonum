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

#![deny(unsafe_code, bare_trait_objects)]
#![warn(missing_docs, missing_debug_implementations)]

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

use exonum::{
    api::ServiceApiBuilder,
    crypto::Hash,
    impl_service_dispatcher,
    runtime::rust::{
        AfterCommitContext, RustArtifactId, Service, ServiceDescriptor, ServiceFactory,
    },
};
use exonum_merkledb::Snapshot;

use std::sync::Arc;

use crate::{
    schema::TimeSchema,
    time_provider::{SystemTimeProvider, TimeProvider},
    transactions::{TimeOracleInterface, TxTime},
};

// TODO there is no way to provide provider for now.
// It should be configurable through the configuration service.

/// Define the service.
#[derive(Debug)]
pub struct TimeService {
    /// Current time.
    time: Arc<dyn TimeProvider>,
}

impl_service_dispatcher!(TimeService, TimeOracleInterface);

impl Service for TimeService {
    fn wire_api(&self, descriptor: ServiceDescriptor, builder: &mut ServiceApiBuilder) {
        let name = descriptor.service_name();
        api::PublicApi::new(name).wire(builder);
        api::PrivateApi::new(name).wire(builder);
    }

    fn state_hash(&self, descriptor: ServiceDescriptor, snapshot: &dyn Snapshot) -> Vec<Hash> {
        let schema = TimeSchema::new(descriptor.service_name(), snapshot);
        schema.state_hash()
    }

    /// Creates transaction after commit of the block.
    fn after_commit(&self, context: AfterCommitContext) {
        // The transaction must be created by the validator.
        if context.validator_id().is_some() {
            context.broadcast_transaction(TxTime::new(self.time.current_time()));
        }
    }
}

/// Time oracle service factory implementation.
#[derive(Debug)]
pub struct TimeServiceFactory {
    time_provider: Arc<dyn TimeProvider>,
}

impl TimeServiceFactory {
    /// Create a new `TimeServiceFactory` with the custom time provider.
    pub fn with_provider(time_provider: impl Into<Arc<dyn TimeProvider>>) -> Self {
        Self {
            time_provider: time_provider.into(),
        }
    }
}

impl Default for TimeServiceFactory {
    fn default() -> Self {
        Self::with_provider(SystemTimeProvider)
    }
}

impl ServiceFactory for TimeServiceFactory {
    fn artifact_id(&self) -> RustArtifactId {
        exonum::artifact_spec_from_crate!()
    }

    fn create_instance(&self) -> Box<dyn Service> {
        Box::new(TimeService {
            time: self.time_provider.clone(),
        })
    }
}
