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

use exonum_derive::*;

/// Common errors emitted by transactions during execution.
///
/// Errors are divided into sub-groups by the corresponding error codes ranges:
/// - 0 - 31: Common `Supervisor` errors.
/// - 32 - 63: Errors related to artifacts.
/// - 64 - 95: Errors related to service instances.
/// - 96 - 128: Errors related to configuration changes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[derive(ExecutionFail)]
pub enum Error {
    // # General errors that can occur within supervisor interaction.
    // Error codes 0-31.
    // ------------------
    /// Transaction author is not a validator.
    UnknownAuthor = 0,
    /// Deadline exceeded for the current transaction.
    DeadlineExceeded = 1,
    /// Actual height for transaction is in the past.
    ActualFromIsPast = 2,

    // # Artifact-related errors group.
    // Error codes 32-63.
    // ------------------
    /// Artifact has been already deployed.
    AlreadyDeployed = 32,
    /// Artifact identifier has incorrect format.
    InvalidArtifactId = 33,
    /// Deploy request has been already registered.
    DeployRequestAlreadyRegistered = 34,
    /// Deploy request has not been registered or accepted.
    DeployRequestNotRegistered = 35,
    /// Start request contains unknown artifact.
    UnknownArtifact = 36,

    // # Instance-related errors group.
    // Error codes 64-95.
    // ------------------
    /// Instance with the given name already exists.
    InstanceExists = 64,
    /// Instance name is incorrect.
    InvalidInstanceName = 65,

    // # Configuration-related errors group.
    // Error codes 96-127.
    // -------------------
    /// Active configuration change proposal already exists.
    ConfigProposeExists = 96,
    /// Malformed configuration change proposal.
    MalformedConfigPropose = 97,
    /// This configuration change proposal is not registered.
    ConfigProposeNotRegistered = 98,
    /// Transaction author attempts to vote twice.
    AttemptToVoteTwice = 99,
    /// Incorrect configuration number.
    IncorrectConfigurationNumber = 100,
    /// Invalid configuration for supervisor.
    InvalidConfig = 101,
}
