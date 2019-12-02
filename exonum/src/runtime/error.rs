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

//! The set of errors for the Runtime module.

use byteorder::{ByteOrder, LittleEndian};
use exonum_derive::*;
use exonum_merkledb::ObjectHash;
use exonum_proto::ProtobufConvert;

use std::{any::Any, convert::TryFrom, fmt::Display, panic};

use super::InstanceId;
use crate::{
    blockchain::FatalError,
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
/// [`DispatcherError`]: ../enum.DispatcherError.html
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
        /// [description]: ../enum.DispatcherError.html
        code: u8,
    },

    /// An error in the runtime logic. For example, the runtime could not compile an artifact.
    Runtime {
        /// Identifier of the runtime that has raised the error.
        runtime_id: u32,
        /// Runtime-specific error code.
        /// Error codes can have different meanings for different runtimes.
        code: u8,
    },

    /// An error in the service code reported to the blockchain users.
    Service {
        /// Identifier of the service that has generated the error. If necessary, clients may use
        /// the dispatcher schema or the `services` endpoint of the node API to find out
        /// the artifact / version of the service by this ID.
        instance_id: InstanceId,
        /// User-defined error code.
        /// Error codes can have different meanings for different services.
        code: u8,
    },
}

impl ErrorKind {
    /// Creates an unexpected error.
    pub fn unexpected() -> Self {
        ErrorKind::Unexpected
    }

    /// Creates a dispatcher error with the specified code.
    pub(crate) fn dispatcher(code: u8) -> Self {
        ErrorKind::Dispatcher { code }
    }

    /// Creates a runtime error with the specified code.
    pub fn runtime(runtime_id: u32, code: u8) -> Self {
        ErrorKind::Runtime { code, runtime_id }
    }

    fn into_raw(self) -> (runtime_proto::ErrorKind, u8) {
        match self {
            ErrorKind::Unexpected => (runtime_proto::ErrorKind::UNEXPECTED, 0),
            ErrorKind::Dispatcher { code } => (runtime_proto::ErrorKind::DISPATCHER, code),
            ErrorKind::Runtime { code, .. } => (runtime_proto::ErrorKind::RUNTIME, code),
            ErrorKind::Service { code, .. } => (runtime_proto::ErrorKind::SERVICE, code),
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
            RUNTIME => ErrorKind::Runtime {
                code,
                runtime_id: 0,
            },
            SERVICE => ErrorKind::Service {
                code,
                instance_id: 0,
            },
        };
        Ok(kind)
    }
}

impl Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorKind::Unexpected => f.write_str("panic"),
            ErrorKind::Dispatcher { code } => write!(f, "dispatcher:{}", code),
            ErrorKind::Runtime { code, runtime_id } => write!(f, "runtime#{}:{}", runtime_id, code),
            ErrorKind::Service { code, instance_id } => {
                write!(f, "service#{}:{}", instance_id, code)
            }
        }
    }
}

/// Result of unsuccessful runtime execution.
///
/// An execution error consists of an error kind and optional description.
/// The error kind affects the blockchain state hash, while the description does not.
/// Therefore descriptions are mostly used for developer purposes, not for interaction of
/// the system with users.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Fail, BinaryValue)]
pub struct ExecutionError {
    /// The kind of error that indicates in which module and with which code the error occurred.
    pub kind: ErrorKind,
    /// Optional description which doesn't affect `object_hash`.
    pub description: String,
}

