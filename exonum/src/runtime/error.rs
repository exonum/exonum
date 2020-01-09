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

//! The set of errors for the Runtime module.

#[doc(hidden)]
pub mod execution_error;
mod execution_result;
#[cfg(test)]
mod tests;

// This import is used in the blockchain explorer.
pub use self::execution_result::ExecutionStatus as SerdeExecutionStatus;

use exonum_derive::*;
use exonum_merkledb::Error as MerkledbError;
use exonum_merkledb::{BinaryValue, ObjectHash};
use exonum_proto::ProtobufConvert;
use failure::{bail, ensure, Fail};

use std::{
    any::Any,
    convert::TryFrom,
    fmt::{self, Display},
    panic,
};

use super::{InstanceId, MethodId, RuntimeIdentifier};
use crate::{
    crypto::{self, Hash},
    proto::schema::runtime as runtime_proto,
};

/// Kind of execution error, indicates the location of the error.
///
/// # Note to Runtime Developers
///
/// When should a runtime use different kinds of errors? Here's the guide.
///
/// ## `Service` errors
///
/// Use `Service` kind if the error has occurred in the service code and it makes sense to notify
/// users about the error cause and/or its precise kind. These errors are generally raised
/// if the input data (e.g., the transaction payload) violate certain invariants imposed by the service.
/// For example, a `Service` error can be raised if the sender of a transfer transaction
/// in the token service does not have sufficient amount of tokens.
///
/// ## `Unexpected` errors
///
/// Use `Unexpected` kind if the error has occurred in the service code, and at least one
/// of the following conditions holds:
///
/// - The error is caused by the environment (e.g., out-of-memory)
/// - The error should never occur during normal execution (e.g., out-of-bounds indexing, null pointer
///   dereference)
///
/// This kind of errors generally corresponds to panics in Rust and unchecked exceptions in Java.
/// `Unexpected` errors are assumed to be reasonably rare by the framework; e.g., they are logged
/// with a higher priority than other kinds.
///
/// Runtime environments can have mechanisms to convert `Unexpected` errors to `Service` ones
/// (e.g., by catching exceptions in Java or calling [`catch_unwind`] in Rust),
/// but whether it makes sense heavily depends on the use case.
///
/// ## `Dispatcher` errors
///
/// Use `Dispatcher` kind if the error has occurred while dispatching the request (i.e., *not*
/// in the client code). See [`DispatcherError`] for more details.
///
/// ## `Runtime` errors
///
/// Use `Runtime` kind if a recoverable error has occurred in the runtime code and
/// it makes sense to report the error to the users. A primary example here is artifact deployment:
/// if the deployment has failed due to a reproducible condition (e.g., the artifact
/// cannot be compiled), a `Runtime` error can provide more details about the cause.
///
/// ## Policy on panics
///
/// Panic in the Rust wrapper of the runtime if a fundamental runtime invariant is broken and
/// continuing node operation is impossible. A panic will not be caught and will lead
/// to the node termination.
///
/// [`catch_unwind`]: https://doc.rust-lang.org/std/panic/fn.catch_unwind.html
/// [`DispatcherError`]: enum.DispatcherError.html
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ErrorKind {
    /// An unexpected error that has occurred in the service code.
    ///
    /// Unlike [`Service`](#variant.Service) errors, unexpected errors do not have a user-defined code.
    /// Thus, it may be impossible for users to figure out the precise cause of the error;
    /// they can only use the accompanying error description.
    Unexpected,

    /// An error in the dispatcher code. For example, the method with the specified ID
    /// was not found in the service instance.
    Dispatcher {
        /// Error code. Available values can be found in the [description] of dispatcher errors.
        ///
        /// [description]: enum.DispatcherError.html
        code: u8,
    },

    /// An error in the runtime logic. For example, the runtime could not compile an artifact.
    Runtime {
        /// Runtime-specific error code.
        /// Error codes can have different meanings for different runtimes.
        code: u8,
    },

    /// An error in the service code reported to the blockchain users.
    Service {
        /// User-defined error code.
        /// Error codes can have different meanings for different services.
        code: u8,
    },
}

impl ErrorKind {
    fn into_raw(self) -> (runtime_proto::ErrorKind, u8) {
        match self {
            ErrorKind::Unexpected => (runtime_proto::ErrorKind::UNEXPECTED, 0),
            ErrorKind::Dispatcher { code } => (runtime_proto::ErrorKind::DISPATCHER, code),
            ErrorKind::Runtime { code } => (runtime_proto::ErrorKind::RUNTIME, code),
            ErrorKind::Service { code } => (runtime_proto::ErrorKind::SERVICE, code),
        }
    }

    fn from_raw(kind: runtime_proto::ErrorKind, code: u8) -> Result<Self, failure::Error> {
        use runtime_proto::ErrorKind::*;
        let kind = match kind {
            UNEXPECTED => {
                ensure!(code == 0, "Error code for panic should be zero");
                ErrorKind::Unexpected
            }
            DISPATCHER => ErrorKind::Dispatcher { code },
            RUNTIME => ErrorKind::Runtime { code },
            SERVICE => ErrorKind::Service { code },
        };
        Ok(kind)
    }
}

