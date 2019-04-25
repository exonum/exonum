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

use crate::crypto::{Hash, PublicKey};
use crate::messages::{CallInfo, ServiceInstanceId};
use exonum_merkledb::{Fork, Snapshot};

#[macro_use]
pub mod rust;

pub mod configuration;
pub mod configuration_new;
pub mod dispatcher;
pub mod error;

use error::{DeployError, ExecutionError, InitError};

#[derive(Debug, PartialEq, Eq)]
pub enum DeployStatus {
    DeployInProgress,
    Deployed,
}

#[derive(Debug)]
pub struct InstanceInitData {
    pub instance_id: ServiceInstanceId,
    pub constructor_data: Any,
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

/// Service runtime environment.
/// It does not assign id to services/interfaces, ids are given to runtime from outside.
pub trait RuntimeEnvironment {
    /// Start artifact deploy.
    fn start_deploy(&self, artifact: ArtifactSpec) -> Result<(), DeployError>;

    /// Check deployment status.
    fn check_deploy_status(
        &self,
        artifact: ArtifactSpec,
        cancel_if_not_complete: bool,
    ) -> Result<DeployStatus, DeployError>;

    /// Init artifact with given ID and constructor parameters.
    fn init_service(
        &self,
        ctx: &mut RuntimeContext,
        artifact: ArtifactSpec,
        init: &InstanceInitData,
    ) -> Result<(), InitError>;

    /// Execute transaction.
    fn execute(
        &self,
        ctx: &mut RuntimeContext,
        dispatch: CallInfo,
        payload: &[u8],
    ) -> Result<(), ExecutionError>;

    /// Gets state hashes of the every contained service.
    fn state_hashes(&self, snapshot: &dyn Snapshot) -> Vec<(ServiceInstanceId, Vec<Hash>)>;

    /// Calls `before_commit` for all the services stored in the runtime.
    fn before_commit(&self, fork: &mut Fork);

    // TODO interface should be re-worked
    /// Calls `after_commit` for all the services stored in the runtime.
    fn after_commit(&self, fork: &mut Fork);
}

#[derive(Debug)]
pub struct RuntimeContext<'a> {
    fork: &'a mut Fork,
    author: PublicKey,
    tx_hash: Hash,
}

impl<'a> RuntimeContext<'a> {
    pub fn new(fork: &'a mut Fork, &author: &PublicKey, &tx_hash: &Hash) -> Self {
        Self {
            fork,
            author,
            tx_hash,
        }
    }

    fn from_fork(fork: &'a mut Fork) -> Self {
        Self::new(fork, &PublicKey::zero(), &Hash::zero())
    }
}
