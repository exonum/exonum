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
extern crate exonum;
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
    helpers::fabric,
    runtime::rust::{
        service::{GenesisInitInfo, Service, ServiceFactory},
        RustArtifactSpec, TransactionContext,
    },
};
use exonum_time::schema::TimeSchema;

use crate::{
    api::PublicApi as TimestampingApi,
    schema::{Schema, TimestampEntry},
    transactions::{Error, TxTimestamp},
};

const TIMESTAMPING_SERVICE: u16 = 130;
const SERVICE_NAME: &str = "timestamping";

#[service_interface]
pub trait Timestamping {
    fn timestamp(&self, ctx: TransactionContext, arg: TxTimestamp) -> ExecutionResult;
}

#[derive(Debug)]
pub struct TimestampingServiceImpl;

impl Timestamping for TimestampingServiceImpl {
    fn timestamp(&self, context: TransactionContext, arg: TxTimestamp) -> ExecutionResult {
        let tx_hash = context.tx_hash();
        let time = TimeSchema::new(context.fork())
            .time()
            .get()
            .expect("Can't get the time");

        let hash = &arg.content.content_hash;

        let schema = Schema::new(context.fork());
        if let Some(_entry) = schema.timestamps().get(hash) {
            Err(Error::HashAlreadyExists)?;
        }

        trace!("Timestamp added: {:?}", arg);
        let entry = TimestampEntry::new(arg.content.clone(), &tx_hash, time);
        schema.add_timestamp(entry);

        Ok(())
    }
}

impl_service_dispatcher!(TimestampingServiceImpl, Timestamping);

impl Service for TimestampingServiceImpl {
    fn wire_api(&self, builder: &mut ServiceApiBuilder) {
        TimestampingApi::wire(builder);
    }

    fn state_hash(&self, snapshot: &dyn Snapshot) -> Vec<Hash> {
        let schema = Schema::new(snapshot);
        schema.state_hash()
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
        Box::new(TimestampingServiceImpl)
    }

    fn genesis_init_info(&self) -> Vec<GenesisInitInfo> {
        vec![]
    }
}

/// A configuration service creator for the `NodeBuilder`.
#[derive(Debug)]
pub struct TimestampingServiceFactory;

impl fabric::ServiceFactory for TimestampingServiceFactory {
    fn make_service_builder(&self, _run_context: &fabric::Context) -> Box<dyn ServiceFactory> {
        Box::new(ServiceFactoryImpl)
    }
}