/// Trait representing an error type defined in the service code.
///
/// This trait can be derived from an enum using an eponymous derive macro from the `exonum-derive`
/// crate. Using an error with the [`CallContext::err`] method is the preferred way to generate
/// errors in the Rust services.
///
/// # Examples
///
/// ```
/// use exonum_derive::*;
/// # use exonum::runtime::{rust::CallContext, ExecutionError};
///
/// /// Error codes emitted by wallet transactions during execution:
/// #[derive(Debug, ServiceFail)]
/// pub enum Error {
///     /// Content hash already exists.
///     HashAlreadyExists = 0,
///     /// Unable to parse the service configuration.
///     ConfigParseError = 1,
///     /// Time service with the specified name does not exist.
///     TimeServiceNotFound = 2,
/// }
///
/// // Using errors in the service code:
/// # struct Arg { field: String }
/// # struct MyService;
/// # trait MyInterface {
/// #     fn do_something(&self, context: CallContext<'_>, arg: Arg) -> Result<(), ExecutionError>;
/// # }
/// impl MyInterface for MyService {
///     fn do_something(
///         &self,
///         context: CallContext<'_>,
///         arg: Arg,
///     ) -> Result<(), ExecutionError> {
///         if arg.field.is_empty() {
///             return Err(context.err(Error::ConfigParseError));
///         }
///         // do other stuff...
/// #       Ok(())
///     }
/// }
/// ```
///
/// [`for_service`] method allows to use errors in testing:
///
/// ```no_run
/// use exonum::runtime::{ExecutionError, InstanceId, ServiceFail};
/// use exonum_derive::ServiceFail;
/// # use exonum::explorer::BlockWithTransactions;
/// # struct Tx;
/// # struct TestKit;
/// # impl TestKit {
/// #     fn create_block_with_transaction(&mut self, tx: Tx)
/// #         -> BlockWithTransactions { unimplemented!() }
/// # }
///
/// #[derive(Debug, ServiceFail)]
/// pub enum Error {
///     /// Content hash already exists.
///     HashAlreadyExists = 0,
///     // other variants...
/// }
///
/// // Identifier of the service that will cause an error.
/// const SERVICE_ID: InstanceId = 100;
///
/// let mut testkit: TestKit = // ...
/// #    TestKit;
/// let tx = // ...
/// #    Tx;
/// let block = testkit.create_block_with_transaction(tx);
/// let err: &ExecutionError = block[0].status().unwrap_err();
/// assert_eq!(*err, Error::HashAlreadyExists.for_service(SERVICE_ID));
/// ```
///
/// [`CallContext::err`]: ../rust/struct.CallContext.html#method.err
/// [`for_service`]: #tymethod.for_service
pub trait ServiceFail {
    /// Extracts the error code.
    fn code(&self) -> u8;

    /// Extracts the human-readable error description.
    fn description(self) -> String;

    /// Creates an error with the externally provided description. The output value implements
    /// `ServiceFail` and thus can be used to create errors during service execution.
    ///
    /// This operation is not meant to be overridden.
    fn with_description(&self, description: impl Display) -> (u8, String) {
        (self.code(), description.to_string())
    }

    /// Converts an error into a generic representation. This is primarily useful to compare
    /// an error via `assert_eq!` in tests.
    ///
    /// This operation is not meant to be overridden.
    fn for_service(self, instance_id: InstanceId) -> ServiceExecutionError
    where
        Self: Sized,
    {
        ServiceExecutionError {
            code: self.code(),
            instance_id,
            description: self.description(),
        }
    }
}

impl ServiceFail for (u8, &str) {
    fn code(&self) -> u8 {
        self.0
    }

    fn description(self) -> String {
        self.1.to_owned()
    }
}

impl ServiceFail for (u8, String) {
    fn code(&self) -> u8 {
        self.0
    }

    fn description(self) -> String {
        self.1
    }
}

/// Generalized form of `ServiceFail` produced by the `for_service` method. Can be compared
/// to `ExecutionError`.
#[derive(Debug, PartialEq)]
pub struct ServiceExecutionError {
    code: u8,
    instance_id: InstanceId,
    description: String,
}

impl PartialEq<ServiceExecutionError> for ExecutionError {
    fn eq(&self, other: &ServiceExecutionError) -> bool {
        if let ErrorKind::Service { code, instance_id } = self.kind {
            code == other.code
                && instance_id == other.instance_id
                && self.description == other.description
        } else {
            false
        }
    }
}

