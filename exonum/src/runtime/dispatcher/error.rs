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

use std::fmt;

use super::{ErrorKind, ExecutionError};

/// List of possible dispatcher errors.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
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
    /// Specified service is not started.
    ServiceNotStarted = 6,
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
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        use self::Error::*;

        formatter.write_str(match self {
            IncorrectRuntime => "Runtime identifier is incorrect in this context",
            UnknownArtifactId => "Artifact identifier is unknown",
            ArtifactAlreadyDeployed => "Artifact with the given identifier is already deployed",
            ArtifactNotDeployed => "Artifact with the given identifier is not deployed",
            ServiceNameExists => "Specified service name is already used",
            ServiceIdExists => "Specified service identifier is already used",
            ServiceNotStarted => "Specified service is not started",
            IncorrectInstanceId => {
                "Suitable runtime for the given service instance ID is not found"
            }
            NoSuchInterface => "The interface is absent in the service",
            NoSuchMethod => "The method is absent in the service interface",
            StackOverflow => "Maximum depth of the call stack has been reached",
            UnauthorizedCaller => "This caller is not authorized to call this method",
            MalformedArguments => "Malformed arguments for calling a service interface method",
        })
    }
}

impl From<Error> for ErrorKind {
    fn from(error: Error) -> Self {
        ErrorKind::dispatcher(error as u8)
    }
}

impl From<Error> for ExecutionError {
    fn from(error: Error) -> Self {
        ExecutionError::new(error.into(), error.to_string())
    }
}

impl Error {
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
