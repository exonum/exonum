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

use protobuf::well_known_types::Any;

use crate::storage::Fork;

pub mod dispatcher;
pub mod error;
mod rust;

use error::{DeployError, ExecutionError, InitError};

#[derive(Debug, PartialEq, Eq)]
pub enum DeployStatus {
    DeployInProgress,
    Deployed,
}

type ServiceInstanceId = u32;
type MethodId = String;

#[derive(Debug)]
pub struct InstanceInitData {
    instance_id: ServiceInstanceId,
    constructor_data: Option<Any>,
}

#[derive(Debug)]
pub struct CallInfo {
    pub instance_id: ServiceInstanceId,
    pub method_id: MethodId,
}

impl CallInfo {
    pub fn new(instance_id: ServiceInstanceId, method_id: MethodId) -> Self {
        Self {
            instance_id,
            method_id,
        }
    }
}

#[derive(Debug)]
pub enum RuntimeIdentifier {
    Rust,
    Java,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum ArtifactSpec {
    Rust(rust::RustArtifactSpec),
    Java,
}

/// Service runtime environment.
/// It does not assign id to services/interfaces, ids are given to runtime from outside.
pub trait RuntimeEnvironment {
    /// Start artifact deploy.
    fn start_deploy(&self, artifact: ArtifactSpec) -> Result<(), DeployError>;

    /// Check deployment status.
    fn check_deploy_status(&self, artifact: ArtifactSpec) -> Result<DeployStatus, DeployError>;

    /// Init artifact with given ID and constructor parameters.
    fn init_service(
        &mut self,
        ctx: &mut EnvContext,
        artifact: ArtifactSpec,
        init: &InstanceInitData,
    ) -> Result<(), InitError>;

    /// Execute transaction.
    fn execute(
        &self,
        ctx: &mut EnvContext,
        dispatch: CallInfo,
        payload: &[u8],
    ) -> Result<(), ExecutionError>;
}

#[derive(Debug)]
pub struct EnvContext<'a> {
    fork: &'a mut Fork,
    error: Option<ExecutionError>,
}

impl<'a> EnvContext<'a> {
    fn from_fork(fork: &'a mut Fork) -> Self {
        Self { fork, error: None }
    }
}
