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

use crate::blockchain::ExecutionError;
use crate::storage::Fork;

mod rust;

#[derive(Debug)]
pub enum DeployError {
    WrongArtifact,
    FailedToDeploy,
    AlreadyDeployed,
}

#[derive(Debug)]
pub enum InitError {
    WrongArtifact,
    NotDeployed,
    ServiceIdExists,
}

type ServiceInstanceId = u32;
type InterfaceId = u32;

#[derive(Debug)]
pub struct InstanceInitData {
    instance_id: ServiceInstanceId,
    constructor_data: Option<Any>,
}

#[derive(Debug)]
pub struct DispatchInfo {
    pub instance_id: ServiceInstanceId,
    pub interface_id: InterfaceId,
    pub method_id: u32,
}

pub enum ArtifactSpec {
    Rust(rust::RustArtifactSpec),
    Java,
}

/// Service runtime environment.
/// It does not assign id to services/interfaces, ids are given to runtime from outside.
pub trait RuntimeEnvironment {
    /// Deploy artifact.
    fn deploy(&self, artifact: ArtifactSpec) -> Result<(), DeployError>;

    /// Init artifact with given ID and constructor parameters.
    fn start_init(
        &self,
        ctx: &mut EnvContext,
        artifact: ArtifactSpec,
        init: &InstanceInitData,
    ) -> Result<(), InitError>;

    /// Finalize artifact initialization.
    fn finish_init(&self, ctx: &mut EnvContext, instance_id: ServiceInstanceId, abort: bool);

    /// Execute transaction.
    fn execute(&self, ctx: &mut EnvContext, dispatch: DispatchInfo, payload: &[u8]);
}

pub struct EnvContext<'a> {
    fork: &'a mut Fork,
    error: Option<ExecutionError>,
}

impl<'a> EnvContext<'a> {
    fn from_fork(fork: &'a mut Fork) -> Self {
        Self { fork, error: None }
    }
}