impl PartialEq<ExecutionError> for ServiceExecutionError {
    fn eq(&self, other: &ExecutionError) -> bool {
        other.eq(self)
    }
}

/// Invokes closure, capturing the cause of the unwinding panic if one occurs.
///
/// This function will return the result of the closure if the closure does not panic.
/// If the closure panics, it returns an `Unexpected` error with the description derived
/// from the panic object.
///
/// `FatalError`s are not caught by this method.
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
            if panic.is::<FatalError>() {
                // Continue panic unwinding if the reason is FatalError.
                panic::resume_unwind(panic);
            }
            Err(ExecutionError::from_panic(panic))
        }
        // Normal execution.
        Ok(Ok(value)) => Ok(value),
    }
}

impl ExecutionError {
    /// Creates a new execution error instance with the specified error kind
    /// and an optional description.
    pub fn new(kind: ErrorKind, description: impl Into<String>) -> Self {
        Self {
            kind,
            description: description.into(),
        }
    }

    /// Creates an execution error from the panic description.
    pub(crate) fn from_panic(any: impl AsRef<(dyn Any + Send)>) -> Self {
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

        Self {
            kind: ErrorKind::Unexpected,
            description,
        }
    }
}

impl Display for ExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "An execution error `{}` occurred with description: {}",
            self.kind, self.description
        )
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
        match self.kind {
            ErrorKind::Service { instance_id, .. } => inner.set_instance_id(instance_id),
            ErrorKind::Runtime { runtime_id, .. } => inner.set_runtime_id(runtime_id),
            _ => { /* No additional actions required */ }
        }
        inner
    }

    #[rustfmt::skip] // Formatter mangles `match` operator.
    fn from_pb(mut pb: Self::ProtoStruct) -> Result<Self, failure::Error> {
        let kind = pb.get_kind();
        let code = u8::try_from(pb.get_code())?;
        let mut kind = ErrorKind::from_raw(kind, code)?;

        match kind {
            ErrorKind::Service { ref mut instance_id, .. } => {
                *instance_id = pb.get_instance_id();
            }
            ErrorKind::Runtime { ref mut runtime_id, .. } => {
                *runtime_id = pb.get_runtime_id();
            }
            _ => { /* No additional actions required */ }
        }

        Ok(Self {
            kind,
            description: pb.take_description(),
        })
    }
}

// String content (`ExecutionError::description`) is intentionally excluded from the hash
// calculation because user can be tempted to use error description from a third-party libraries
// which aren't stable across the versions.
impl ObjectHash for ExecutionError {
    fn object_hash(&self) -> Hash {
        let (kind, code) = self.kind.into_raw();
        if let ErrorKind::Service { instance_id, .. } = self.kind {
            let mut buffer = [0; 6];
            buffer[0] = kind as u8;
            buffer[1] = code;
            LittleEndian::write_u32(&mut buffer[2..], instance_id);
            crypto::hash(&buffer)
        } else if let ErrorKind::Runtime { runtime_id, .. } = self.kind {
            let mut buffer = [0; 6];
            buffer[0] = kind as u8;
            buffer[1] = code;
            LittleEndian::write_u32(&mut buffer[2..], runtime_id);
            crypto::hash(&buffer)
        } else {
            crypto::hash(&[kind as u8, code])
        }
    }
}

/// Returns an status of the dispatcher execution.
/// This result may be either an empty unit type, in case of success,
/// or an `ExecutionError`, if execution has failed.
/// Errors consist of an error kind and an optional description.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, BinaryValue)]
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

/// More convenient serde layout for the `ExecutionResult`.
mod execution_result {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use super::{ErrorKind, ExecutionError, InstanceId};

    #[serde(tag = "type", rename_all = "snake_case")]
    #[derive(Debug, Serialize, Deserialize)]
    enum ExecutionStatus {
        Success,
        Panic {
            description: String,
        },
        DispatcherError {
            description: String,
            code: u8,
        },
        RuntimeError {
            description: String,
            code: u8,
            runtime_id: u32,
        },
        ServiceError {
            description: String,
            code: u8,
            instance_id: InstanceId,
        },
    }

