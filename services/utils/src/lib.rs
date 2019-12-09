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

//! Utility service providing ways to compose transactions from the simpler building blocks.
//!
//! # Functionality overview
//!
//! ## Transaction batching
//!
//! [Batching] allows to atomically execute several transactions; if an error occurs
//! during execution, changes made by all transactions are rolled back. All transactions
//! in the batch are authorized in the same way as the batch itself.
//!
//! ## Checked call
//!
//! [Checked call] is a way to ensure that the called service corresponds to a specific artifact
//! with an expected version range. Unlike alternatives (e.g., finding out this information via
//! the `services` endpoint of the node HTTP API), using checked calls is most failsafe; by design,
//! it cannot suffer from [TOCTOU] issues. It does impose a certain overhead on the execution, though.
//!
//! [Batching]: trait.UtilsInterface.html#tymethod.batch
//! [Checked call]: trait.UtilsInterface.html#tymethod.checked_call
//! [TOCTOU]: https://en.wikipedia.org/wiki/Time-of-check_to_time-of-use

#![deny(
    unsafe_code,
    bare_trait_objects,
    missing_docs,
    missing_debug_implementations
)]

pub use self::transactions::{Batch, CheckedCall, Error, UtilsInterface};

pub mod proto;
mod transactions;

use exonum::runtime::{
    rust::{DefaultInstance, Service},
    InstanceId,
};
use exonum_derive::*;

/// Utility service.
#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_dispatcher(implements("UtilsInterface"))]
#[service_factory(proto_sources = "proto")]
pub struct UtilsService;

impl Service for UtilsService {}

impl DefaultInstance for UtilsService {
    const INSTANCE_ID: InstanceId = 1;
    const INSTANCE_NAME: &'static str = "utils";
}
