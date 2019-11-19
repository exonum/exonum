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

use exonum::{
    crypto::Hash,
    helpers::multisig::ValidatorMultisig,
    runtime::{ArtifactId, InstanceId},
};
use exonum_merkledb::{
    access::{Access, FromAccess, Prefixed},
    Entry, Fork, ProofMapIndex,
};

use super::{
    ConfigProposalWithHash, DeployConfirmation, DeployRequest, StartService,
    MAX_BUILTIN_INSTANCE_ID,
};

const DEPLOY_REQUESTS: &str = "deploy_requests";
const DEPLOY_CONFIRMATIONS: &str = "deploy_confirmations";
const PENDING_DEPLOYMENTS: &str = "pending_deployments";
const PENDING_INSTANCES: &str = "pending_instances";
const CONFIG_CONFIRMS: &str = "config_confirms";
const PENDING_PROPOSAL: &str = "pending_proposal";
const CONFIGURATION_NUMBER: &str = "configuration_number";
const VACANT_INSTANCE_ID: &str = "vacant_instance_id";

/// Service information schema.
#[derive(Debug)]
pub struct Schema<T: Access> {
    pub deploy_requests: ValidatorMultisig<T, DeployRequest>,
    pub deploy_confirmations: ValidatorMultisig<T, DeployConfirmation>,
    pub pending_deployments: ProofMapIndex<T::Base, ArtifactId, DeployRequest>,
    pub pending_instances: ValidatorMultisig<T, StartService>,
    pub config_confirms: ValidatorMultisig<T, Hash>,
    pub pending_proposal: Entry<T::Base, ConfigProposalWithHash>,
    pub configuration_number: Entry<T::Base, u64>,
    pub vacant_instance_id: Entry<T::Base, InstanceId>,
}

impl<'a, T: Access> Schema<Prefixed<'a, T>> {
    /// Constructs schema for the given `access`.
    pub fn new(access: Prefixed<'a, T>) -> Self {
        Self {
            deploy_requests: construct(&access, DEPLOY_REQUESTS),
            deploy_confirmations: construct(&access, DEPLOY_CONFIRMATIONS),
            pending_deployments: construct(&access, PENDING_DEPLOYMENTS),
            pending_instances: construct(&access, PENDING_INSTANCES),
            config_confirms: construct(&access, CONFIG_CONFIRMS),
            pending_proposal: construct(&access, PENDING_PROPOSAL),
            configuration_number: construct(&access, CONFIGURATION_NUMBER),
            vacant_instance_id: construct(&access, VACANT_INSTANCE_ID),
        }
    }

    pub fn get_configuration_number(&self) -> u64 {
        self.configuration_number.get().unwrap_or(0)
    }
}

impl Schema<Prefixed<'_, &Fork>> {
    pub fn increase_configuration_number(&mut self) {
        let new_configuration_number = self.configuration_number.get().unwrap_or(0) + 1;
        self.configuration_number.set(new_configuration_number);
    }

    /// Assign unique identifier for an instance.
    pub(crate) fn assign_instance_id(&mut self) -> InstanceId {
        let id = self
            .vacant_instance_id
            .get()
            .unwrap_or(MAX_BUILTIN_INSTANCE_ID);
        self.vacant_instance_id.set(id + 1);
        id
    }
}

/// Creates an index given its name and access object.
fn construct<'a, T: Access, U: FromAccess<Prefixed<'a, T>>>(
    access: &Prefixed<'a, T>,
    index_name: &str,
) -> U {
    FromAccess::from_access(access.clone(), index_name.into()).unwrap()
}
