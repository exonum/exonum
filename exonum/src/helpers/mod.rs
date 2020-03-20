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

//! Different assorted utilities.

pub use self::types::{Height, Round, ValidatorId};

// Required by `consensus-tests`. This is not a public API, since `user_agent::get` is hidden
// under `doc(hidden)`.
pub use self::user_agent::{exonum_version, os_info, rust_version, user_agent};

pub(crate) use self::ordered_map::OrderedMap;
// `Milliseconds` is just `u64`, but more readable within context.
pub use self::types::Milliseconds;

mod ordered_map;

use env_logger::Builder;
use log::SetLoggerError;

mod types;
mod user_agent;

/// Initializes the logger.
///
/// See [`env_logger`] crate for details how to configure the logger output.
///
/// [`env_logger`]: https://docs.rs/env_logger/
pub fn init_logger() -> Result<(), SetLoggerError> {
    Builder::from_default_env()
        .format_timestamp_nanos()
        .try_init()
}

/// Basic trait to validate user defined input.
pub trait ValidateInput: Sized {
    /// The type returned in the event of a validate error.
    type Error;
    /// Perform parameters validation for this configuration and return error if
    /// value is inconsistent.
    fn validate(&self) -> Result<(), Self::Error>;
    /// The same as validate method, but returns the value itself as a successful result.
    fn into_validated(self) -> Result<Self, Self::Error> {
        self.validate().map(|_| self)
    }
}

/// Returns sufficient number of votes for the given validators number.
pub fn byzantine_quorum(total: usize) -> usize {
    total * 2 / 3 + 1
}

/// Module for serializing `Option<Hash>` to Protobuf.
///
/// It can be used with `ProtobufConvert` derive macro, e.g.:
///
/// ```ignore
/// #[derive(Debug, ProtobufConvert)]
/// #[protobuf_convert(source = "path::to::ProtoStructure")]
/// struct Structure {
///     #[protobuf_convert(with = "exonum::helpers::pb_optional_hash")]
///     pub maybe_hash: Option<Hash>,
/// }
/// ```
pub mod pb_optional_hash {
    use exonum_crypto::{proto::types::Hash as PbHash, Hash};
    use exonum_proto::ProtobufConvert;

    /// Deserializes `Option<Hash>` from Protobuf.
    pub fn from_pb(pb: PbHash) -> anyhow::Result<Option<Hash>> {
        if pb.get_data().is_empty() {
            Ok(None)
        } else {
            Hash::from_pb(pb).map(Some)
        }
    }

    /// Serializes `Option<Hash>` to Protobuf.
    pub fn to_pb(value: &Option<Hash>) -> PbHash {
        if let Some(hash) = value {
            hash.to_pb()
        } else {
            PbHash::new()
        }
    }
}

/// Module for serializing `semver::Version` to Protobuf.
///
/// It can be used with `ProtobufConvert` derive macro, e.g.:
///
/// ```ignore
/// #[derive(Debug, ProtobufConvert)]
/// #[protobuf_convert(source = "path::to::ProtoStructure")]
/// struct Structure {
///     #[protobuf_convert(with = "exonum::helpers::pb_version")]
///     pub some_version: Version,
/// }
/// ```
pub mod pb_version {
    use semver::Version;

    /// Deserializes `semver::Version` from string.
    #[allow(clippy::needless_pass_by_value)] // False positive, we need a `String` type here.
    pub fn from_pb(pb: String) -> anyhow::Result<Version> {
        pb.parse().map_err(From::from)
    }

    /// Serializes `semver::Version` to string.
    pub fn to_pb(value: &Version) -> String {
        value.to_string()
    }
}
