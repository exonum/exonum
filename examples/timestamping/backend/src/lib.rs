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

extern crate bodyparser;
extern crate chrono;
#[macro_use]
extern crate exonum;
extern crate exonum_time;
#[macro_use]
extern crate failure;
extern crate iron;
#[macro_use]
extern crate log;
extern crate router;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;

pub mod api;
pub mod schema;
pub mod transactions;

use exonum::api::Api;
use exonum::blockchain::{self, ApiContext, Transaction, TransactionSet};
use exonum::crypto::Hash;
use exonum::encoding::Error as StreamStructError;
use exonum::helpers::fabric;
use exonum::messages::RawTransaction;
use exonum::storage::Snapshot;

use iron::Handler;
use router::Router;

use api::PublicApi;
use schema::Schema;
use transactions::TimeTransactions;

const TIMESTAMPING_SERVICE: u16 = 130;

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
        "timestamping"
    }

    fn state_hash(&self, view: &Snapshot) -> Vec<Hash> {
        let schema = Schema::new(view);
        schema.state_hash()
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, StreamStructError> {
        let tx = TimeTransactions::tx_from_raw(raw)?;
        Ok(tx.into())
    }

    fn public_api_handler(&self, context: &ApiContext) -> Option<Box<Handler>> {
        let mut router = Router::new();
        let api = PublicApi::new(context.blockchain().clone(), context.node_channel().clone());
        api.wire(&mut router);
        Some(Box::new(router))
    }
}

/// A configuration service creator for the `NodeBuilder`.
#[derive(Debug)]
pub struct ServiceFactory;

impl fabric::ServiceFactory for ServiceFactory {
    fn make_service(&mut self, _: &fabric::Context) -> Box<blockchain::Service> {
        Box::new(Service::new())
    }
}
