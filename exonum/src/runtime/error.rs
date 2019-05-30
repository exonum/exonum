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

// TODO: Use two level error codes for ExecutionError, first level is runtime errors [ECR-3236]
// second level is service errors.

pub use crate::blockchain::ExecutionError;

use crate::blockchain;

// TODO: summarize error codes/simplify error creation

pub const DISPATCH_ERROR: u8 = 255;
pub const WRONG_ARG_ERROR: u8 = 254;
pub const WRONG_RUNTIME: u8 = 253;

#[derive(Debug, PartialEq, Eq)]
pub enum DeployError {
    WrongRuntime,
    WrongArtifact,
    FailedToDeploy,
    AlreadyDeployed,
}

#[derive(Debug, PartialEq, Eq)]
pub enum InitError {
    WrongRuntime,
    WrongArtifact,
    NotDeployed,
    ServiceIdExists,
    ExecutionError(ExecutionError),
}

impl From<DeployError> for ExecutionError {
    fn from(deploy: DeployError) -> Self {
        ExecutionError::with_description(128, format!("Deploy failed because: {:?}", deploy))
    }
}

impl From<InitError> for ExecutionError {
    fn from(init: InitError) -> Self {
        ExecutionError::with_description(129, format!("Init failed because: {:?}", init))
    }
}
