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

//! Timestamping demo. This example shows how to use Exonum framework to create a fast
//! and secure service to prove the existence of a specific file at some moment
//! of time using blockchain as a secure database.

#![deny(
    missing_debug_implementations,
    // missing_docs,
    unsafe_code,
    bare_trait_objects
)]

#[macro_use]
extern crate serde_derive; // Required for Protobuf.

mod api;
#[doc(hidden)]
pub mod proto;
mod schema;
mod transactions;

pub use crate::{
    api::{TimestampProof, TimestampQuery},
    schema::{Timestamp, TimestampEntry},
    transactions::{Config, Error, TimestampingInterface},
};

use exonum::{
    merkledb::BinaryValue,
    runtime::{CommonError, ExecutionContext, ExecutionError},
};
use exonum_derive::{ServiceDispatcher, ServiceFactory};
use exonum_rust_runtime::{api::ServiceApiBuilder, Service};
use exonum_supervisor::Configure;
use exonum_time::TimeSchema;

use crate::{api::PublicApi as TimestampingApi, schema::Schema};

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_dispatcher(implements("TimestampingInterface", raw = "Configure<Params = Config>"))]
#[service_factory(proto_sources = "proto")]
pub struct TimestampingService;

fn verify_config(context: &ExecutionContext<'_>, config: &Config) -> Result<(), ExecutionError> {
    let _time_schema: TimeSchema<_> = context
        .data()
        .service_schema(config.time_service_name.as_str())?;

    Ok(())
}

impl Service for TimestampingService {
    fn initialize(
        &self,
        context: ExecutionContext<'_>,
        params: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        let config = Config::from_bytes(params.into()).map_err(CommonError::malformed_arguments)?;
        verify_config(&context, &config)?;

        Schema::new(context.service_data()).config.set(config);
        Ok(())
    }

    fn wire_api(&self, builder: &mut ServiceApiBuilder) {
        TimestampingApi.wire(builder);
    }
}

impl Configure for TimestampingService {
    type Params = Config;

    fn verify_config(
        &self,
        context: ExecutionContext<'_>,
        params: Self::Params,
    ) -> Result<(), ExecutionError> {
        verify_config(&context, &params)
    }

    fn apply_config(
        &self,
        context: ExecutionContext<'_>,
        params: Self::Params,
    ) -> Result<(), ExecutionError> {
        let mut schema = Schema::new(context.service_data());
        schema.config.set(params);
        Ok(())
    }
}
