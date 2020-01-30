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

//! Module with `ExecutionError`, the essential representation of unsuccessful runtime execution.

use serde::{de::Error, Deserialize, Deserializer, Serialize, Serializer};

use exonum_merkledb::{BinaryValue, ObjectHash};
use exonum_proto::ProtobufConvert;
use failure::bail;

use std::{
    any::Any,
    convert::TryFrom,
    fmt::{self, Display},
};

use super::{execution_status::serde::ExecutionStatus, ErrorKind, ErrorMatch, ExecutionError};
use crate::{
    crypto::{self, Hash},
    proto::schema,
    runtime::{CallSite, RuntimeIdentifier},
};

/// Custom `serde` implementation for `ExecutionError`.
#[doc(hidden)]
#[derive(Debug)]
pub struct ExecutionErrorSerde;

impl ExecutionError {
    /// Creates a new execution error instance with the specified error kind
    /// and an optional description.
    #[doc(hidden)] // used by `derive(ExecutionFail)`
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
        Self::new(ErrorKind::Service { code }, description)
    }

    /// Tries to get a meaningful description from the given panic.
    pub(crate) fn description_from_panic(any: impl AsRef<(dyn Any + Send)>) -> String {
        let any = any.as_ref();

        if let Some(s) = any.downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = any.downcast_ref::<String>() {
            s.clone()
        } else if let Some(error) = any.downcast_ref::<Box<(dyn std::error::Error + Send)>>() {
            error.description().to_string()
        } else if let Some(error) = any.downcast_ref::<failure::Error>() {
            error.to_string()
        } else {
            // Unknown error kind; keep its description empty.
            String::new()
        }
    }

    /// Creates an execution error from the panic description.
    pub(super) fn from_panic(any: impl AsRef<(dyn Any + Send)>) -> Self {
        let description = Self::description_from_panic(any);
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

    /// Returns the ID of a runtime in which this error has occurred. If the runtime is not known
    /// (e.g., the error originates in the core code), returns `None`.
    pub fn runtime_id(&self) -> Option<u32> {
        self.runtime_id
    }

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
    type ProtoStruct = schema::errors::ExecutionError;

    fn to_pb(&self) -> Self::ProtoStruct {
        let mut inner = Self::ProtoStruct::default();
        let (kind, code) = self.kind.into_raw();
        inner.set_kind(kind);
        inner.set_code(u32::from(code));
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
        let code = u8::try_from(pb.get_code())?;

        let kind = ErrorKind::from_raw(kind, code)?;

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