impl Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorKind::Unexpected => f.write_str("unexpected"),
            ErrorKind::Dispatcher { code } => write!(f, "dispatcher:{}", code),
            ErrorKind::Runtime { code } => write!(f, "runtime:{}", code),
            ErrorKind::Service { code } => write!(f, "service:{}", code),
        }
    }
}

/// Trait representing an error type defined in the service or runtime code.
///
/// This trait can be derived from an enum using an eponymous derive macro from the `exonum-derive`
/// crate. Using such errors is the preferred way to generate errors in Rust services.
///
/// # Examples
///
/// ```
/// use exonum_derive::ExecutionFail;
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
/// ```
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

/// Matcher for `ExecutionError`s that can have some fields unspecified. Can be compared to
/// an `ExceptionError`, e.g., in tests. The unspecified fields will match any value in the error.
///
/// # Examples
///
/// ```
/// use exonum::runtime::{ExecutionError, InstanceId, ErrorMatch};
/// use exonum_derive::ExecutionFail;
///
/// #[derive(Debug, ExecutionFail)]
/// pub enum Error {
///     /// Content hash already exists.
///     HashAlreadyExists = 0,
///     // other variants...
/// }
///
/// // Identifier of the service that will cause an error.
/// const SERVICE_ID: InstanceId = 100;
///
/// # fn not_run(error: ExecutionError) {
/// let err: &ExecutionError = // ...
/// #    &error;
/// let matcher = ErrorMatch::from_fail(&Error::HashAlreadyExists)
///     .for_service(SERVICE_ID);
/// assert_eq!(*err, matcher);
/// # }
/// ```
#[derive(Debug)]
pub struct ErrorMatch {
    kind: ErrorKind,
    description: StringMatch,
    runtime_id: Option<u32>,
    instance_id: Option<InstanceId>,
    call_type: Option<CallType>,
}

impl ErrorMatch {
    /// Creates a matcher from the provided error.
    ///
    /// The converted error has a kind and description set to the values returned
    /// by the corresponding methods of the [`ExecutionFail`] trait. The call site information
    /// (e.g., the instance ID) is not set.
    ///
    /// [`ExecutionFail`]: trait.ExecutionFail.html
    pub fn from_fail<F: ExecutionFail + ?Sized>(fail: &F) -> Self {
        Self::new(fail.kind(), fail.description())
    }

    /// Creates a matcher for `Unexpected` kind of errors.
    /// By default it will match any description.
    pub fn any_unexpected() -> Self {
        Self {
            kind: ErrorKind::Unexpected,
            description: StringMatch::Any,
            runtime_id: None,
            instance_id: None,
            call_type: None,
        }
    }

    fn new(kind: ErrorKind, description: String) -> Self {
        Self {
            kind,
            description: StringMatch::Exact(description),
            runtime_id: None,
            instance_id: None,
            call_type: None,
        }
    }

    /// Accepts an error with any description.
    pub fn with_any_description(mut self) -> Self {
        self.description = StringMatch::Any;
        self
    }

    /// Accepts an error with any description containing the specified string.
    pub fn with_description_containing(mut self, pat: impl Into<String>) -> Self {
        self.description = StringMatch::Contains(pat.into());
        self
    }

    /// Accepts an error with any description matching the specified closure.
    pub fn with_description_matching<P>(mut self, pat: P) -> Self
    where
        P: Fn(&str) -> bool + 'static,
    {
        self.description = StringMatch::Generic(Box::new(pat));
        self
    }

    /// Accepts an error that has occurred in a runtime with the specified ID.
    pub fn in_runtime(mut self, runtime_id: u32) -> Self {
        self.runtime_id = Some(runtime_id);
        self
    }

    /// Accepts an error that has occurred in a service with the specified ID.
    pub fn for_service(mut self, instance_id: InstanceId) -> Self {
        self.instance_id = Some(instance_id);
        self
    }

    /// Accepts an error that has occurred in a call of the specified format.
    pub fn in_call(mut self, call_type: CallType) -> Self {
        self.call_type = Some(call_type);
        self
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

enum StringMatch {
    Any,
    Exact(String),
    Contains(String),
    Generic(Box<dyn Fn(&str) -> bool>),
}

impl fmt::Debug for StringMatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StringMatch::Any => formatter.write_str("Any"),
            StringMatch::Exact(s) => formatter.debug_tuple("Exact").field(s).finish(),
            StringMatch::Contains(s) => formatter.debug_tuple("Contains").field(s).finish(),
            StringMatch::Generic(_) => formatter.debug_tuple("Generic").field(&"_").finish(),
        }
    }
}

