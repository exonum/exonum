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

//! The comprehensive set of utilities to work with unsuccessful execution results
//! occurred within runtime workflow.
//!
//! The most important parts of the module are:
//!
//! - [`ExecutionFail`] - the trait representing an error type;
//! - [`CallSite`] - struct denoting the location of error;
//! - [`ExecutionError`] - the representation of occurred error;
//! - [`ExecutionStatus`] - result of execution, either successful or unsuccessful;
//! - [`ErrorMatch`] - the utility structure for matching the error against expected one.
//!
//! [`ExecutionFail`]: trait.ExecutionFail.html
//! [`CallSite`]: struct.CallSite.html
//! [`ExecutionError`]: struct.ExecutionError.html
//! [`ExecutionStatus`]: struct.ExecutionStatus.html
//! [`ErrorMatch`]: struct.ErrorMatch.html

#[doc(hidden)]
pub mod execution_error;

mod common_errors;
mod core_errors;
mod error_kind;
mod error_match;
mod execution_status;
#[cfg(test)]
mod tests;

pub use self::{
    common_errors::CommonError, core_errors::CoreError, error_kind::ErrorKind,
    error_match::ErrorMatch, execution_status::ExecutionStatus,
};

use exonum_derive::*;
use exonum_merkledb::Error as MerkledbError;
use exonum_proto::ProtobufConvert;
use thiserror::Error;

use std::{
    fmt::{self, Display},
    panic,
};

use super::{CallInfo, InstanceId, MethodId};
use crate::proto::schema::errors as errors_proto;

/// Trait representing an error type defined in the service or runtime code.
///
/// This trait can be derived from an enum using an eponymous derive macro from the `exonum-derive`
/// crate. Using such errors is the preferred way to generate errors in Rust services.
///
/// # Examples
///
/// ```
/// use exonum_derive::*;
/// # use exonum::runtime::{ExecutionError};
///
/// /// Error codes emitted by wallet transactions during execution:
/// #[derive(Debug, ExecutionFail)]
/// pub enum Error {
///     /// Content hash already exists.
///     HashAlreadyExists = 0,
///     /// Unable to parse the service configuration.
///     ConfigParseError = 1,
///     /// Time service with the specified name does not exist.
///     TimeServiceNotFound = 2,
/// }
pub trait ExecutionFail {
    /// Extracts the error kind.
    fn kind(&self) -> ErrorKind;

    /// Extracts the human-readable error description.
    fn description(&self) -> String;

    /// Creates an error with an externally provided description. The default implementation
    /// takes the `description` as is; implementations can redefine this to wrap it in
    /// an error-specific wrapper.
    fn with_description(&self, description: impl Display) -> ExecutionError {
        ExecutionError::new(self.kind(), description.to_string())
    }
}

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
/// Therefore descriptions are mostly used for developer purposes, not for interaction with users.
///
/// [`ErrorKind`]: enum.ErrorKind.html
/// [`CallSite`]: struct.CallSite.html
#[derive(Clone, Debug, Error, BinaryValue)]
#[cfg_attr(test, derive(PartialEq))]
// ^-- Comparing `ExecutionError`s directly is error-prone, since the call info is not controlled
// by the caller. It is useful for roundtrip tests, though.
pub struct ExecutionError {
    kind: ErrorKind,
    description: String,
    runtime_id: Option<u32>,
    call_site: Option<CallSite>,
}

/// Additional details about an `ExecutionError` that do not influence blockchain state hash.
#[derive(Debug, Clone, ProtobufConvert, BinaryValue)]
#[protobuf_convert(source = "errors_proto::ExecutionErrorAux")]
pub(crate) struct ExecutionErrorAux {
    /// Human-readable error description.
    pub description: String,
}

