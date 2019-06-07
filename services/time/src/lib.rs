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
    blockchain::ExecutionResult,
    crypto::Hash,
    helpers::fabric::Context,
    impl_service_dispatcher,
    runtime::rust::{
        AfterCommitContext, RustArtifactSpec, Service, ServiceDescriptor, ServiceFactory,
        ServiceInstanceId, TransactionContext,
    },
};
use exonum_merkledb::Snapshot;

use crate::{
    schema::TimeSchema,
    time_provider::{SystemTimeProvider, TimeProvider},
    transactions::TxTime,
};

#[service_interface]
pub trait TimeOracleInterface {
    fn time(&self, ctx: TransactionContext, arg: TxTime) -> ExecutionResult;
}

/// Define the service.
#[derive(Debug)]
pub struct TimeService {
    /// Current time.
    time: Box<dyn TimeProvider>,
}

impl Default for TimeService {
    fn default() -> Self {
        Self {
            time: Box::new(SystemTimeProvider) as Box<dyn TimeProvider>,
        }
    }
}

impl TimeService {
    /// Create a new `TimeService`.
    pub fn new() -> Self {
        Self::default()
    }

    // TODO there is no way to provide provider for now.
    // It should be configurable through the configuration service.
}

impl TimeOracleInterface for TimeService {
    fn time(&self, context: TransactionContext, arg: TxTime) -> ExecutionResult {
        let author = context.author();
        let view = context.fork();
        let service_name = context.service_name();

        arg.check_signed_by_validator(view.as_ref(), &author)?;
        arg.update_validator_time(service_name, view, &author)?;
        TxTime::update_consolidated_time(service_name, view);
        Ok(())
    }
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

        // TODO can't implement after_commit via fork
        unimplemented!();
        // if context.validator_id().is_none() {
        //     return;
        // }
        // context.broadcast_transaction(TxTime::new(self.time.current_time()));
    }
}

#[derive(Debug)]
pub struct TimeServiceFactory;

impl ServiceFactory for TimeServiceFactory {
    fn artifact(&self) -> RustArtifactSpec {
        RustArtifactSpec::new("exonum-time", 0, 1, 0)
    }

    fn new_instance(&self) -> Box<dyn Service> {
        Box::new(TimeService::new())
    }
}
