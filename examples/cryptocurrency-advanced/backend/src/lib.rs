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

//! Cryptocurrency implementation example using [exonum](http://exonum.com/).

#![deny(unsafe_code, bare_trait_objects)]
#![warn(missing_docs, missing_debug_implementations)]

#[macro_use]
extern crate serde_derive; // Required for Protobuf.

pub use crate::{schema::Schema, transactions::CryptocurrencyInterface};

pub mod api;
pub mod proto;
pub mod schema;
pub mod transactions;
pub mod wallet;

use exonum::runtime::{ExecutionContext, ExecutionError, InstanceId};
use exonum_derive::{ServiceDispatcher, ServiceFactory};
use exonum_rust_runtime::{api::ServiceApiBuilder, DefaultInstance, Service};

use crate::{api::PublicApi as CryptocurrencyApi, schema::SchemaImpl};

/// Initial balance of the wallet.
pub const INITIAL_BALANCE: u64 = 100;

/// Cryptocurrency service implementation.
#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_dispatcher(implements("CryptocurrencyInterface"))]
#[service_factory(proto_sources = "proto")]
pub struct CryptocurrencyService;

impl Service for CryptocurrencyService {
    fn initialize(
        &self,
        context: ExecutionContext<'_>,
        _params: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        // Initialize indexes. Not doing this may lead to errors in HTTP API, since it relies on
        // `wallets` indexes being initialized for returning corresponding proofs.
        SchemaImpl::new(context.service_data());
        Ok(())
    }

    fn wire_api(&self, builder: &mut ServiceApiBuilder) {
        CryptocurrencyApi::wire(builder);
    }
}

/// Use predefined instance name and id for frontend.
impl DefaultInstance for CryptocurrencyService {
    const INSTANCE_ID: InstanceId = 3;
    const INSTANCE_NAME: &'static str = "crypto";
}
