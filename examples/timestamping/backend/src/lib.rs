// Copyright 2018 The Exonum Team
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

#![deny(missing_debug_implementations, unsafe_code, bare_trait_objects)]

extern crate chrono;
#[macro_use]
extern crate exonum;
extern crate exonum_time;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate log;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

pub mod api;
pub mod schema;
pub mod transactions;

use exonum::{
    api::{ServiceApiBuilder, ServiceWorkerContext},
    blockchain::{self, Transaction, TransactionSet}, crypto::Hash,
    encoding::Error as StreamStructError, helpers::fabric, messages::RawTransaction,
    storage::Snapshot,
};

use api::PublicApi;
use schema::Schema;
use transactions::TimeTransactions;

const TIMESTAMPING_SERVICE: u16 = 130;
pub const SERVICE_NAME: &str = "timestamping";

#[derive(Debug, Default)]
pub struct Service;

impl Service {
    pub fn new() -> Self {
        Service
    }
}

impl blockchain::Service for Service {
    fn service_id(&self) -> u16 {
        TIMESTAMPING_SERVICE
    }

    fn service_name(&self) -> &'static str {
        SERVICE_NAME
    }

    fn state_hash(&self, view: &dyn Snapshot) -> Vec<Hash> {
        let schema = Schema::new(view);
        schema.state_hash()
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<dyn Transaction>, StreamStructError> {
        let tx = TimeTransactions::tx_from_raw(raw)?;
        Ok(tx.into())
    }

    fn wire_api(&self, builder: &mut ServiceApiBuilder) {
        PublicApi::wire(builder);
        builder.additional_worker(|context: ServiceWorkerContext| {
            debug!("Service worker started");
            while context.is_running() {
                ::std::thread::sleep(::std::time::Duration::from_secs(5));
                debug!("Long operation finished");
            }
            debug!("Service worker stopped");
            Ok(())
        });
    }
}

/// A configuration service creator for the `NodeBuilder`.
#[derive(Debug, Clone, Copy)]
pub struct ServiceFactory;

impl fabric::ServiceFactory for ServiceFactory {
    fn service_name(&self) -> &str {
        SERVICE_NAME
    }

    fn make_service(&mut self, _: &fabric::Context) -> Box<dyn blockchain::Service> {
        Box::new(Service::new())
    }
}
