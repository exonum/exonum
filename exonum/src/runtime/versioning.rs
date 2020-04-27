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

//! Versioning tools for Exonum artifacts.
//!
//! # Versioning Problem
//!
//! The problem solved by versioning is as follows. Exonum services have clients, both internal
//! (other services on the same blockchain) and external (e.g., light clients and other software
//! capable of submitting transactions). For a multitude of reasons, the clients may have
//! different idea as to the service capabilities than the reality at hand.
//!
//! Here's hypothetical manifestations of the problem:
//!
//! - The client thinks service with a certain ID is a crypto-token service, but in reality
//!   it is a time oracle.
//! - The client correctly thinks that a service with a certain ID is a crypto-token service,
//!   but is unaware that the format of the transfer transaction has changed.
//! - The client (another service) attempts to get the consolidated time from the schema of a
//!   time oracle, but in reality it's not a time oracle. (Or it *is* a newer time oracle with
//!   changed schema layout.)
//!
//! In all these cases, the lack of knowledge on the client side may lead to unpredictable
//! consequences. In the best case, a transaction constructed by such a client will turn out
//! to be garbage from the service perspective, so it will *just* return a deserialization error.
//! In the worst case, the transaction may be interpreted arbitrarily. The same reasoning is
//! true for the service schema; in the best case, accessing the bogus schema will lead to an error
//! due to the mismatch of expected an actual index types. In the worst case, the indexes *will*
//! be accessed, but will return garbage data or lead to undefined behavior of the node.
//!
//! # Artifact versioning
//!
//! For any reasonable solution to the problem above to work, Exonum artifacts **must** be
//! [semantically versioned]. Indeed, semantic versioning allows to reason about client / service
//! compatibility in terms other than "Any specific version of a service artifact is absolutely
//! incompatible with any other version."
//!
//! Correct versioning is the responsibility of the service developers; the framework does not
//! (and cannot) check versioning automatically.
//!
//! The general guidelines to maximize service longevity are:
//!
//! - Versioning concerns *all* public interfaces of the service. As of Exonum 1.0, these interfaces
//!   are transactions and the (public part of) service schema.
//! - Transaction methods can be evolved much like Protobuf messages (in fact, transaction payloads
//!   should be Protobuf messages for this reason). Semantics of a method with the given ID must
//!   never change; in particular, the method ID must never be reused.
//! - Removing a method or disabling processing for certain payloads should be considered
//!   a breaking change (with a possible exclusion of bug fixes).
//! - Public service schema should expose the minimum possible number of indexes, since the changes
//!   in these indexes will be breaking. See the example below.
//! - Having non-public indexes in the public part of the schema does not solve the problem of
//!   compatibility. The client code will construct these indexes anyway, and if the indexes
//!   are gone or have been modified in a newer service version, this will lead to an error
//!   or undefined behavior. <!-- The root problem is that, unlike transactions, schema involves
//!   access code duplicated on the service and client sides. This will be solved after implementing
//!   transaction-like read requests. -->
//!
//! [semantically versioned]: https://semver.org/
//!
//! ## Transactions versioning
//!
//! To be able to process transactions, service must have a static mapping between numeric
//! identifier of transaction and logic of transaction processing. Logic of transaction processing
//! may include deserializing input parameters from byte array, processing the input and reporting
//! the execution result (which can be either successful or unsuccessful).
//!
//! **Important:** Transaction numeric identifier is considered a constant during all the time of
//! service existence. It means that if transaction was declared with certain ID, its logic can
//! be updated (e.g., to fix a bug) or be removed, but it **never** should be replaced with other
//! transaction.
//!
//! If transaction was removed from service, attempt to invoke it should always
//! result in returning an `ExecutionError`.
//!
//! You should use [`CommonError::MethodRemoved`] to report the error in case a method was removed.
//!
//! At the same time, Exonum core does not provide a tool for marking transaction as deprecated.
//! It is expected that service authors will notify users about transaction deprecation via
//! documentation update or in any other applicable way.
//!
//! [`CommonError::MethodRemoved`]: ../enum.CommonError.html#variant.MethodRemoved
//!
//! # Versioning for clients
//!
//! To defend against these scenarios, Exonum provides following defences.
//!
//! ## Manual Artifact Verification
//!
//! The client may check the name and version of the artifact for a specific service using
//! builtin APIs:
//!
//! - Internal clients may use the [`DispatcherSchema`] via the `for_dispatcher` method in
//!   [`BlockchainData`] or [`SnapshotExt`].
//! - External clients may use the public HTTP API of the node. Note that this check may be
//!   susceptible to [TOCTOU] issues.
//!
//! ## Version Tooling
//!
//! - For service schemas, `BlockchainData` and `SnapshotExt` expose the [`service_schema`]
//!   method. This allows to run versioning checks automatically.
//! - For transactions, clients may use the middleware service.
//!
//! # Examples
//!
//! Demonstrates how to define a service schema in a forward-compatible way.
//!
//! ```
//! # use exonum_merkledb::{
//! #     access::Access, Database, Entry, Group, ListIndex, ProofMapIndex, Snapshot,
//! #     TemporaryDB,
//! # };
//! # use exonum_derive::*;
//! /// Full schema which embeds the public part.
//! #[derive(Debug, FromAccess)]
//! pub(crate) struct SchemaImpl<T: Access> {
//!     /// Public part of the schema.
//!     #[from_access(flatten)]
//!     pub public: Schema<T>,
//!
//!     // Private fields (public within the crate). These fields may arbitrarily change
//!     // without breaking compatibility.
//!     pub private_entry: Entry<T::Base, String>,
//!     pub private_group: Group<T, str, ListIndex<T::Base, u64>>,
//! }
//!
//! /// Public part of the schema.
//! #[derive(Debug, FromAccess, RequireArtifact)]
//! #[require_artifact(name = "some.Token", version = "^1")]
//! pub struct Schema<T: Access> {
//!     /// Public index. Note that changing key or value type will be a breaking change.
//!     /// To extend interface longevity, it makes sense to make key / value types
//!     /// Protobuf messages.
//!     pub wallets: ProofMapIndex<T::Base, str, u64>,
//! }
//!
//! // Then, the `Schema` may be used like this:
//! use exonum::runtime::SnapshotExt;
//!
//! # fn access_schema() -> anyhow::Result<()> {
//! # let db = TemporaryDB::new();
//! let snapshot: Box<dyn Snapshot> = // ...
//! #   db.snapshot();
//! let schema: Schema<_> = snapshot.service_schema("my-service")?;
//! let balance = schema.wallets.get("Alice").unwrap_or(0);
//! # Ok(())
//! # }
//! ```
//!
//! [`DispatcherSchema`]: ../struct.DispatcherSchema.html
//! [`BlockchainData`]: ../struct.BlockchainData.html
//! [`SnapshotExt`]: ../trait.SnapshotExt.html
//! [`service_schema`]: ../struct.BlockchainData.html#method.service_schema
//! [TOCTOU]: https://en.wikipedia.org/wiki/Time-of-check_to_time-of-use

