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

//! Module with `ExecutionError`, the essential representation of unsuccessfull runtime execution.

use serde::{de::Error, Deserialize, Deserializer, Serialize, Serializer};

use super::execution_status::serde::ExecutionStatus;

use exonum_derive::*;
use exonum_merkledb::{BinaryValue, ObjectHash};
use exonum_proto::ProtobufConvert;
use failure::{bail, Fail};

use std::{
    any::Any,
    convert::TryFrom,
    fmt::{self, Display},
};

use super::{ErrorCode, ErrorKind, ErrorMatch};
use crate::{
    crypto::{self, Hash},
    proto::schema::runtime as runtime_proto,
    runtime::{CallSite, RuntimeIdentifier},
};

/// Result of unsuccessful runtime execution.
///
/// An execution error consists of:
///
/// - an [error kind][`ErrorKind`]
/// - call information (runtime ID and, if appropriate, [`CallSite`] where the error has occurred)
/// - an optional description
///
/// Call information is added by the core automatically; it is impossible to add from the service
/// code. It *is* possible to inspect the call info for an error that was returned by a service
/// though.
///
/// The error kind and call info affect the blockchain state hash, while the description does not.
/// Therefore descriptions are mostly used for developer purposes, not for interaction of
/// the system with users.
///
/// [`ErrorKind`]: enum.ErrorKind.html
/// [`CallSite`]: struct.CallSite.html
#[derive(Clone, Debug, Fail, BinaryValue)]
#[cfg_attr(test, derive(PartialEq))]
// ^-- Comparing `ExecutionError`s directly is error-prone, since the call info is not controlled
// by the caller. It is useful for roundtrip tests, though.
pub struct ExecutionError {
    pub(super) kind: ErrorKind,
    pub(super) description: String,
    pub(super) runtime_id: Option<u32>,
    pub(super) call_site: Option<CallSite>,
}

/// Custom `serde` implementation for `ExecutionError`.
#[doc(hidden)]
#[derive(Debug)]
pub struct ExecutionErrorSerde;

impl ExecutionError {
    /// Creates a new execution error instance with the specified error kind
    /// and an optional description.
    pub fn new(kind: ErrorKind, description: impl Into<String>) -> Self {
        Self {
            kind,
            description: description.into(),
            runtime_id: None,
            call_site: None,
        }
    }

    /// Creates an execution error for use in service code.
    pub fn service(code: u8, description: impl Into<String>) -> Self {
        Self::new(
            ErrorKind::Service {
                code: ErrorCode::Custom(code),
            },
            description,
        )
    }

    /// Creates an execution error from the panic description.
    pub(super) fn from_panic(any: impl AsRef<(dyn Any + Send)>) -> Self {
        let any = any.as_ref();

        // Tries to get a meaningful description from the given panic.
        let description = if let Some(s) = any.downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = any.downcast_ref::<String>() {
            s.clone()
        } else if let Some(error) = any.downcast_ref::<Box<(dyn std::error::Error + Send)>>() {
            error.description().to_string()
        } else if let Some(error) = any.downcast_ref::<failure::Error>() {
            error.to_string()
        } else {
            // Unknown error kind; keep its description empty.
            String::new()
        };

        Self::new(ErrorKind::Unexpected, description)
    }

    /// Converts an error to a matcher. The matcher expect the exact kind and description
    /// of this error, and does not check any other error fields.
    pub fn to_match(&self) -> ErrorMatch {
        ErrorMatch::new(self.kind, self.description.clone())
    }

    /// The kind of error that indicates in which module and with which code the error occurred.
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// Human-readable error description. May be empty.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Returns the ID of a runtime in which this error has occurred. If the runtime is not known,
    /// returns `None`.
    pub fn runtime_id(&self) -> Option<u32> {
        self.runtime_id
    }

    #[inline]
    pub(crate) fn set_runtime_id(&mut self, runtime_id: u32) -> &mut Self {
        if self.runtime_id.is_none() {
            self.runtime_id = Some(runtime_id);
        }
        self
    }

    /// Returns the call site of the error.
    pub fn call_site(&self) -> Option<&CallSite> {
        self.call_site.as_ref()
    }

    #[inline]
    pub(crate) fn set_call_site(&mut self, call_site: impl FnOnce() -> CallSite) -> &mut Self {
        if self.call_site.is_none() {
            self.call_site = Some(call_site());
        }
        self
    }
}

