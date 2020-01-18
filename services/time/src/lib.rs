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

//! The time oracle service for Exonum.
//!
//! See [the Exonum documentation][docs:time] for a high-level overview of the service,
//! in particular, its design rationale and the proof of correctness.
//!
//! [docs:time]: https://exonum.com/doc/version/latest/advanced/time

#![deny(
    unsafe_code,
    bare_trait_objects,
    missing_docs,
    missing_debug_implementations
)]

/// Node API.
mod api;
/// Protobuf generated structs.
mod proto;
/// Database schema.
mod schema;
/// System time provider.
mod time_provider;
/// Node transactions.
mod transactions;

use exonum_derive::{ServiceDispatcher, ServiceFactory};
use exonum_rust_runtime::{api::ServiceApiBuilder, AfterCommitContext, Service};

use std::sync::Arc;

pub use crate::{
    schema::TimeSchema,
    time_provider::{MockTimeProvider, SystemTimeProvider, TimeProvider},
    transactions::{TimeOracleInterface, TxTime},
};

/// Define the service.
#[derive(Debug, ServiceDispatcher)]
#[service_dispatcher(implements("TimeOracleInterface"))]
pub struct TimeService {
    /// Current time.
    time: Arc<dyn TimeProvider>,
}

impl Service for TimeService {
    /// Creates transaction after commit of the block.
    fn after_commit(&self, context: AfterCommitContext<'_>) {
        if let Some(broadcast) = context.broadcaster() {
            let time = TxTime::new(self.time.current_time());
            broadcast.report_time((), time).ok();
        }
    }

    fn wire_api(&self, builder: &mut ServiceApiBuilder) {
        api::PublicApi.wire(builder);
        api::PrivateApi.wire(builder);
    }
}

/// Time oracle service factory implementation.
#[derive(Debug, ServiceFactory)]
#[service_factory(
    proto_sources = "proto",
    service_constructor = "TimeServiceFactory::create_instance"
)]
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

    fn create_instance(&self) -> Box<dyn Service> {
        Box::new(TimeService {
            time: self.time_provider.clone(),
        })
    }
}

impl Default for TimeServiceFactory {
    fn default() -> Self {
        Self::with_provider(SystemTimeProvider)
    }
}
