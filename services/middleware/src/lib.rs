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

use exonum::runtime::InstanceId;
use exonum_derive::*;
use exonum_rust_runtime::{DefaultInstance, Service};
use failure::format_err;
use semver::VersionReq;

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

/// Requirement on an artifact.
///
/// # Examples
///
/// Requirements can be used as a stub, generating a [`CheckedCall`].
///
/// [`CheckedCall`]: struct.CheckedCall.html
///
/// ```
/// # use exonum::runtime::InstanceId;
/// # use exonum_derive::*;
/// # use exonum_middleware_service::{ArtifactReq, CheckedCall};
/// // Requirements can be parsed from a string.
/// let req: ArtifactReq = "some.Service@^1.3.0".parse().unwrap();
///
/// // Suppose the interface for `some.Service` is defined as follows:
/// #[exonum_interface]
/// trait SomeService<Ctx> {
///     type Output;
///     fn do_something(&self, ctx: Ctx, arg: String) -> Self::Output;
/// }
///
/// // Then, requirements can be used to perform a checked call to the service.
/// const SERVICE_ID: InstanceId = 100;
/// let checked_call: CheckedCall = req.do_something(SERVICE_ID, "Arg".into());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ArtifactReq {
    /// Artifact name.
    pub name: String,
    /// Allowed artifact versions.
    pub version: VersionReq,
}

impl FromStr for ArtifactReq {
    type Err = failure::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<_> = s.splitn(2, '@').collect();
        match &parts[..] {
            [name, version] => Ok(Self {
                name: name.to_string(),
                version: version.parse()?,
            }),
            _ => Err(format_err!(
                "Invalid artifact requirement. Use `name@version` format, \
                 e.g., `exonum.Token@^1.3.0`"
            )),
        }
    }
}

impl fmt::Display for ArtifactReq {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}@{}", self.name, self.version)
    }
}

#[test]
fn artifact_req_parsing() {
    let req: ArtifactReq = "exonum.Token@^1.0.5".parse().unwrap();
    assert_eq!(req.name, "exonum.Token");
    assert_eq!(req.version, "^1.0.5".parse().unwrap());
    assert_eq!(req.to_string(), "exonum.Token@^1.0.5");
}
