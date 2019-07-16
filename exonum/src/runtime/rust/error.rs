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

//! The set of specific for the Rust runtime implementation errors.

use crate::runtime::error::{ErrorKind, ExecutionError};

/// Result of unsuccessful transaction execution.
///
/// A transaction error consists of an error code and optional description.
/// The error code affects the blockchain state hash, while the description does not.
/// Therefore descriptions are mostly used for developer purposes, not for interaction of
/// the system with users.
///
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TransactionError {
    /// User-defined error code. Error codes can have different meanings for the
    /// different transactions and services.
    pub code: u8,
    /// Optional error description.
    pub description: Option<String>,
}

impl TransactionError {
    /// Constructs a new error instance with the given error code.
    pub fn new(code: u8) -> Self {
        Self {
            code,
            description: None,
        }
    }

    /// Constructs a new error instance with the given error code and description.
    pub fn with_description(code: u8, description: impl Into<String>) -> Self {
        Self {
            code,
            description: Some(description.into()),
        }
    }
}

impl From<TransactionError> for ExecutionError {
    fn from(inner: TransactionError) -> Self {
        ExecutionError {
            kind: ErrorKind::service(inner.code),
            description: inner.description.unwrap_or_default(),
        }
    }
}

/// List of possible Rust runtime errors.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, IntoExecutionError)]
#[exonum(crate = "crate", kind = "runtime")]
pub enum Error {
    /// Unable to parse artifact identifier or specified artifact has non-empty spec.
    IncorrectArtifactId = 0,
    /// Unable to deploy artifact with the specified identifier, it is not listed in available artifacts.
    UnableToDeploy = 1,
    /// Unable to parse service configuration.
    ConfigParseError = 2,
    /// Unspecified error during the call invocation.
    UnspecifiedError = 3,
}
