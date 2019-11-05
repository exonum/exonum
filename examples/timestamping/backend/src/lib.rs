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
extern crate exonum_derive;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;

pub mod api;
pub mod proto;
pub mod schema;
pub mod transactions;

use exonum::{
    blockchain::ExecutionError,
    crypto::Hash,
    merkledb::{BinaryValue, Snapshot},
    runtime::{
        api::ServiceApiBuilder,
        rust::{CallContext, Service},
        BlockchainData, DispatcherError,
    },
};

use crate::{
    api::PublicApi as TimestampingApi,
    schema::{Schema, TimestampEntry},
    transactions::{Config, Error, TimestampingInterface},
};

#[derive(Debug, ServiceFactory)]
#[exonum(proto_sources = "proto", implements("TimestampingInterface"))]
pub struct TimestampingService;

impl Service for TimestampingService {
    fn initialize(&self, context: CallContext, params: Vec<u8>) -> Result<(), ExecutionError> {
        let config =
            Config::from_bytes(params.into()).map_err(DispatcherError::malformed_arguments)?;

        if context
            .data()
            .for_dispatcher()
            .get_instance(&*config.time_service_name)
            .is_none()
        {
            return Err(Error::TimeServiceNotFound.into());
        }

        Schema::ensure(context.service_data()).config.set(config);
        Ok(())
    }

    fn state_hash(&self, data: BlockchainData<&'_ dyn Snapshot>) -> Vec<Hash> {
        Schema::new(data.for_executing_service()).state_hash()
    }

    fn wire_api(&self, builder: &mut ServiceApiBuilder) {
        TimestampingApi.wire(builder);
    }
}