pub use semver::{Version, VersionReq};

use anyhow::format_err;
use thiserror::Error;

use std::{fmt, str::FromStr};

use crate::runtime::{ArtifactId, CoreError, ExecutionError, ExecutionFail};

/// Requirement on an artifact. Can be matched against artifact identifiers.
///
/// # Examples
///
/// ```
/// # use exonum::runtime::{versioning::ArtifactReq, ArtifactId, RuntimeIdentifier};
/// # fn main() -> anyhow::Result<()> {
/// // Requirements can be parsed from a string.
/// let req: ArtifactReq = "some.Service@^1.3.0".parse()?;
///
/// let valid_artifact = ArtifactId::new(
///     RuntimeIdentifier::Rust as u32,
///     "some.Service".to_owned(),
///     "1.5.7".parse()?,
/// )?;
/// assert!(req.try_match(&valid_artifact).is_ok());
///
/// // This artifact is outdated.
/// let mut outdated_artifact = valid_artifact.clone();
/// outdated_artifact.version = "1.2.0".parse()?;
/// assert!(req.try_match(&outdated_artifact).is_err());
///
/// // This artifact is too new.
/// let mut novel_artifact = valid_artifact.clone();
/// novel_artifact.version = "2.0.0".parse()?;
/// assert!(req.try_match(&novel_artifact).is_err());
///
/// // This artifact has wrong name.
/// let mut other_artifact = valid_artifact.clone();
/// other_artifact.name = "other.Service".to_owned();
/// assert!(req.try_match(&novel_artifact).is_err());
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ArtifactReq {
    /// Artifact name.
    pub name: String,
    /// Allowed artifact versions.
    pub version: VersionReq,
}

