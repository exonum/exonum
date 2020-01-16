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

//! Middleware service providing ways to compose transactions from the simpler building blocks.
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
//! [Batching]: trait.MiddlewareInterface.html#tymethod.batch
//! [Checked call]: trait.MiddlewareInterface.html#tymethod.checked_call
//! [TOCTOU]: https://en.wikipedia.org/wiki/Time-of-check_to_time-of-use

#![deny(
    unsafe_code,
    bare_trait_objects,
    missing_docs,
    missing_debug_implementations
)]

pub use self::transactions::{
    Batch, CheckedCall, Error, MiddlewareInterface, MiddlewareInterfaceMut,
};

pub mod proto;
mod transactions;

use exonum::runtime::{versioning, InstanceId};
use exonum_derive::*;
use exonum_rust_runtime::{DefaultInstance, Service};

use std::{fmt, str::FromStr};

/// Middleware service.
#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_dispatcher(implements("MiddlewareInterface"))]
#[service_factory(proto_sources = "proto")]
pub struct MiddlewareService;

impl Service for MiddlewareService {}

impl DefaultInstance for MiddlewareService {
    const INSTANCE_ID: InstanceId = 1;
    const INSTANCE_NAME: &'static str = "middleware";
}

/// A wrapper around an artifact requirement.
///
/// Necessary as a separate type because of Rust orphaning rules: we want to use the requirement
/// as a stub, but the return type ([`CheckedCall`]) is defined in this crate.
///
/// [`CheckedCall`]: struct.CheckedCall.html
///
/// # Examples
///
/// ```
/// # use exonum::runtime::InstanceId;
/// # use exonum_derive::*;
/// # use exonum_middleware_service::{ArtifactReq, CheckedCall};
/// let req: ArtifactReq = "some.Service@^1.3.0".parse().unwrap();
///
/// // Suppose the interface for `some.Service` is defined as follows:
/// #[exonum_interface]
/// trait SomeService<Ctx> {
///     type Output;
///     #[interface_method(id = 0)]
///     fn do_something(&self, ctx: Ctx, arg: String) -> Self::Output;
/// }
///
/// // Then, requirements can be used to perform a checked call to the service.
/// const SERVICE_ID: InstanceId = 100;
/// let checked_call: CheckedCall = req.do_something(SERVICE_ID, "Arg".into());
/// ```
#[derive(Clone, PartialEq)]
pub struct ArtifactReq(pub versioning::ArtifactReq);

impl From<versioning::ArtifactReq> for ArtifactReq {
    fn from(value: versioning::ArtifactReq) -> Self {
        ArtifactReq(value)
    }
}

impl From<ArtifactReq> for versioning::ArtifactReq {
    fn from(value: ArtifactReq) -> Self {
        value.0
    }
}

impl fmt::Debug for ArtifactReq {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, formatter)
    }
}

impl fmt::Display for ArtifactReq {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, formatter)
    }
}

impl FromStr for ArtifactReq {
    type Err = <versioning::ArtifactReq as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        versioning::ArtifactReq::from_str(s).map(ArtifactReq)
    }
}
