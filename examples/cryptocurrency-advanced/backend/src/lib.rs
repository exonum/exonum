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

//! Cryptocurrency implementation example using [exonum](http://exonum.com/).

#![deny(unsafe_code, bare_trait_objects)]
#![warn(missing_docs, missing_debug_implementations)]

#[macro_use]
extern crate exonum_derive;
#[macro_use]
extern crate serde_derive;

pub use crate::schema::Schema;

pub mod api;
pub mod proto;
pub mod schema;
pub mod transactions;
pub mod wallet;

use exonum::{
    api::ServiceApiBuilder,
    runtime::rust::{Service, ServiceDescriptor},
};

use crate::api::PublicApi as CryptocurrencyApi;

/// Initial balance of the wallet.
pub const INITIAL_BALANCE: u64 = 100;

/// Cryptocurrency service implementation.
#[derive(Debug, ServiceFactory)]
#[exonum(proto_sources = "proto")]
pub struct CryptocurrencyService;

impl Service for CryptocurrencyService {
    fn wire_api(&self, descriptor: ServiceDescriptor, builder: &mut ServiceApiBuilder) {
        CryptocurrencyApi::new(descriptor).wire(builder);
    }
}