impl Display for ExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref call_site) = self.call_site {
            write!(
                formatter,
                "Execution error with code `{kind}` occurred in {site}",
                kind = self.kind,
                site = call_site
            )?;
        } else if let Some(runtime_id) = self.runtime_id {
            write!(
                formatter,
                "Execution error with code `{kind}` occurred in {runtime}",
                kind = self.kind,
                runtime = match RuntimeIdentifier::transform(runtime_id) {
                    Ok(runtime) => runtime.to_string(),
                    Err(_) => format!("Non-default runtime with id {}", runtime_id),
                }
            )?;
        } else {
            write!(
                formatter,
                "Execution error with code `{kind}` occurred",
                kind = self.kind
            )?;
        }

        if !self.description.is_empty() {
            write!(formatter, ": {}", self.description)?;
        }
        Ok(())
    }
}

impl ProtobufConvert for ExecutionError {
    type ProtoStruct = runtime_proto::ExecutionError;

    fn to_pb(&self) -> Self::ProtoStruct {
        let mut inner = Self::ProtoStruct::default();
        let (kind, code) = self.kind.into_raw();
        inner.set_kind(kind);
        inner.set_code(code as u32);
        inner.set_description(self.description.clone());

        if let Some(runtime_id) = self.runtime_id {
            inner.set_runtime_id(runtime_id);
        } else {
            inner.set_no_runtime_id(Default::default());
        }

        if let Some(ref call_site) = self.call_site {
            inner.set_call_site(call_site.to_pb());
        } else {
            inner.set_no_call_site(Default::default());
        }
        inner
    }

    fn from_pb(mut pb: Self::ProtoStruct) -> Result<Self, failure::Error> {
        let kind = pb.get_kind();
        let raw_code = u16::try_from(pb.get_code())?;

        let kind = ErrorKind::from_raw(kind, raw_code)?;

        let runtime_id = if pb.has_no_runtime_id() {
            None
        } else if pb.has_runtime_id() {
            Some(pb.get_runtime_id())
        } else {
            bail!("No runtime info or no_runtime_id marker");
        };

        let call_site = if pb.has_no_call_site() {
            None
        } else if pb.has_call_site() {
            Some(CallSite::from_pb(pb.take_call_site())?)
        } else {
            bail!("No call site info or no_call_site marker");
        };

        Ok(Self {
            kind,
            description: pb.take_description(),
            runtime_id,
            call_site,
        })
    }
}

// String content (`ExecutionError::description`) is intentionally excluded from the hash
// calculation because user can be tempted to use error description from a third-party libraries
// which aren't stable across the versions.
impl ObjectHash for ExecutionError {
    fn object_hash(&self) -> Hash {
        let error_with_empty_description = Self {
            kind: self.kind,
            description: String::new(),
            runtime_id: self.runtime_id,
            call_site: self.call_site.clone(),
        };
        crypto::hash(&error_with_empty_description.into_bytes())
    }
}

impl PartialEq<ErrorMatch> for ExecutionError {
    fn eq(&self, error_match: &ErrorMatch) -> bool {
        let kind_matches = self.kind == error_match.kind;
        let runtime_matches = match (error_match.runtime_id, self.runtime_id) {
            (None, _) => true,
            (Some(match_id), Some(id)) => match_id == id,
            _ => false,
        };
        let instance_matches = match (error_match.instance_id, &self.call_site) {
            (None, _) => true,
            (Some(match_id), Some(CallSite { instance_id, .. })) => match_id == *instance_id,
            _ => false,
        };
        let call_type_matches = match (&error_match.call_type, &self.call_site) {
            (None, _) => true,
            (Some(match_type), Some(CallSite { call_type, .. })) => match_type == call_type,
            _ => false,
        };
        kind_matches
            && runtime_matches
            && instance_matches
            && call_type_matches
            && error_match.description.matches(&self.description)
    }
}

impl PartialEq<ExecutionError> for ErrorMatch {
    fn eq(&self, other: &ExecutionError) -> bool {
        other.eq(self)
    }
}

impl ExecutionErrorSerde {
    pub fn serialize<S>(inner: &ExecutionError, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ExecutionStatus::from(Err(inner)).serialize(serializer)
    }

    pub fn deserialize<'a, D>(deserializer: D) -> Result<ExecutionError, D::Error>
    where
        D: Deserializer<'a>,
    {
        ExecutionStatus::deserialize(deserializer).and_then(|status| {
            status
                .into_result()
                .and_then(|res| match res {
                    Err(err) => Ok(err),
                    Ok(()) => Err("Not an error"),
                })
                .map_err(D::Error::custom)
        })
    }
}
