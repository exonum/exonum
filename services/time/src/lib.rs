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
//! Time service is designed to be a trusted source of actual time for the
//! Exonum blockchain services.
//! With this service, user services are able to access the calendar time
//! supplied by validator nodes to the blockchain.
//!
//! # Basics of the Approach
//!
//! Each validator at a specific time sends a transaction indicating its local
//! time (usually immediately after the commit of each block). The time service
//! maintains an index with the most current time values indicated separately by
//! each validator. This index is updated after each transaction from any of the
//! validators. A 1/3 quantile of these values (ordered by decreasing time) is
//! then picked out from the index; this quantile is considered the actual time
//! and is applied to determine the exact service time.
//!
//! See [the Exonum documentation][docs:time] for a high-level overview of the
//! service, in particular, its design rationale and the proof of correctness.
//!
//! [docs:time]: https://exonum.com/doc/version/latest/advanced/time
//!
//! # HTTP API
//!
//! REST API of the service is documented in the [`api` module](api/index.html).
//!
//! # Examples
//!
//! ## Use with TestKit
//!
//! ```
//! use chrono::{TimeZone, Utc};
//! use exonum::{
//!     helpers::Height,
//!     runtime::{InstanceId, SnapshotExt},
//! };
//! use exonum_rust_runtime::ServiceFactory;
//! use exonum_testkit::{TestKit, TestKitBuilder};
//! use exonum_time::{MockTimeProvider, TimeProvider, TimeSchema, TimeServiceFactory};
//!
//! use std::sync::Arc;
//!
//! const TIME_SERVICE_ID: InstanceId = 100;
//! const TIME_SERVICE_NAME: &'static str = "time-oracle";
//!
//! // Time provider for a time service is chosen at the compile time.
//! // Here we use `MockTimeProvider` to get controllable results.
//! // In real world it makes sense to use `SystemTimeProvider` or your
//! // own implementation of `TimeProvider`.
//! let time_provider = Arc::new(MockTimeProvider::default());
//!
//! // Factory for time service will create instances of the service with given
//! // time provider.
//! let time_service_factory =
//!     TimeServiceFactory::with_provider(time_provider.clone() as Arc<dyn TimeProvider>);
//! let time_service_artifact = time_service_factory.artifact_id();
//!
//! // Create testkit with the time service.
//! let mut testkit: TestKit = TestKitBuilder::validator()
//!     .with_artifact(time_service_artifact.clone())
//!     .with_instance(
//!         time_service_artifact.into_default_instance(TIME_SERVICE_ID, TIME_SERVICE_NAME),
//!     )
//!     .with_rust_service(time_service_factory)
//!     // Add other services here
//!     .build();
//!
//! // Set time in `MockTimeProvider`.
//! time_provider.set_time(Utc.timestamp(10, 0));
//!
//! // Create some blocks for time to appear in the blockchain.
//! testkit.create_blocks_until(Height(2));
//!
//! // Obtain time service schema.
//! let snapshot = testkit.snapshot();
//! let time_schema: TimeSchema<_> = snapshot.service_schema(TIME_SERVICE_NAME).unwrap();
//!
//! // Obtain time from the schema. Service can base its logic on this time.
//! let time = time_schema.time.get();
//!
//! // With `MockServiceProvider` we can ensure that time is based on data
//! // provided by `TimeProvider`.
//! assert_eq!(time, Some(time_provider.time()));
//! ```
//!
//! ## Interaction with other service
//!
//! Example of interaction with the time service can be found [at GitHub].
//!
//! [at GitHub]: https://github.com/exonum/exonum/blob/master/services/time/examples/simple_service/main.rs

#![deny(
    unsafe_code,
    bare_trait_objects,
    missing_docs,
    missing_debug_implementations
)]

pub mod api;

mod proto;
mod schema;
mod time_provider;
mod transactions;

use exonum_derive::{ServiceDispatcher, ServiceFactory};
use exonum_rust_runtime::{api::ServiceApiBuilder, AfterCommitContext, Service};

use std::sync::Arc;

pub use crate::{
    api::ValidatorTime,
    schema::TimeSchema,
    time_provider::{MockTimeProvider, SystemTimeProvider, TimeProvider},
    transactions::{Error, TimeOracleInterface, TimeOracleInterfaceMut, TxTime},
};

/// Time oracle service.
#[derive(Debug, ServiceDispatcher)]
#[service_dispatcher(implements("TimeOracleInterface"))]
pub struct TimeService {
    /// Provider of the actual time.
    time: Arc<dyn TimeProvider>,
}

impl Service for TimeService {
    fn after_commit(&self, context: AfterCommitContext<'_>) {
        // If current node is a validator, after each block it should broadcast
        // the transaction with the current time.
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

/// Time oracle service factory.
///
/// By default, it creates time service instances with [`SystemTimeProvider`].
///
/// [`SystemTimeProvider`]: struct.SystemTimeProvider.html
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
    ///
    /// One can implement a custom time provider by implementing
    /// [`TimeProvider`] trait.
    ///
    /// [`TimeProvider`]: trait.TimeProvider.html
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
