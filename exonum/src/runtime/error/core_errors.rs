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

//! The set of errors for the Dispatcher module.

use exonum_derive::ExecutionFail;

use crate::runtime::{ErrorKind, ExecutionError};

/// List of possible core errors.
///
/// Note that in most cases you don't need to spawn a core error,
/// unless your service is providing some wrapper for core logic and
/// should behave just like core.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[derive(ExecutionFail)]
#[execution_fail(crate = "crate", kind = "core")]
pub enum CoreError {
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
    /// Maximum depth of the call stack has been reached.
    StackOverflow = 8,
    /// Service instance is already transitioning to a new status.
    ServicePending = 9,
}

impl CoreError {
    pub(crate) fn stack_overflow(max_depth: usize) -> ExecutionError {
        let description = format!(
            "Maximum depth of call stack ({}) has been reached.",
            max_depth
        );
        ExecutionError::new(
            ErrorKind::Core {
                code: CoreError::StackOverflow as u8,
            },
            description,
        )
    }
}
