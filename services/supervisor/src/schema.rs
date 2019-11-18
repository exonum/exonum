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
    access::{Access, FromAccess, Prefixed},
    Entry, ProofMapIndex,
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
            deploy_requests: FromAccess::from_access(access.clone(), "deploy_requests".into())
                .unwrap(),
            deploy_confirmations: FromAccess::from_access(
                access.clone(),
                "deploy_confirmations".into(),
            )
            .unwrap(),
            pending_deployments: FromAccess::from_access(
                access.clone(),
                "pending_deployments".into(),
            )
            .unwrap(),
            pending_instances: FromAccess::from_access(access.clone(), "pending_instances".into())
                .unwrap(),
            config_confirms: FromAccess::from_access(access.clone(), "config_confirms".into())
                .unwrap(),
            pending_proposal: FromAccess::from_access(access, "pending_proposal".into()).unwrap(),
        }
    }
}
