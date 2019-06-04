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

use exonum_merkledb::{BinaryValue, Fork, Snapshot, BinaryKey};
use protobuf::well_known_types::Any;

use std::fmt::Debug;

use crate::{
    api::ServiceApiBuilder,
    crypto::{Hash, PublicKey},
    messages::{CallInfo, ServiceInstanceId},
};

use self::{
    error::{DeployError, ExecutionError, InitError},
    rust::RustArtifactSpec,
};

#[macro_use]
pub mod rust;
pub mod configuration;
pub mod dispatcher;
pub mod error;

#[derive(Debug, PartialEq, Eq)]
pub enum DeployStatus {
    DeployInProgress,
    Deployed,
}

#[derive(Debug)]
pub struct ServiceConstructor {
    pub instance_id: ServiceInstanceId,
    pub data: Any,
}

impl ServiceConstructor {
    pub fn new(instance_id: ServiceInstanceId, data: impl BinaryValue) -> Self {
        let bytes = data.into_bytes();

        Self {
            instance_id,
            data: {
                let mut data = Any::new();
                data.set_value(bytes);
                data
            },
        }
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum RuntimeIdentifier {
    Rust = 0,
    Java = 1,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct ArtifactSpec {
    runtime_id: u32,
    raw_spec: Vec<u8>,
}

impl From<RustArtifactSpec> for ArtifactSpec {
    fn from(artifact: RustArtifactSpec) -> Self {
        ArtifactSpec {
            runtime_id: RuntimeIdentifier::Rust as u32,
            raw_spec: artifact.into_bytes(),
        }
    }
}

impl BinaryKey for ArtifactSpec {
    fn size(&self) -> usize {
        BinaryKey::size(&self.runtime_id) + BinaryKey::size(&self.raw_spec)
    }

    fn write(&self, buffer: &mut [u8]) -> usize {
        self.runtime_id.write(&mut buffer[0..4]);
        self.raw_spec.write(&mut buffer[4..4 + self.raw_spec.len()]);
        self.size()
    }

    fn read(buffer: &[u8]) -> Self {
        let runtime_id = u32::read(&buffer[0..4]);
        let raw_spec: Vec<u8> = Vec::read(&buffer[4..]);
        Self { runtime_id, raw_spec }
    }
}

// TODO Think about environment methods' names. [ECR-3222]

/// Runtime environment for services.
/// It does not assign id to services/interfaces, ids are given to runtime from outside.
pub trait Runtime: Send + Debug + 'static {
    /// Start artifact deploy.
    fn start_deploy(&mut self, artifact: ArtifactSpec) -> Result<(), DeployError>;

    /// Check deployment status.
    fn check_deploy_status(
        &self,
        artifact: ArtifactSpec,
        cancel_if_not_complete: bool,
    ) -> Result<DeployStatus, DeployError>;

    /// Init artifact with given ID and constructor parameters.
    fn init_service(
        &mut self,
        ctx: &mut RuntimeContext,
        artifact: ArtifactSpec,
        constructor: &ServiceConstructor,
    ) -> Result<(), InitError>;

    /// Execute transaction.
    fn execute(
        &self,
        ctx: &mut RuntimeContext,
        call_info: CallInfo,
        payload: &[u8],
    ) -> Result<(), ExecutionError>;

    /// Gets state hashes of the every contained service.
    fn state_hashes(&self, snapshot: &dyn Snapshot) -> Vec<(ServiceInstanceId, Vec<Hash>)>;

    /// Calls `before_commit` for all the services stored in the runtime.
    fn before_commit(&self, fork: &mut Fork);

    // TODO interface should be re-worked
    /// Calls `after_commit` for all the services stored in the runtime.
    fn after_commit(&self, fork: &Fork);

    fn services_api(&self) -> Vec<(String, ServiceApiBuilder)> {
        Vec::new()
    }
}

#[derive(Debug)]
pub struct RuntimeContext<'a> {
    fork: &'a Fork,
    author: PublicKey,
    tx_hash: Hash,
    dispatcher_actions: Vec<dispatcher::Action>,
}

impl<'a> RuntimeContext<'a> {
    pub fn new(fork: &'a Fork, author: PublicKey, tx_hash: Hash) -> Self {
        Self {
            fork,
            author,
            tx_hash,
            dispatcher_actions: Vec::new(),
        }
    }

    pub(crate) fn dispatch_action(&mut self, action: dispatcher::Action) {
        self.dispatcher_actions.push(action);
    }

    pub(crate) fn take_dispatcher_actions(&mut self) -> Vec<dispatcher::Action> {
        let mut other = Vec::new();
        std::mem::swap(&mut self.dispatcher_actions, &mut other);
        other
    }
}

impl<T> From<T> for Box<dyn Runtime>
where
    T: Runtime,
{
    fn from(runtime: T) -> Self {
        Box::new(runtime) as Self
    }
}
