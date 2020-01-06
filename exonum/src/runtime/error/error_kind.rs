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

//! Internal raw error representation.

use failure::ensure;

use std::{convert::TryFrom, fmt::Display};

use crate::proto::schema::runtime as runtime_proto;

/// Code of execution error.
///
/// Code can be either a well-known code from the list provided by Exonum core, or
/// a custom code specific to the environment that produced the error.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// Common error with the code described in `CommonError` enum. // TODO add link.
    Common(u8),
    /// Custom error specific to the core/runtime/service.
    Custom(u8),
}

impl ErrorCode {
    const CUSTOM_ERROR_OFSET: u16 = 256;
    const CUSTOM_ERROR_MAX: u16 = 512;

    pub(super) fn into_raw(self) -> u16 {
        match self {
            ErrorCode::Common(code) => code as u16,
            ErrorCode::Custom(code) => (code as u16) + ErrorCode::CUSTOM_ERROR_OFSET,
        }
    }

    pub(super) fn from_raw(raw_code: u16) -> Result<Self, failure::Error> {
        ensure!(
            raw_code < ErrorCode::CUSTOM_ERROR_MAX,
            "Incorrect raw code: {:?}",
            raw_code
        );

        let code = if raw_code < ErrorCode::CUSTOM_ERROR_OFSET {
            let code = u8::try_from(raw_code).unwrap();
            ErrorCode::Common(code)
        } else {
            let code = u8::try_from(raw_code - ErrorCode::CUSTOM_ERROR_OFSET).unwrap();
            ErrorCode::Custom(code)
        };

        Ok(code)
    }
}

impl Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorCode::Common(code) => write!(f, "common:{}", code),
            ErrorCode::Custom(code) => write!(f, "custom:{}", code),
        }
    }
}

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
        code: ErrorCode,
    },

    /// An error in the runtime logic. For example, the runtime could not compile an artifact.
    Runtime {
        /// Runtime-specific error code.
        /// Error codes can have different meanings for different runtimes.
        code: ErrorCode,
    },

    /// An error in the service code reported to the blockchain users.
    Service {
        /// User-defined error code.
        /// Error codes can have different meanings for different services.
        code: ErrorCode,
    },
}

impl ErrorKind {
    pub(super) fn into_raw(self) -> (runtime_proto::ErrorKind, u16) {
        match self {
            ErrorKind::Unexpected => (runtime_proto::ErrorKind::UNEXPECTED, 0),
            ErrorKind::Dispatcher { code } => {
                (runtime_proto::ErrorKind::DISPATCHER, code.into_raw())
            }
            ErrorKind::Runtime { code } => (runtime_proto::ErrorKind::RUNTIME, code.into_raw()),
            ErrorKind::Service { code } => (runtime_proto::ErrorKind::SERVICE, code.into_raw()),
        }
    }

    pub(super) fn from_raw(
        kind: runtime_proto::ErrorKind,
        code: u16,
    ) -> Result<Self, failure::Error> {
        use runtime_proto::ErrorKind::*;
        let kind = match kind {
            UNEXPECTED => {
                ensure!(code == 0, "Error code for panic should be zero");
                ErrorKind::Unexpected
            }
            DISPATCHER => ErrorKind::Dispatcher {
                code: ErrorCode::from_raw(code)?,
            },
            RUNTIME => ErrorKind::Runtime {
                code: ErrorCode::from_raw(code)?,
            },
            SERVICE => ErrorKind::Service {
                code: ErrorCode::from_raw(code)?,
            },
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
