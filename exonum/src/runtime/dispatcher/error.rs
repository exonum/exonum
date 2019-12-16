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

//! The set of errors for the Dispatcher module.

use exonum_derive::ExecutionFail;

use std::fmt::Display;

use crate::runtime::{ErrorKind, ExecutionError, ExecutionFail};

/// List of possible dispatcher errors.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[derive(ExecutionFail)]
#[execution_fail(crate = "crate", kind = "dispatcher")]
pub enum Error {
    /// Runtime identifier is incorrect in this context.
    IncorrectRuntime = 0,
    /// Artifact identifier is unknown.
    UnknownArtifactId = 1,
    /// Artifact with the given identifier is already deployed.
    ArtifactAlreadyDeployed = 2,
    /// Artifact with the given identifier is not deployed.
    ArtifactNotDeployed = 3,
    /// Specified service name is already used.
    ServiceNameExists = 4,
    /// Specified service identifier is already used.
    ServiceIdExists = 5,
    /// Specified service is not active.
    ServiceNotActive = 6,
    /// Suitable runtime for the given service instance ID is not found.
    IncorrectInstanceId = 7,
    /// The interface is absent in the service.
    NoSuchInterface = 8,
    /// The method is absent in the service interface.
    NoSuchMethod = 9,
    /// Maximum depth of the call stack has been reached.
    StackOverflow = 10,
    /// This caller is not authorized to call this method.
    UnauthorizedCaller = 11,
    /// Malformed arguments for calling a service interface method.
    MalformedArguments = 12,
    /// Service instance is already transitioning to a new status.
    ServicePending = 13,
}

impl Error {
    /// Creates a `MalformedArguments` error with the user-provided error cause.
    /// The cause does not need to include the error location; this information is added
    /// by the framework automatically.
    pub fn malformed_arguments(cause: impl Display) -> ExecutionError {
        let description = format!(
            "Malformed arguments for calling a service interface method: {}",
            cause
        );
        Error::MalformedArguments.with_description(description)
    }

    pub(crate) fn stack_overflow(max_depth: usize) -> ExecutionError {
        let description = format!(
            "Maximum depth of call stack ({}) has been reached.",
            max_depth
        );
        ExecutionError::new(
            ErrorKind::Dispatcher {
                code: Error::StackOverflow as u8,
            },
            description,
        )
    }
}
