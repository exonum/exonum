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

use exonum_merkledb::{BinaryValue, Fork, Snapshot};
use protobuf::well_known_types::Any;

use std::fmt::Debug;

use crate::{
    api::ServiceApiBuilder,
    crypto::{Hash, PublicKey, SecretKey},
    messages::{CallInfo, ServiceInstanceId},
    node::ApiSender,
};

use self::{
    error::{DeployError, ExecutionError, StartError},
    rust::RustArtifactSpec,
};

#[macro_use]
pub mod rust;
pub mod configuration_new;
pub mod dispatcher;
pub mod error;

#[derive(Debug, PartialEq, Eq)]
pub enum DeployStatus {
    DeployInProgress,
    Deployed,
}

#[derive(Debug, Default)]
pub struct ServiceConstructor {
    pub data: Any,
}

impl ServiceConstructor {
    pub fn new(data: impl BinaryValue) -> Self {
        let bytes = data.into_bytes();

        Self {
            data: {
                let mut data = Any::new();
                data.set_value(bytes);
                data
            },
        }
    }
}

#[derive(Debug)]
pub struct ServiceInstanceSpec {
    pub id: ServiceInstanceId,
    pub name: String,
    pub artifact: ArtifactSpec,
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

// TODO Think about environment methods' names. [ECR-3222]

/// Runtime environment for services.
///
/// It does not assign id to services/interfaces, ids are given to runtime from outside.
pub trait Runtime: Send + Debug + 'static {
    /// Begins deploy artifact with the given specification.
    fn begin_deploy(&mut self, artifact: ArtifactSpec) -> Result<(), DeployError>;

    /// Checks deployment status.
    fn check_deploy_status(
        &self,
        artifact: ArtifactSpec,
        cancel_if_not_complete: bool,
    ) -> Result<DeployStatus, DeployError>;

    /// Starts a new service instance with the given specification.
    fn start_service(&mut self, spec: &ServiceInstanceSpec) -> Result<(), StartError>;

    /// Configures a service instance with the given parameters.
    fn configure_service(
        &self,
        context: &mut RuntimeContext,
        spec: &ServiceInstanceSpec,
        parameters: &ServiceConstructor,
    ) -> Result<(), StartError>;

    /// Stops existing service instance with the given specification.
    fn stop_service(&mut self, spec: &ServiceInstanceSpec) -> Result<(), StartError>;

    /// Execute transaction.
    fn execute(
        &self,
        context: &mut RuntimeContext,
        call_info: CallInfo,
        payload: &[u8],
    ) -> Result<(), ExecutionError>;

    /// Gets state hashes of the every contained service.
    fn state_hashes(&self, snapshot: &dyn Snapshot) -> Vec<(ServiceInstanceId, Vec<Hash>)>;

    /// Calls `before_commit` for all the services stored in the runtime.
    fn before_commit(&self, fork: &mut Fork);

    // TODO interface should be re-worked
    /// Calls `after_commit` for all the services stored in the runtime.
    fn after_commit(
        &self,
        snapshot: &dyn Snapshot,
        service_keypair: &(PublicKey, SecretKey),
        tx_sender: &ApiSender,
    );

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