    impl From<&Result<(), ExecutionError>> for ExecutionStatus {
        fn from(inner: &Result<(), ExecutionError>) -> Self {
            if let Err(err) = &inner {
                let description = err.description.clone();
                match err.kind {
                    ErrorKind::Unexpected => ExecutionStatus::Panic { description },
                    ErrorKind::Dispatcher { code } => {
                        ExecutionStatus::DispatcherError { code, description }
                    }
                    ErrorKind::Runtime { code, runtime_id } => ExecutionStatus::RuntimeError {
                        code,
                        runtime_id,
                        description,
                    },
                    ErrorKind::Service { code, instance_id } => ExecutionStatus::ServiceError {
                        code,
                        description,
                        instance_id,
                    },
                }
            } else {
                ExecutionStatus::Success
            }
        }
    }

    impl From<ExecutionStatus> for Result<(), ExecutionError> {
        fn from(inner: ExecutionStatus) -> Self {
            match inner {
                ExecutionStatus::Success => Ok(()),

                ExecutionStatus::Panic { description } => {
                    Err(ExecutionError::new(ErrorKind::Unexpected, description))
                }

                ExecutionStatus::DispatcherError { description, code } => Err(ExecutionError::new(
                    ErrorKind::Dispatcher { code },
                    description,
                )),

                ExecutionStatus::RuntimeError {
                    description,
                    code,
                    runtime_id,
                } => Err(ExecutionError::new(
                    ErrorKind::Runtime { code, runtime_id },
                    description,
                )),

                ExecutionStatus::ServiceError {
                    description,
                    code,
                    instance_id,
                } => Err(ExecutionError::new(
                    ErrorKind::Service { code, instance_id },
                    description,
                )),
            }
        }
    }

    pub fn serialize<S>(
        inner: &Result<(), ExecutionError>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ExecutionStatus::from(inner).serialize(serializer)
    }

    pub fn deserialize<'a, D>(deserializer: D) -> Result<Result<(), ExecutionError>, D::Error>
    where
        D: Deserializer<'a>,
    {
        ExecutionStatus::deserialize(deserializer).map(From::from)
    }
}

#[cfg(test)]
mod tests {
    use exonum_merkledb::BinaryValue;
    use protobuf::Message;
    use std::panic;

    use super::*;

