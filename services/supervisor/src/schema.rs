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

use serde_derive::{Deserialize, Serialize};

use exonum::{
    crypto::{hash, Hash},
    helpers::multisig::ValidatorMultisig,
    runtime::ArtifactId,
};
use exonum_merkledb::{BinaryValue, Entry, IndexAccess, ObjectHash, ProofMapIndex};

use std::borrow::Cow;

use super::{ConfigPropose, DeployConfirmation, DeployRequest, StartService};

/// Pending config change proposal entry
#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigProposalWithHash {
    pub config_propose: ConfigPropose,
    pub propose_hash: Hash,
}

impl BinaryValue for ConfigProposalWithHash {
    fn to_bytes(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap()
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Result<Self, failure::Error> {
        bincode::deserialize(bytes.as_ref()).map_err(From::from)
    }
}

impl ObjectHash for ConfigProposalWithHash {
    fn object_hash(&self) -> Hash {
        hash(&self.to_bytes())
    }
}

/// Service information schema.
#[derive(Debug)]
pub struct Schema<'a, T> {
    access: T,
    instance_name: &'a str,
}

impl<'a, T: IndexAccess> Schema<'a, T> {
    /// Constructs schema for the given `access`.
    pub fn new(instance_name: &'a str, access: T) -> Self {
        Self {
            instance_name,
            access,
        }
    }

    pub fn deploy_requests(&self) -> ValidatorMultisig<T, DeployRequest> {
        ValidatorMultisig::new(
            [self.instance_name, ".deploy_requests"].concat(),
            self.access.clone(),
        )
    }

    pub fn deploy_confirmations(&self) -> ValidatorMultisig<T, DeployConfirmation> {
        ValidatorMultisig::new(
            [self.instance_name, ".deploy_confirmations"].concat(),
            self.access.clone(),
        )
    }

    pub fn pending_deployments(&self) -> ProofMapIndex<T, ArtifactId, DeployRequest> {
        ProofMapIndex::new(
            [self.instance_name, ".pending_deployments"].concat(),
            self.access.clone(),
        )
    }

    pub fn pending_instances(&self) -> ValidatorMultisig<T, StartService> {
        ValidatorMultisig::new(
            [self.instance_name, ".pending_instances"].concat(),
            self.access.clone(),
        )
    }

    pub fn config_confirms(&self) -> ValidatorMultisig<T, Hash> {
        ValidatorMultisig::new(
            [self.instance_name, ".config_confirms"].concat(),
            self.access.clone(),
        )
    }

    pub fn pending_proposal(&self) -> Entry<T, ConfigProposalWithHash> {
        Entry::new(
            [self.instance_name, ".pending_proposal"].concat(),
            self.access.clone(),
        )
    }

    /// Returns hashes for tables with proofs.
    pub fn state_hash(&self) -> Vec<Hash> {
        vec![
            self.deploy_requests().object_hash(),
            self.deploy_confirmations().object_hash(),
            self.pending_deployments().object_hash(),
            self.pending_instances().object_hash(),
            self.config_confirms().object_hash(),
        ]
    }
}
