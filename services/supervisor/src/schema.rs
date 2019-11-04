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

use exonum::{crypto::Hash, helpers::multisig::ValidatorMultisig, runtime::ArtifactId};
use exonum_merkledb::{AccessExt, Entry, IndexAccessMut, ObjectHash, ProofMapIndex};

use super::{ConfigProposalWithHash, DeployConfirmation, DeployRequest, StartService};

const NOT_INITIALIZED: &str = "Supervisor schema is not initialized";

/// Service information schema.
#[derive(Debug)]
pub struct Schema<T: AccessExt> {
    pub deploy_requests: ValidatorMultisig<T, DeployRequest>,
    pub deploy_confirmations: ValidatorMultisig<T, DeployConfirmation>,
    pub pending_deployments: ProofMapIndex<T::Base, ArtifactId, DeployRequest>,
    pub pending_instances: ValidatorMultisig<T, StartService>,
    pub config_confirms: ValidatorMultisig<T, Hash>,
    pub pending_proposal: Entry<T::Base, ConfigProposalWithHash>,
}

impl<T: AccessExt + Clone> Schema<T> {
    /// Constructs schema for the given `access`.
    pub fn new(access: T) -> Self {
        Self {
            deploy_requests: ValidatorMultisig::get("deploy_requests", access.clone())
                .expect(NOT_INITIALIZED),
            deploy_confirmations: ValidatorMultisig::get("deploy_confirmations", access.clone())
                .expect(NOT_INITIALIZED),
            pending_deployments: access
                .proof_map("pending_deployments")
                .expect(NOT_INITIALIZED),
            pending_instances: ValidatorMultisig::get("pending_instances", access.clone())
                .expect(NOT_INITIALIZED),
            config_confirms: ValidatorMultisig::get("config_confirms", access.clone())
                .expect(NOT_INITIALIZED),
            pending_proposal: access.entry("pending_proposal").expect(NOT_INITIALIZED),
        }
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

impl<T> Schema<T>
where
    T: AccessExt + Clone,
    T::Base: IndexAccessMut,
{
    pub(crate) fn initialize(access: T) -> Self {
        Self {
            deploy_requests: ValidatorMultisig::initialize("deploy_requests", access.clone()),
            deploy_confirmations: ValidatorMultisig::initialize(
                "deploy_confirmations",
                access.clone(),
            ),
            pending_deployments: access.ensure_proof_map("pending_deployments"),
            pending_instances: ValidatorMultisig::initialize("pending_instances", access.clone()),
            config_confirms: ValidatorMultisig::initialize("config_confirms", access.clone()),
            pending_proposal: access.ensure_entry("pending_proposal"),
        }
    }
}