impl ArtifactReq {
    /// Creates a new artifact requirement.
    pub fn new(name: impl Into<String>, version: VersionReq) -> Self {
        Self {
            name: name.into(),
            version,
        }
    }

    /// Tries to match this requirement against the provided artifact.
    pub fn try_match(&self, artifact: &ArtifactId) -> Result<(), ArtifactReqError> {
        if artifact.name != self.name {
            return Err(ArtifactReqError::UnexpectedName {
                expected: self.name.clone(),
                actual: artifact.name.clone(),
            });
        }
        if !self.version.matches(&artifact.version) {
            return Err(ArtifactReqError::IncompatibleVersion {
                actual: artifact.version.clone(),
            });
        }
        Ok(())
    }
}

impl FromStr for ArtifactReq {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<_> = s.splitn(2, '@').collect();
        match &parts[..] {
            [name, version] => Ok(Self::new((*name).to_string(), version.parse()?)),
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

/// Versioned object that checks compatibility with the artifact of a service.
///
/// # Examples
///
/// This trait is usually implemented via the derive macro from the `exonum_derive` crate:
///
/// ```
/// use exonum_derive::*;
/// # use exonum_merkledb::{access::Access, Fork, ProofMapIndex};
/// # use exonum::runtime::versioning::RequireArtifact;
///
/// #[derive(Debug, FromAccess, RequireArtifact)]
/// #[require_artifact(name = "some.Service", version = "1")]
/// pub struct Schema<T: Access> {
///     pub wallets: ProofMapIndex<T::Base, str, u64>,
/// }
///
/// assert_eq!(
///     Schema::<&'static Fork>::required_artifact(),
///     "some.Service@^1".parse().unwrap()
/// );
/// ```
///
/// Both `name` and `version` fields of the `require_artifact` are have default values:
///
/// - `name` needs to agree with the artifact name as defined in the service factory
///   for the corresponding service. By default, it is set to the crate name.
/// - `version` is a semantic version requirement. By default, it is set to be semver-compatible
///   with the current version of the crate. For stability, it may make sense to set `version`
///   when the interface is created and not change it since. For example, a service may set
///   `version = "1"` in the v1.0.0 release and keep this requirement in the following
///   semver-compatible versions.
///
/// If the interface needs to be extended, you may define the extension as a new type
/// with the corresponding bump in `version`.
///
/// ```
/// # use exonum_derive::*;
/// # use exonum_merkledb::{access::Access, Fork, ProofEntry, ProofMapIndex};
/// # use exonum::runtime::versioning::RequireArtifact;
/// #[derive(Debug, FromAccess, RequireArtifact)]
/// #[require_artifact(name = "some.Service", version = "1.3.0")]
/// pub struct ExtendedSchema<T: Access> {
///     pub wallets: ProofMapIndex<T::Base, str, u64>,
///     /// Added in version 1.3.0.
///     pub total_token_amount: ProofEntry<T::Base, u64>,
/// }
/// # assert_eq!(
/// #     ExtendedSchema::<&'static Fork>::required_artifact(),
/// #     "some.Service@^1.3.0".parse().unwrap()
/// # );
/// ```
pub trait RequireArtifact {
    /// Returns the artifact requirement.
    fn required_artifact() -> ArtifactReq;
}

/// Artifact requirement error.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ArtifactReqError {
    /// No service with the specified identifier exists.
    #[error("No service with the specified identifier exists")]
    NoService,

    /// Unexpected artifact name.
    #[error("Unexpected artifact name ({}), was expecting `{}`", expected, actual)]
    UnexpectedName {
        /// Expected artifact name.
        expected: String,
        /// Actual artifact name.
        actual: String,
    },

    /// Incompatible artifact version.
    #[error("Incompatible artifact version ({})", actual)]
    IncompatibleVersion {
        /// Actual artifact version.
        actual: Version,
    },
}

impl From<ArtifactReqError> for ExecutionError {
    fn from(err: ArtifactReqError) -> Self {
        CoreError::IncorrectInstanceId.with_description(err.to_string())
    }
}

#[test]
fn artifact_req_parsing() {
    use pretty_assertions::assert_eq;

    let req: ArtifactReq = "exonum.Token@^1.0.5".parse().unwrap();
    assert_eq!(req.name, "exonum.Token");
    assert_eq!(req.version, "^1.0.5".parse().unwrap());
    assert_eq!(req.to_string(), "exonum.Token@^1.0.5");
}
