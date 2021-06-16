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

pub use crate::{config::Config, schema::Schema, transactions::CryptocurrencyInterface};

pub mod api;
pub mod config;
pub mod migrations;
pub mod proto;
pub mod schema;
pub mod transactions;
pub mod wallet;

use exonum::{
    merkledb::BinaryValue,
    runtime::{CommonError, ExecutionContext, ExecutionError, InstanceId},
};
use exonum_derive::{ServiceDispatcher, ServiceFactory};
use exonum_rust_runtime::{api::ServiceApiBuilder, Service};
use exonum_supervisor::Configure;

use crate::{api::PublicApi as CryptocurrencyApi, schema::SchemaImpl};

/// Cryptocurrency service ID.
pub const INSTANCE_ID: InstanceId = 3;
/// Cryptocurrency service instance name.
pub const INSTANCE_NAME: &str = "crypto";

/// Cryptocurrency service implementation.
#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_dispatcher(implements("CryptocurrencyInterface", raw = "Configure<Params = Config>"))]
#[service_factory(artifact_name = "exonum-cryptocurrency", proto_sources = "proto")]
pub struct CryptocurrencyService;

impl Configure for CryptocurrencyService {
    type Params = Config;

    fn verify_config(
        &self,
        _context: ExecutionContext<'_>,
        params: Self::Params,
    ) -> Result<(), ExecutionError> {
        params.verify().map_err(Into::into)
    }

    fn apply_config(
        &self,
        context: ExecutionContext<'_>,
        params: Self::Params,
    ) -> Result<(), ExecutionError> {
        let mut schema = SchemaImpl::new(context.service_data());
        schema.config.set(params);
        Ok(())
    }
}

impl Service for CryptocurrencyService {
    fn initialize(
        &self,
        context: ExecutionContext<'_>,
        params: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        let config = Config::from_bytes(params.into()).map_err(CommonError::malformed_arguments)?;
        config.verify()?;

        let mut schema = SchemaImpl::new(context.service_data());
        schema.config.set(config);

        Ok(())
    }

    fn wire_api(&self, builder: &mut ServiceApiBuilder) {
        CryptocurrencyApi::wire(builder);
    }
}