    fn make_panic<T: Send + 'static>(val: T) -> Box<dyn Any + Send> {
        panic::catch_unwind(panic::AssertUnwindSafe(|| panic!(val))).unwrap_err()
    }

    #[test]
    fn execution_error_binary_value_round_trip() {
        let values = vec![
            (ErrorKind::Unexpected, "AAAA"),
            (ErrorKind::Dispatcher { code: 0 }, ""),
            (ErrorKind::Dispatcher { code: 0 }, "b"),
            (
                ErrorKind::Runtime {
                    runtime_id: 0,
                    code: 1,
                },
                "c",
            ),
            (
                ErrorKind::Service {
                    code: 18,
                    instance_id: 100,
                },
                "ddc",
            ),
        ];

        for (kind, description) in values {
            let err = ExecutionError {
                kind,
                description: description.to_owned(),
            };

            let bytes = err.to_bytes();
            let err2 = ExecutionError::from_bytes(bytes.into()).unwrap();
            assert_eq!(err, err2);
        }
    }

    #[test]
    fn execution_error_binary_value_unexpected_with_code() {
        let bytes = {
            let mut inner = runtime_proto::ExecutionError::default();
            inner.set_kind(runtime_proto::ErrorKind::UNEXPECTED);
            inner.set_code(2);
            inner.write_to_bytes().unwrap()
        };

        assert_eq!(
            ExecutionError::from_bytes(bytes.into())
                .unwrap_err()
                .to_string(),
            "Error code for panic should be zero"
        )
    }

    #[test]
    fn execution_error_object_hash_description() {
        let first_err = ExecutionError {
            kind: ErrorKind::Service {
                code: 5,
                instance_id: 100,
            },
            description: "foo".to_owned(),
        };
        let second_err = ExecutionError {
            kind: ErrorKind::Service {
                code: 5,
                instance_id: 100,
            },
            description: "foo bar".to_owned(),
        };
        assert_eq!(first_err.object_hash(), second_err.object_hash());

        let second_err = ExecutionError {
            kind: ErrorKind::Service {
                code: 5,
                instance_id: 101,
            },
            description: "foo".to_owned(),
        };
        assert_ne!(first_err.object_hash(), second_err.object_hash());
    }

    #[test]
    fn object_hash_for_runtime_errors() {
        let first_err = ExecutionError {
            kind: ErrorKind::Runtime {
                code: 5,
                runtime_id: 0,
            },
            description: "foo".to_owned(),
        };
        let second_err = ExecutionError {
            kind: ErrorKind::Runtime {
                code: 5,
                runtime_id: 1,
            },
            description: "foo bar".to_owned(),
        };
        assert_ne!(first_err.object_hash(), second_err.object_hash());

        let second_err = ExecutionError {
            kind: ErrorKind::Service {
                code: 5,
                instance_id: 1,
            },
            description: "foo bar".to_owned(),
        };
        assert_ne!(first_err.object_hash(), second_err.object_hash());

        let second_err = ExecutionError {
            kind: ErrorKind::Runtime {
                code: 6,
                runtime_id: 0,
            },
            description: "foo bar".to_owned(),
        };
        assert_ne!(first_err.object_hash(), second_err.object_hash());

        let second_err = ExecutionError {
            kind: ErrorKind::Runtime {
                code: 5,
                runtime_id: 0,
            },
            description: "Description doesn't matter".to_owned(),
        };
        assert_eq!(first_err.object_hash(), second_err.object_hash());
    }

    #[test]
    fn execution_result_serde_roundtrip() {
        let values = vec![
            Err((ErrorKind::Unexpected, "AAAA")),
            Err((ErrorKind::Dispatcher { code: 0 }, "")),
            Err((ErrorKind::Dispatcher { code: 0 }, "b")),
            Err((
                ErrorKind::Runtime {
                    runtime_id: 0,
                    code: 1,
                },
                "c",
            )),
            Err((
                ErrorKind::Service {
                    code: 18,
                    instance_id: 200,
                },
                "ddc",
            )),
            Ok(()),
        ];

        for value in values {
            let res = ExecutionStatus(value.map_err(|(kind, description)| ExecutionError {
                kind,
                description: description.to_owned(),
            }));
            let body = serde_json::to_string_pretty(&res).unwrap();
            let res2 = serde_json::from_str(&body).unwrap();
            assert_eq!(res, res2);
        }
    }

    #[test]
    fn str_panic() {
        let static_str = "Static string (&str)";
        let panic = make_panic(static_str);
        assert_eq!(ExecutionError::from_panic(panic).description, static_str);
    }

    #[test]
    fn string_panic() {
        let string = "Owned string (String)".to_owned();
        let panic = make_panic(string.clone());
        assert_eq!(ExecutionError::from_panic(panic).description, string);
    }

    #[test]
    fn box_error_panic() {
        let error: Box<dyn std::error::Error + Send> = Box::new("e".parse::<i32>().unwrap_err());
        let description = error.description().to_owned();
        let panic = make_panic(error);
        assert_eq!(ExecutionError::from_panic(panic).description, description);
    }

    #[test]
    fn failure_panic() {
        let error = format_err!("Failure panic");
        let description = error.to_string().to_owned();
        let panic = make_panic(error);
        assert_eq!(ExecutionError::from_panic(panic).description, description);
    }

    #[test]
    fn unknown_panic() {
        let panic = make_panic(1);
        assert_eq!(ExecutionError::from_panic(panic).description, "");
    }
}