impl StringMatch {
    fn matches(&self, s: &str) -> bool {
        match self {
            StringMatch::Any => true,
            StringMatch::Exact(ref_str) => ref_str == s,
            StringMatch::Contains(needle) => s.contains(needle),
            StringMatch::Generic(match_fn) => match_fn(s),
        }
    }
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
    kind: ErrorKind,
    description: String,
    runtime_id: Option<u32>,
    call_site: Option<CallSite>,
}

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
        Self::new(ErrorKind::Service { code }, description)
    }

    /// Tries to get a meaningful description from the given panic.
    pub(super) fn description_from_panic(any: impl AsRef<(dyn Any + Send)>) -> String {
        let any = any.as_ref();

        if let Some(s) = any.downcast_ref::<&str>() {
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
        }
    }

    /// Creates an execution error from the panic description.
    fn from_panic(any: impl AsRef<(dyn Any + Send)>) -> Self {
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

    /// Returns the ID of a runtime in which this error has occurred. If the runtime is not known,
    /// returns `None`.
    pub fn runtime_id(&self) -> Option<u32> {
        self.runtime_id
    }

    #[inline]
    pub(super) fn set_runtime_id(&mut self, runtime_id: u32) -> &mut Self {
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
    pub(super) fn set_call_site(&mut self, call_site: impl FnOnce() -> CallSite) -> &mut Self {
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

/// Site of a call where an `ExecutionError` may occur.
///
/// Note that an error may occur in the runtime code (including the code glue provided by the runtime)
/// or in the service code, depending on the `kind` of the error.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, BinaryValue)]
pub struct CallSite {
    /// ID of the service instance that has generated an error.
    pub instance_id: InstanceId,
    /// Type of a call.
    #[serde(flatten)]
    pub call_type: CallType,
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
    type ProtoStruct = runtime_proto::CallSite;

    fn to_pb(&self) -> Self::ProtoStruct {
        use runtime_proto::CallSite_Type::*;

        let mut pb = Self::ProtoStruct::new();
        pb.set_instance_id(self.instance_id);
        match &self.call_type {
            CallType::Constructor => pb.set_call_type(CONSTRUCTOR),
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

    fn from_pb(mut pb: Self::ProtoStruct) -> Result<Self, failure::Error> {
        use runtime_proto::CallSite_Type::*;

        let call_type = match pb.get_call_type() {
            CONSTRUCTOR => CallType::Constructor,
            BEFORE_TRANSACTIONS => CallType::BeforeTransactions,
            AFTER_TRANSACTIONS => CallType::AfterTransactions,
            METHOD => CallType::Method {
                interface: pb.take_interface(),
                id: pb.get_method_id(),
            },
        };
        Ok(Self {
            instance_id: pb.get_instance_id(),
            call_type,
        })
    }
}

/// Type of a call to a service.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "call_type", rename_all = "snake_case")]
pub enum CallType {
    /// Service constructor.
    Constructor,
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
            CallType::Constructor => formatter.write_str("constructor"),
            CallType::Method { interface, id } if interface.is_empty() => {
                write!(formatter, "method {}", id)
            }
            CallType::Method { interface, id } => {
                write!(formatter, "{}::(method {})", interface, id)
            }
            CallType::BeforeTransactions => formatter.write_str("before_transactions hook"),
            CallType::AfterTransactions => formatter.write_str("after_transactions hook"),
        }
    }
}

/// Status of a call execution in a way it is stored in the blockchain.
/// This result may be either an empty unit type, in case of success,
/// or an `ExecutionError`, if execution has failed.
#[derive(Clone, Debug, Serialize, Deserialize, BinaryValue)]
#[cfg_attr(test, derive(PartialEq))]
pub struct ExecutionStatus(#[serde(with = "execution_result")] pub Result<(), ExecutionError>);

impl ExecutionStatus {
    /// Creates status for the successful execution.
    pub fn ok() -> Self {
        Self(Ok(()))
    }

    /// Creates status for the failed execution.
    pub fn err(err: impl Into<ExecutionError>) -> Self {
        Self(Err(err.into()))
    }
}

impl From<Result<(), ExecutionError>> for ExecutionStatus {
    fn from(inner: Result<(), ExecutionError>) -> Self {
        Self(inner)
    }
}

impl ProtobufConvert for ExecutionStatus {
    type ProtoStruct = runtime_proto::ExecutionStatus;

    fn to_pb(&self) -> Self::ProtoStruct {
        let mut inner = Self::ProtoStruct::default();
        match &self.0 {
            Result::Ok(_) => inner.set_ok(protobuf::well_known_types::Empty::new()),
            Result::Err(e) => inner.set_error(e.to_pb()),
        }
        inner
    }

    fn from_pb(mut pb: Self::ProtoStruct) -> Result<Self, failure::Error> {
        let inner = if pb.has_error() {
            ensure!(!pb.has_ok(), "ExecutionStatus has both of variants.");
            Err(ExecutionError::from_pb(pb.take_error())?)
        } else {
            Ok(())
        };
        Ok(Self(inner))
    }
}

impl ObjectHash for ExecutionStatus {
    fn object_hash(&self) -> Hash {
        match &self.0 {
            Err(e) => e.object_hash(),
            Ok(_) => Hash::zero(),
        }
    }
}
