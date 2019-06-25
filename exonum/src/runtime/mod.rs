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

pub use self::dispatcher::Dispatcher;
pub use crate::messages::ServiceInstanceId;

use exonum_merkledb::{BinaryValue, Fork, Snapshot};
use protobuf::well_known_types::Any;
use serde_derive::{Deserialize, Serialize};

use std::fmt::Debug;

use crate::{
    api::ServiceApiBuilder,
    crypto::{Hash, PublicKey, SecretKey},
    messages::CallInfo,
    node::ApiSender,
    proto::schema,
};

use self::error::{DeployError, ExecutionError, StartError};

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

impl DeployStatus {
    pub fn is_deployed(&self) -> bool {
        if let DeployStatus::Deployed = self {
            true
        } else {
            false
        }
    }

    pub fn is_pending(&self) -> bool {
        if let DeployStatus::DeployInProgress = self {
            true
        } else {
            false
        }
    }
}

#[derive(Debug, Default)]
pub struct ServiceConfig {
    pub data: Any,
}

impl ServiceConfig {
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, ProtobufConvert, Serialize, Deserialize)]
#[exonum(pb = "schema::runtime::InstanceSpec", crate = "crate")]
pub struct InstanceSpec {
    pub id: ServiceInstanceId,
    pub name: String,
    pub artifact: ArtifactId,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum RuntimeIdentifier {
    Rust = 0,
    Java = 1,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, ProtobufConvert, Serialize, Deserialize)]
#[exonum(pb = "schema::runtime::ArtifactId", crate = "crate")]
pub struct ArtifactId {
    pub runtime: u32,
    pub raw_id: String,
}

impl ArtifactId {
    /// Creates a new artifact identifier from the given runtime and
    /// corresponding runtime-specific artifact id.
    pub fn new(runtime: u32, raw: impl Into<String>) -> Self {
        Self {
            runtime,
            raw_id: raw.into(),
        }
    }
}

// TODO Think about runtime methods' names. [ECR-3222]

/// Runtime environment for services.
///
/// It does not assign id to services/interfaces, ids are given to runtime from outside.
pub trait Runtime: Send + Debug + 'static {
    /// Begins deploy artifact with the given specification.
    fn begin_deploy(&mut self, artifact: &ArtifactId) -> Result<DeployStatus, DeployError>;

    /// Checks deployment status.
    fn check_deploy_status(
        &self,
        artifact: &ArtifactId,
        cancel_if_not_complete: bool,
    ) -> Result<DeployStatus, DeployError>;

    /// Starts a new service instance with the given specification.
    fn start_service(&mut self, spec: &InstanceSpec) -> Result<(), StartError>;

    /// Configures a service instance with the given parameters.
    fn configure_service(
        &self,
        context: &Fork,
        spec: &InstanceSpec,
        parameters: &ServiceConfig,
    ) -> Result<(), StartError>;

    /// Stops existing service instance with the given specification.
    fn stop_service(&mut self, spec: &InstanceSpec) -> Result<(), StartError>;

    /// Execute transaction.
    // TODO Do not use dispatcher struct directly.
    fn execute(
        &self,
        dispatcher: &dispatcher::Dispatcher,
        context: &mut RuntimeContext,
        call_info: CallInfo,
        payload: &[u8],
    ) -> Result<(), ExecutionError>;

    /// Gets state hashes of the every contained service.
    fn state_hashes(&self, snapshot: &dyn Snapshot) -> Vec<(ServiceInstanceId, Vec<Hash>)>;

    /// Calls `before_commit` for all the services stored in the runtime.
    fn before_commit(&self, dispatcher: &dispatcher::Dispatcher, fork: &mut Fork);

    // TODO interface should be re-worked
    /// Calls `after_commit` for all the services stored in the runtime.
    fn after_commit(
        &self,
        dispatcher: &dispatcher::Dispatcher,
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

    pub fn without_author(fork: &'a Fork) -> Self {
        Self {
            fork,
            author: PublicKey::zero(),
            tx_hash: Hash::zero(),
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
