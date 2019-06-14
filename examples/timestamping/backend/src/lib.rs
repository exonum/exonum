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
extern crate failure;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;

pub mod api;
pub mod proto;
pub mod schema;
pub mod transactions;

use exonum_merkledb::Snapshot;

use exonum::{
    api::ServiceApiBuilder,
    blockchain::ExecutionResult,
    crypto::Hash,
    impl_service_dispatcher,
    runtime::rust::{
        RustArtifactSpec, Service, ServiceDescriptor, ServiceFactory, TransactionContext,
    },
};
use exonum_time::schema::TimeSchema;

use crate::{
    api::PublicApi as TimestampingApi,
    schema::{Schema, TimestampEntry},
    transactions::{Error, TxTimestamp},
};

#[service_interface]
pub trait TimestampingInterface {
    fn timestamp(&self, ctx: TransactionContext, arg: TxTimestamp) -> ExecutionResult;
}

#[derive(Debug)]
pub struct TimestampingService;

impl TimestampingInterface for TimestampingService {
    fn timestamp(&self, context: TransactionContext, arg: TxTimestamp) -> ExecutionResult {
        let tx_hash = context.tx_hash();
        // TODO Add exonum time oracle name to service configuration parameters.
        let time = TimeSchema::new("exonum-time", context.fork())
            .time()
            .get()
            .expect("Can't get the time");

        let hash = &arg.content.content_hash;

        let schema = Schema::new(context.service_name(), context.fork());
        if let Some(_entry) = schema.timestamps().get(hash) {
            Err(Error::HashAlreadyExists)?;
        }

        trace!("Timestamp added: {:?}", arg);
        let entry = TimestampEntry::new(arg.content.clone(), &tx_hash, time);
        schema.add_timestamp(entry);

        Ok(())
    }
}

impl_service_dispatcher!(TimestampingService, TimestampingInterface);

impl Service for TimestampingService {
    fn wire_api(&self, descriptor: ServiceDescriptor, builder: &mut ServiceApiBuilder) {
        TimestampingApi::new(descriptor).wire(builder);
    }

    fn state_hash(&self, descriptor: ServiceDescriptor, snapshot: &dyn Snapshot) -> Vec<Hash> {
        let schema = Schema::new(descriptor.service_name(), snapshot);
        schema.state_hash()
    }
}

impl ServiceFactory for TimestampingService {
    fn artifact(&self) -> RustArtifactSpec {
        exonum::artifact_spec_from_crate!()
    }

    fn new_instance(&self) -> Box<dyn Service> {
        Box::new(TimestampingService)
    }
}
