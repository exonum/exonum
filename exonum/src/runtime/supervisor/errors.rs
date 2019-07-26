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

/// Common errors emitted by transactions during execution.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, IntoExecutionError)]
#[exonum(crate = "crate")]
pub enum Error {
    /// Artifact has been already deployed.
    AlreadyDeployed = 0,
    /// Transaction author is not a validator.
    UnknownAuthor = 1,
    /// Deadline exceeded for the current transaction.
    DeadlineExceeded = 2,
    /// Instance with the given name already exists.
    InstanceExists = 3,
    /// Deploy request has been already registered.
    DeployRequestAlreadyRegistered = 4,
    /// Deploy request has not been registered.
    DeployRequestNotRegistered = 5,
    /// Artifact identifier has incorrect format.
    InvalidArtifactId = 6,
    /// Instance name is incorrect.
    InvalidInstanceName = 7,
}
