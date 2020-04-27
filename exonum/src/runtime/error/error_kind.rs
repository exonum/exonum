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

use anyhow::ensure;

use std::fmt::Display;

use crate::proto::schema::errors as errors_proto;

/// Kind of execution error, divided into several distinct sub-groups.
///
/// Note that kind of error **does not** specify the source from which error originates.
/// This kind of information is available from [`ExecutionError`] via `runtime_id` and `call_site`
/// methods.
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
/// ## `Core` errors
///
/// Use `Core` kind only if you should mimic a core behavior, e.g. when proxying
/// requests and the behavior should be the same as if the action was performed by
/// core. In most cases you **don't need** to use `Core` type of errors.
/// See [`CoreError`] for more details.
///
/// ## `Common` errors
///
/// `Common` errors set provides various error codes that can occur within `Runtime`
/// and `Service` lifecycle. They are intended to be reused in the service and runtime code instead
/// of defining new error codes with the same effect. See [`CommonError`] for more details.
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
/// [`ExecutionError`]: struct.ExecutionError.html
/// [`catch_unwind`]: https://doc.rust-lang.org/std/panic/fn.catch_unwind.html
/// [`CoreError`]: enum.CoreError.html
/// [`CommonError`]: enum.CommonError.html
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum ErrorKind {
    /// An unexpected error that has occurred in the service code.
    ///
    /// Unlike [`Service`](#variant.Service) errors, unexpected errors do not have a user-defined code.
    /// Thus, it may be impossible for users to figure out the precise cause of the error;
    /// they can only use the accompanying error description.
    Unexpected,

    /// A common error which can occur in different contexts.
    Common {
        /// Well-known error code. Available values can be found in the [description] of core errors
        ///
        /// [description]: enum.CommonError.html
        code: u8,
    },

    /// An error in the core code. For example, stack overflow caused by recursive service calls.
    Core {
        /// Error code. Available values can be found in the [description] of core errors.
        ///
        /// [description]: enum.CoreError.html
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
    pub(super) fn into_raw(self) -> (errors_proto::ErrorKind, u8) {
        match self {
            Self::Unexpected => (errors_proto::ErrorKind::UNEXPECTED, 0),
            Self::Common { code } => (errors_proto::ErrorKind::COMMON, code),
            Self::Core { code } => (errors_proto::ErrorKind::CORE, code),
            Self::Runtime { code } => (errors_proto::ErrorKind::RUNTIME, code),
            Self::Service { code } => (errors_proto::ErrorKind::SERVICE, code),
        }
    }

    pub(super) fn from_raw(kind: errors_proto::ErrorKind, code: u8) -> anyhow::Result<Self> {
        use errors_proto::ErrorKind::*;

        let kind = match kind {
            UNEXPECTED => {
                ensure!(code == 0, "Error code for panic should be zero");
                Self::Unexpected
            }
            COMMON => Self::Common { code },
            CORE => Self::Core { code },
            RUNTIME => Self::Runtime { code },
            SERVICE => Self::Service { code },
        };
        Ok(kind)
    }
}

impl Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unexpected => f.write_str("unexpected"),
            Self::Common { code } => write!(f, "common:{}", code),
            Self::Core { code } => write!(f, "core:{}", code),
            Self::Runtime { code } => write!(f, "runtime:{}", code),
            Self::Service { code } => write!(f, "service:{}", code),
        }
    }
}
