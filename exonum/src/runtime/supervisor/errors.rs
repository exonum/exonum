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

use crate::blockchain::ExecutionError;

/// Common errors emitted by transactions during execution.
#[derive(Debug, Fail)]
#[repr(u8)]
pub enum Error {
    /// Artifact has been already deployed.
    #[fail(display = "Artifact has been already deployed")]
    AlreadyDeployed = 0,
    /// Transaction author is not a validator.
    #[fail(display = "Transaction author is not a validator")]
    UnknownAuthor = 1,
    /// Reached deadline for deploying artifact.
    #[fail(display = "Reached deadline for deploying artifact")]
    DeployDeadline = 2,
    /// Instance with the given name already exists.
    #[fail(display = "Instance with the given name already exists")]
    InstanceExists = 3,    
}

impl From<Error> for ExecutionError {
    fn from(value: Error) -> ExecutionError {
        let description = value.to_string();
        ExecutionError::with_description(value as u8, description)
    }
}
