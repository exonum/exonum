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
    runtime::{ArtifactId, InstanceId},
};
use exonum_derive::FromAccess;
use exonum_merkledb::{
    access::{Access, Prefixed},
    Entry, Fork, ObjectHash, ProofMapIndex,
};

use super::{
    multisig::MultisigIndex, ConfigProposalWithHash, DeployConfirmation, DeployRequest,
    StartService,
};

/// Service information schema.
#[derive(Debug, FromAccess)]
pub struct Schema<T: Access> {
    pub deploy_requests: MultisigIndex<T, DeployRequest>,
    pub deploy_confirmations: MultisigIndex<T, DeployConfirmation>,
    pub pending_deployments: ProofMapIndex<T::Base, ArtifactId, DeployRequest>,
    pub pending_instances: MultisigIndex<T, StartService>,
    pub config_confirms: MultisigIndex<T, Hash>,
    pub pending_proposal: Entry<T::Base, ConfigProposalWithHash>,
    pub configuration_number: Entry<T::Base, u64>,
    pub vacant_instance_id: Entry<T::Base, InstanceId>,
}

impl<T: Access> Schema<T> {
    pub fn get_configuration_number(&self) -> u64 {
        self.configuration_number.get().unwrap_or(0)
    }

    /// Returns hashes for tables with proofs.
    pub fn state_hash(&self) -> Vec<Hash> {
        vec![
            self.deploy_requests.object_hash(),
            self.deploy_confirmations.object_hash(),
            self.pending_deployments.object_hash(),
            self.pending_instances.object_hash(),
            self.config_confirms.object_hash(),
        ]
    }
}

impl Schema<Prefixed<'_, &Fork>> {
    pub fn increase_configuration_number(&mut self) {
        let new_configuration_number = self.get_configuration_number() + 1;
        self.configuration_number.set(new_configuration_number);
    }

    /// Assign unique identifier for an instance.
    /// Returns `None` if `vacant_instance_id` entry was not initialized.
    pub(crate) fn assign_instance_id(&mut self) -> Option<InstanceId> {
        let id = self.vacant_instance_id.get()?;
        self.vacant_instance_id.set(id + 1);
        Some(id)
    }
}
