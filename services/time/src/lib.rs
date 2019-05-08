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
    // missing_docs,
    unsafe_code,
    bare_trait_objects
)]

#[macro_use]
extern crate failure;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate exonum_derive;
#[macro_use]
extern crate exonum;

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
    blockchain::{ServiceContext, Transaction, TransactionSet, ExecutionResult, ExecutionError},
    crypto::Hash,
    helpers::fabric::{self, Context},
    runtime::rust::{
        service::{GenesisInitInfo, Service, ServiceFactory},
        RustArtifactSpec, TransactionContext,
    },
    messages::AnyTx,
};
use serde_json::Value;

use crate::{
    schema::TimeSchema,
    time_provider::{SystemTimeProvider, TimeProvider},
    transactions::TxTime,
};

/// Time service id.
pub const SERVICE_ID: u16 = 4;
/// Time service name.
pub const SERVICE_NAME: &str = "exonum_time";


#[service_interface]
pub trait Time {
    fn time(&self, ctx: TransactionContext, arg: TxTime) -> ExecutionResult;
}

/// Define the service.
#[derive(Debug)]
pub struct TimeServiceImpl {
    /// Current time.
    time: Box<dyn TimeProvider>,
}

impl Default for TimeServiceImpl {
    fn default() -> TimeServiceImpl {
        TimeServiceImpl {
            time: Box::new(SystemTimeProvider) as Box<dyn TimeProvider>,
        }
    }
}

impl TimeServiceImpl {
    /// Create a new `TimeService`.
    pub fn new() -> TimeServiceImpl {
        TimeServiceImpl::default()
    }

    // TODO there is no way to provide provider for now.
    // It should be configurable through the configuration service.
}

impl Time for TimeServiceImpl {
    fn time(
        &self,
        context: TransactionContext,
        arg: TxTime,
    ) -> ExecutionResult {
        let author = context.author();
        let view = context.fork();
        arg.check_signed_by_validator(view.as_ref(), &author)?;
        arg.update_validator_time(view, &author)?;
        TxTime::update_consolidated_time(view);
        Ok(())
    }
}

impl_service_dispatcher!(TimeServiceImpl, Time);

impl Service for TimeServiceImpl {
    fn wire_api(&self, builder: &mut ServiceApiBuilder) {
        api::PublicApi::wire(builder);
        api::PrivateApi::wire(builder);
    }
    
    fn state_hash(&self, snapshot: &dyn Snapshot) -> Vec<Hash> {
        let schema = TimeSchema::new(snapshot);
        schema.state_hash()
    }

    /// Creates transaction after commit of the block.
    fn after_commit(&self, fork: &mut Fork) {
        // The transaction must be created by the validator.

        // TODO can't implement after_commit via fork
        unimplemented!();
        // if context.validator_id().is_none() {
        //     return;
        // }
        // context.broadcast_transaction(TxTime::new(self.time.current_time()));
    }
}

pub fn artifact_spec() -> RustArtifactSpec {
    RustArtifactSpec::new(SERVICE_NAME, 0, 1, 0)
}

#[derive(Debug)]
pub struct ServiceFactoryImpl;

impl ServiceFactory for ServiceFactoryImpl {
    fn artifact(&self) -> RustArtifactSpec {
        artifact_spec()
    }

    fn new_instance(&self) -> Box<dyn Service> {
        Box::new(TimeServiceImpl::new())
    }

    fn genesis_init_info(&self) -> Vec<GenesisInitInfo> {
        vec![]
    }
}

/// A configuration service creator for the `NodeBuilder`.
#[derive(Debug)]
pub struct TimeServiceFactory;

impl fabric::ServiceFactory for TimeServiceFactory {
    fn make_service_builder(&self, _run_context: &Context) -> Box<dyn ServiceFactory> {
        Box::new(ServiceFactoryImpl)
    }
}
