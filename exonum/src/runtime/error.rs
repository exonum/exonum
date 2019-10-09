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

use exonum_merkledb::{BinaryValue, ObjectHash};
use exonum_proto::ProtobufConvert;
use protobuf::Message;

use std::{any::Any, convert::TryFrom, fmt::Display, panic};

use crate::{
    blockchain::FatalError,
    crypto::{self, Hash},
    proto::schema::runtime,
};

/// Kind of execution error, indicates in which module error occurred.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ErrorKind {
    /// Operation execution has been finished with panic.
    Panic,
    /// An error in dispatcher during the execution occurred.
    Dispatcher {
        /// Error code, available values ​​can be found in the [description] of the dispatcher's errors.
        ///
        /// [description]: ../dispatcher/error/enum.Error.html
        code: u8,
    },
    /// An error in the runtime occurred.
    Runtime {
        /// User-defined error code.
        /// Error codes can have different meanings for the different runtimes.
        code: u8,
    },
    /// An error during the service's transaction execution occurred.
    Service {
        /// User-defined error code.
        /// Error codes can have different meanings for the different transactions
        /// and services.
        code: u8,
    },
}

impl ErrorKind {
    /// Creates panic error.
    pub fn panic() -> Self {
        ErrorKind::Panic
    }

    /// Creates dispatcher error with the specified code.
    pub fn dispatcher(code: impl Into<u8>) -> Self {
        ErrorKind::Dispatcher { code: code.into() }
    }

    /// Creates runtime error with the specified code.
    pub fn runtime(code: impl Into<u8>) -> Self {
        ErrorKind::Runtime { code: code.into() }
    }

    /// Creates service error with the specified code.
    pub fn service(code: impl Into<u8>) -> Self {
        ErrorKind::Service { code: code.into() }
    }

    fn into_raw(self) -> (u8, u8) {
        match self {
            ErrorKind::Panic => (0, 0),
            ErrorKind::Dispatcher { code } => (1, code),
            ErrorKind::Runtime { code } => (2, code),
            ErrorKind::Service { code } => (3, code),
        }
    }

    fn from_raw(kind: u8, code: u8) -> Result<Self, failure::Error> {
        match kind {
            0 => {
                ensure!(code == 0, "Error code for panic should be zero");
                Ok(ErrorKind::Panic)
            }
            1 => Ok(ErrorKind::Dispatcher { code }),
            2 => Ok(ErrorKind::Runtime { code }),
            3 => Ok(ErrorKind::Service { code }),
            _ => bail!("Unknown error kind"),
        }
    }
}

impl Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorKind::Panic => f.write_str("panic"),
            ErrorKind::Dispatcher { code } => write!(f, "dispatcher:{}", code),
            ErrorKind::Runtime { code } => write!(f, "runtime:{}", code),
            ErrorKind::Service { code } => write!(f, "service:{}", code),
        }
    }
}

/// Result of unsuccessful runtime execution.
///
/// An execution error consists of an error kind and optional description.
/// The error code affects the blockchain state hash, while the description does not.
/// Therefore descriptions are mostly used for developer purposes, not for interaction of
/// the system with users.
///
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Fail)]
pub struct ExecutionError {
    /// The kind of error that indicates in which module and with which code the error occurred.
    pub kind: ErrorKind,
    /// Optional description which doesn't affect `object_hash`.
    pub description: String,
}

