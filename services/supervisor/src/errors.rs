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

/// General errors that can occur within supervisor interaction.
/// Error codes 0-15.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[derive(ExecutionFail)]
pub enum CommonError {
    /// Deadline exceeded for the current transaction.
    DeadlineExceeded = 0,
    /// Actual height for transaction is in the past.
    ActualFromIsPast = 1,
}

/// Artifact-related errors group.
/// Error codes 16-31.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[derive(ExecutionFail)]
pub enum ArtifactError {
    /// Artifact has been already deployed.
    AlreadyDeployed = 16,
    /// Artifact identifier has incorrect format.
    InvalidArtifactId = 17,
    /// Deploy request has been already registered.
    DeployRequestAlreadyRegistered = 18,
    /// Deploy request has not been registered or accepted.
    DeployRequestNotRegistered = 19,
    /// Start request contains unknown artifact.
    UnknownArtifact = 20,
}

/// Instance-related errors group.
/// Error codes 32-47.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[derive(ExecutionFail)]
pub enum ServiceError {
    /// Instance with the given name already exists.
    InstanceExists = 32,
    /// Instance name is incorrect.
    InvalidInstanceName = 33,
}

/// Configuration-related errors group.
/// Error codes 48-63.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[derive(ExecutionFail)]
pub enum ConfigurationError {
    /// Active configuration change proposal already exists.
    ConfigProposeExists = 48,
    /// Malformed configuration change proposal.
    MalformedConfigPropose = 49,
    /// This configuration change proposal is not registered.
    ConfigProposeNotRegistered = 50,
    /// Transaction author attempts to vote twice.
    AttemptToVoteTwice = 51,
    /// Incorrect configuration number.
    IncorrectConfigurationNumber = 52,
    /// Invalid configuration for supervisor.
    InvalidConfig = 53,
}

/// Configuration-related errors group.
/// Error codes 64-79.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[derive(ExecutionFail)]
pub enum MigrationError {
    /// Migration request has not been registered or accepted.
    MigrationRequestNotRegistered = 64,
    /// Migration was started but failed during the execution.
    MigrationFailed = 65,
    /// Several nodes reported different state hashes.
    StateHashDivergence = 66,
}
