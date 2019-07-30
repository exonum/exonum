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

/// List of possible dispatcher errors.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, IntoExecutionError)]
#[exonum(crate = "crate", kind = "dispatcher")]
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
    /// Specified service does not started.
    ServiceNotStarted = 6,
    /// Not found a suitable runtime for the given instance id.
    IncorrectInstanceId = 7,
}