/// Invokes closure, capturing the cause of the unwinding panic if one occurs.
///
/// This function will return the result of the closure if the closure does not panic.
/// If the closure panics, it returns `Err(ExecutionError::panic(cause))`.
/// This function does not catch `FatalError` panics.
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
    /// Creates a new execution error instance with the specified error kind and an optional description.
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
        let description = {
            // Strings
            if let Some(s) = any.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = any.downcast_ref::<String>() {
                s.clone()
            }
            // std::error::Error
            else if let Some(error) = any.downcast_ref::<Box<(dyn std::error::Error + Send)>>() {
                error.description().to_string()
            }
            // Failure errors
            else if let Some(error) = any.downcast_ref::<failure::Error>() {
                error.to_string()
            }
            // Other
            else {
                String::new()
            }
        };
        Self {
            kind: ErrorKind::Panic,
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

impl<E, T> From<(E, T)> for ExecutionError
where
    T: Display,
    E: Into<ErrorKind>,
{
    fn from(inner: (E, T)) -> Self {
        Self::new(inner.0.into(), inner.1.to_string())
    }
}

impl ProtobufConvert for ExecutionError {
    type ProtoStruct = runtime::ExecutionError;

    fn to_pb(&self) -> Self::ProtoStruct {
        let mut inner = Self::ProtoStruct::default();
        let (kind, code) = self.kind.into_raw();
        inner.set_kind(u32::from(kind));
        inner.set_code(u32::from(code));
        inner.set_description(self.description.clone());
        inner
    }

    fn from_pb(mut pb: Self::ProtoStruct) -> Result<Self, failure::Error> {
        let kind = u8::try_from(pb.get_kind())?;
        let code = u8::try_from(pb.get_code())?;
        Ok(Self {
            kind: ErrorKind::from_raw(kind, code)?,
            description: pb.take_description(),
        })
    }
}

impl BinaryValue for ExecutionError {
    fn to_bytes(&self) -> Vec<u8> {
        self.to_pb()
            .write_to_bytes()
            .expect("Failed to serialize in BinaryValue for ExecutionError")
    }

    fn from_bytes(value: std::borrow::Cow<[u8]>) -> Result<Self, failure::Error> {
        let mut inner = <Self as ProtobufConvert>::ProtoStruct::new();
        inner.merge_from_bytes(value.as_ref())?;
        ProtobufConvert::from_pb(inner)
    }
}

// String content (`ExecutionError::description`) is intentionally excluded from the hash
// calculation because user can be tempted to use error description from a third-party libraries
// which aren't stable across the versions.
impl ObjectHash for ExecutionError {
    fn object_hash(&self) -> Hash {
        let (kind, code) = self.kind.into_raw();
        crypto::hash(&[kind, code])
    }
}

/// Returns an status of the dispatcher execution.
/// This result may be either an empty unit type, in case of success,
/// or an `ExecutionError`, if execution has failed.
/// Errors consist of an error code and an optional description.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    type ProtoStruct = runtime::ExecutionStatus;

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

impl BinaryValue for ExecutionStatus {
    fn to_bytes(&self) -> Vec<u8> {
        self.to_pb()
            .write_to_bytes()
            .expect("Failed to serialize in BinaryValue for ExecutionStatus")
    }

    fn from_bytes(value: std::borrow::Cow<[u8]>) -> Result<Self, failure::Error> {
        let mut inner = <Self as ProtobufConvert>::ProtoStruct::new();
        inner.merge_from_bytes(value.as_ref())?;
        ProtobufConvert::from_pb(inner)
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

    use super::{ErrorKind, ExecutionError};

    #[serde(tag = "type", rename_all = "snake_case")]
    #[derive(Debug, Serialize, Deserialize)]
    enum ExecutionStatus {
        Success,
        Panic { description: String },
        DispatcherError { description: String, code: u8 },
        RuntimeError { description: String, code: u8 },
        ServiceError { description: String, code: u8 },
    }

    impl From<&Result<(), ExecutionError>> for ExecutionStatus {
        fn from(inner: &Result<(), ExecutionError>) -> Self {
            if let Err(err) = &inner {
                let description = err.description.clone();
                match err.kind {
                    ErrorKind::Panic => ExecutionStatus::Panic { description },
                    ErrorKind::Dispatcher { code } => {
                        ExecutionStatus::DispatcherError { code, description }
                    }
                    ErrorKind::Runtime { code } => {
                        ExecutionStatus::RuntimeError { code, description }
                    }
                    ErrorKind::Service { code } => {
                        ExecutionStatus::ServiceError { code, description }
                    }
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
                    Err(ExecutionError::new(ErrorKind::Panic, description))
                }
                ExecutionStatus::DispatcherError { description, code } => Err(ExecutionError::new(
                    ErrorKind::Dispatcher { code },
                    description,
                )),
                ExecutionStatus::RuntimeError { description, code } => Err(ExecutionError::new(
                    ErrorKind::Runtime { code },
                    description,
                )),
                ExecutionStatus::ServiceError { description, code } => Err(ExecutionError::new(
                    ErrorKind::Service { code },
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
    use std::panic;

    use super::*;

    fn make_panic<T: Send + 'static>(val: T) -> Box<dyn Any + Send> {
        panic::catch_unwind(panic::AssertUnwindSafe(|| panic!(val))).unwrap_err()
    }

    #[test]
    fn execution_error_binary_value_round_trip() {
        let values = vec![
            (ErrorKind::Panic, "AAAA"),
            (ErrorKind::Dispatcher { code: 0 }, ""),
            (ErrorKind::Dispatcher { code: 0 }, "b"),
            (ErrorKind::Runtime { code: 1 }, "c"),
            (ErrorKind::Service { code: 18 }, "ddc"),
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
    fn execution_error_binary_value_wrong_kind() {
        let bytes = {
            let mut inner = runtime::ExecutionError::default();
            inner.set_kind(117);
            inner.set_code(2);
            inner.write_to_bytes().unwrap()
        };

        assert_eq!(
            ExecutionError::from_bytes(bytes.into())
                .unwrap_err()
                .to_string(),
            "Unknown error kind"
        )
    }

    #[test]
    fn execution_error_binary_value_panic_with_code() {
        let bytes = {
            let mut inner = runtime::ExecutionError::default();
            inner.set_kind(0);
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
            kind: ErrorKind::Service { code: 5 },
            description: "foo".to_owned(),
        };

        let second_err = ExecutionError {
            kind: ErrorKind::Service { code: 5 },
            description: "foo bar".to_owned(),
        };

        assert_eq!(first_err.object_hash(), second_err.object_hash());
    }

    #[test]
    fn execution_result_serde_roundtrip() {
        let values = vec![
            Err((ErrorKind::Panic, "AAAA")),
            Err((ErrorKind::Dispatcher { code: 0 }, "")),
            Err((ErrorKind::Dispatcher { code: 0 }, "b")),
            Err((ErrorKind::Runtime { code: 1 }, "c")),
            Err((ErrorKind::Service { code: 18 }, "ddc")),
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
