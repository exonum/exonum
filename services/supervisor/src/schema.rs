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
    runtime::{ArtifactId, InstanceId, Version, VersionReq, Versioned},
};
use exonum_derive::FromAccess;
use exonum_merkledb::{
    access::{Access, Prefixed},
    Entry, Fork, ProofEntry, ProofMapIndex,
};

use super::{
    multisig::MultisigIndex, ConfigProposalWithHash, DeployConfirmation, DeployRequest,
    SupervisorConfig,
};

/// Service information schema.
#[derive(Debug, FromAccess)]
pub(crate) struct Schema<T: Access> {
    /// Public part of the schema.
    #[from_access(flatten)]
    pub public: SchemaInterface<T>,

    /// Stored deploy requests with the confirmations from the validators.
    pub deploy_requests: MultisigIndex<T, DeployRequest>,
    /// Validator confirmations on successful deployments.
    pub deploy_confirmations: MultisigIndex<T, DeployConfirmation>,
    /// Artifacts to be deployed.
    pub pending_deployments: ProofMapIndex<T::Base, ArtifactId, DeployRequest>,
    /// Votes for a configuration change.
    pub config_confirms: MultisigIndex<T, Hash>,
    /// Number of the processed configurations. Used to avoid conflicting configuration proposals.
    pub configuration_number: Entry<T::Base, u64>,
    /// The following free instance ID for assignment.
    pub vacant_instance_id: Entry<T::Base, InstanceId>,
}

/// Public part of the supervisor service.
#[derive(Debug, FromAccess)]
pub struct SchemaInterface<T: Access> {
    /// Supervisor configuration.
    pub configuration: ProofEntry<T::Base, SupervisorConfig>,
    /// Current pending configuration proposal.
    pub pending_proposal: ProofEntry<T::Base, ConfigProposalWithHash>,
}

impl<T: Access> Versioned for SchemaInterface<T> {
    const NAME: &'static str = env!("CARGO_PKG_NAME");

    fn is_compatible(version: &Version) -> bool {
        let version_req: VersionReq = env!("CARGO_PKG_VERSION").parse().unwrap();
        version_req.matches(version)
    }
}

impl<T: Access> Schema<T> {
    /// Gets the stored configuration number.
    pub fn get_configuration_number(&self) -> u64 {
        self.configuration_number.get().unwrap_or(0)
    }

    /// Gets the configuration for the `Supervisor`.
    pub fn supervisor_config(&self) -> SupervisorConfig {
        // Configuration is required to be set, and there is no valid way
        // to obtain `Supervisor` without configuration, thus this expect
        // is intended to be safe.
        self.public
            .configuration
            .get()
            .expect("Supervisor entity was not configured; unable to load configuration")
    }
}

impl Schema<Prefixed<'_, &Fork>> {
    /// Increases the stored configuration number.
    pub fn increase_configuration_number(&mut self) {
        let new_configuration_number = self.get_configuration_number() + 1;
        self.configuration_number.set(new_configuration_number);
    }

    /// Assigns a unique identifier for an instance.
    /// Returns `None` if `vacant_instance_id` entry was not initialized.
    pub(crate) fn assign_instance_id(&mut self) -> Option<InstanceId> {
        let id = self.vacant_instance_id.get()?;
        self.vacant_instance_id.set(id + 1);
        Some(id)
    }
}
