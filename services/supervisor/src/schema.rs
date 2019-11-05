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
use exonum_merkledb::{
    access::{Access, Ensure, Prefixed, RawAccessMut, Restore},
    Entry, ObjectHash, ProofMapIndex,
};

use super::{ConfigProposalWithHash, DeployConfirmation, DeployRequest, StartService};

/// Service information schema.
#[derive(Debug)]
pub struct Schema<T: Access> {
    pub deploy_requests: ValidatorMultisig<T, DeployRequest>,
    pub deploy_confirmations: ValidatorMultisig<T, DeployConfirmation>,
    pub pending_deployments: ProofMapIndex<T::Base, ArtifactId, DeployRequest>,
    pub pending_instances: ValidatorMultisig<T, StartService>,
    pub config_confirms: ValidatorMultisig<T, Hash>,
    pub pending_proposal: Entry<T::Base, ConfigProposalWithHash>,
}

impl<'a, T: Access> Schema<Prefixed<'a, T>> {
    /// Constructs schema for the given `access`.
    pub fn new(access: Prefixed<'a, T>) -> Self {
        Self {
            deploy_requests: Restore::restore(&access, "deploy_requests".into()).unwrap(),
            deploy_confirmations: Restore::restore(&access, "deploy_confirmations".into()).unwrap(),
            pending_deployments: Restore::restore(&access, "pending_deployments".into()).unwrap(),
            pending_instances: Restore::restore(&access, "pending_instances".into()).unwrap(),
            config_confirms: Restore::restore(&access, "config_confirms".into()).unwrap(),
            pending_proposal: Restore::restore(&access, "pending_proposal".into()).unwrap(),
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

impl<'a, T> Schema<Prefixed<'a, T>>
where
    T: Access,
    T::Base: RawAccessMut,
{
    pub(crate) fn ensure(access: Prefixed<'a, T>) -> Self {
        Self {
            deploy_requests: Ensure::ensure(&access, "deploy_requests".into()).unwrap(),
            deploy_confirmations: Ensure::ensure(&access, "deploy_confirmations".into()).unwrap(),
            pending_deployments: Ensure::ensure(&access, "pending_deployments".into()).unwrap(),
            pending_instances: Ensure::ensure(&access, "pending_instances".into()).unwrap(),
            config_confirms: Ensure::ensure(&access, "config_confirms".into()).unwrap(),
            pending_proposal: Ensure::ensure(&access, "pending_proposal".into()).unwrap(),
        }
    }
}