/// Invokes closure, capturing the cause of the unwinding panic if one occurs.
///
/// This function will return the result of the closure if the closure does not panic.
/// If the closure panics, it returns an `Unexpected` error with the description derived
/// from the panic object.
///
/// `merkledb`s are not caught by this method.
pub fn catch_panic<F, T>(maybe_panic: F) -> Result<T, ExecutionError>
where
    F: FnOnce() -> Result<T, ExecutionError>,
{
    let result = panic::catch_unwind(panic::AssertUnwindSafe(maybe_panic));
    match result {
        // ExecutionError without panic.
        Ok(Err(e)) => Err(e),
        // Panic.
        Err(panic) => {
            if panic.is::<MerkledbError>() {
                // Continue panic unwinding if the reason is MerkledbError.
                panic::resume_unwind(panic);
            }
            Err(ExecutionError::from_panic(panic))
        }
        // Normal execution.
        Ok(Ok(value)) => Ok(value),
    }
}

/// Site of a call where an `ExecutionError` may occur.
///
/// Note that an error may occur in the runtime code (including the code glue provided by the runtime)
/// or in the service code, depending on the `kind` of the error.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, BinaryValue)]
#[non_exhaustive]
pub struct CallSite {
    /// ID of the service instance handling the call.
    pub instance_id: InstanceId,
    /// Type of a call.
    #[serde(flatten)]
    pub call_type: CallType,
}

impl CallSite {
    pub(crate) fn new(instance_id: InstanceId, call_type: CallType) -> Self {
        Self {
            instance_id,
            call_type,
        }
    }

    pub(crate) fn from_call_info(call_info: &CallInfo, interface: impl Into<String>) -> Self {
        Self {
            instance_id: call_info.instance_id,
            call_type: CallType::Method {
                interface: interface.into(),
                id: call_info.method_id,
            },
        }
    }
}

impl fmt::Display for CallSite {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} of service {}",
            self.call_type, self.instance_id
        )
    }
}

impl ProtobufConvert for CallSite {
    type ProtoStruct = errors_proto::CallSite;

    fn to_pb(&self) -> Self::ProtoStruct {
        use errors_proto::CallSite_Type::*;

        let mut pb = Self::ProtoStruct::new();
        pb.set_instance_id(self.instance_id);
        match &self.call_type {
            CallType::Constructor => pb.set_call_type(CONSTRUCTOR),
            CallType::Resume => pb.set_call_type(RESUME),
            CallType::Method { interface, id } => {
                pb.set_call_type(METHOD);
                pb.set_interface(interface.clone());
                pb.set_method_id(*id);
            }
            CallType::BeforeTransactions => pb.set_call_type(BEFORE_TRANSACTIONS),
            CallType::AfterTransactions => pb.set_call_type(AFTER_TRANSACTIONS),
        }
        pb
    }

    fn from_pb(mut pb: Self::ProtoStruct) -> anyhow::Result<Self> {
        use errors_proto::CallSite_Type::*;

        let call_type = match pb.get_call_type() {
            CONSTRUCTOR => CallType::Constructor,
            RESUME => CallType::Resume,
            BEFORE_TRANSACTIONS => CallType::BeforeTransactions,
            AFTER_TRANSACTIONS => CallType::AfterTransactions,
            METHOD => CallType::Method {
                interface: pb.take_interface(),
                id: pb.get_method_id(),
            },
        };
        Ok(Self::new(pb.get_instance_id(), call_type))
    }
}

/// Type of a call to a service.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "call_type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum CallType {
    /// Service initialization.
    Constructor,
    /// Service resuming routine.
    Resume,
    /// Service method.
    Method {
        /// Name of the interface defining the method. This field is empty for the default service
        /// interface.
        #[serde(default, skip_serializing_if = "String::is_empty")]
        interface: String,
        /// Numeric ID of the method.
        #[serde(rename = "method_id")]
        id: MethodId,
    },
    /// Hook executing before processing transactions in a block.
    BeforeTransactions,
    /// Hook executing after processing transactions in a block.
    AfterTransactions,
}

impl fmt::Display for CallType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Constructor => formatter.write_str("constructor"),
            Self::Resume => formatter.write_str("resuming routine"),
            Self::Method { interface, id } if interface.is_empty() => {
                write!(formatter, "method {}", id)
            }
            Self::Method { interface, id } => write!(formatter, "{}::(method {})", interface, id),
            Self::BeforeTransactions => formatter.write_str("before_transactions hook"),
            Self::AfterTransactions => formatter.write_str("after_transactions hook"),
        }
    }
}
