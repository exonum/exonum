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

//! Information schema for the runtime dispatcher.

use exonum_crypto::Hash;
use exonum_merkledb::{
    access::{Access, AccessExt, AsReadonly},
    Fork, ListIndex, MapIndex, ObjectHash, ProofMapIndex,
};

use crate::runtime::{
    ArtifactState, ArtifactStatus, InstanceId, InstanceQuery, InstanceState, InstanceStatus,
};

use super::{ArtifactSpec, Error, InstanceSpec};

const ARTIFACTS: &str = "dispatcher_artifacts";
const PENDING_ARTIFACTS: &str = "dispatcher_pending_artifacts";
const INSTANCES: &str = "dispatcher_instances";
const PENDING_INSTANCES: &str = "dispatcher_pending_instances";
const INSTANCE_IDS: &str = "dispatcher_instance_ids";

/// Schema of the dispatcher, used to store information about pending artifacts / service
/// instances, and to reload artifacts / instances on node restart.
// TODO: Add information about implemented interfaces [ECR-3747]
#[derive(Debug)]
pub struct Schema<T: Access> {
    access: T,
}

impl<T: Access> Schema<T> {
    /// Constructs information schema for the given `access`.
    pub(crate) fn new(access: T) -> Self {
        Self {
            access: access.clone(),
        }
    }

    /// Returns an artifacts registry indexed by the artifact name.
    pub(crate) fn artifacts(&self) -> ProofMapIndex<T::Base, String, ArtifactState> {
        self.access.clone().get_proof_map(ARTIFACTS)
    }

    /// Returns a service instances registry indexed by the instance name.
    pub(crate) fn instances(&self) -> ProofMapIndex<T::Base, String, InstanceState> {
        self.access.clone().get_proof_map(INSTANCES)
    }

    /// Returns a lookup table to map instance ID with the instance name.
    fn instance_ids(&self) -> MapIndex<T::Base, InstanceId, String> {
        self.access.clone().get_map(INSTANCE_IDS)
    }

    /// Returns a pending artifacts queue used to notify the runtime about artifacts
    /// to be deployed.
    fn pending_artifacts(&self) -> ListIndex<T::Base, ArtifactSpec> {
        self.access.clone().get_list(PENDING_ARTIFACTS)
    }

    /// Returns a pending instances queue used to notify the runtime about service instances
    /// to be committed.
    fn pending_instances(&self) -> ListIndex<T::Base, InstanceSpec> {
        self.access.clone().get_list(PENDING_INSTANCES)
    }

    /// Returns the information about a service instance by its identifier.
    pub fn get_instance<'q>(&self, query: impl Into<InstanceQuery<'q>>) -> Option<InstanceState> {
        let instances = self.instances();
        match query.into() {
            // TODO It makes sense to indexing by identifiers primary. [ECR-3880]
            InstanceQuery::Id(id) => self
                .instance_ids()
                .get(&id)
                .and_then(|instance_name| instances.get(&instance_name)),

            InstanceQuery::Name(instance_name) => instances.get(instance_name),
        }
    }

    /// Returns information about an artifact by its identifier.
    pub fn get_artifact(&self, name: &str) -> Option<ArtifactState> {
        self.artifacts().get(name)
    }

    /// Returns hashes for tables with proofs.
    pub(crate) fn state_hash(&self) -> Vec<Hash> {
        vec![
            self.artifacts().object_hash(),
            self.instances().object_hash(),
        ]
    }
}

// `AsReadonly` specialization to ensure that we won't leak mutable schema access.
impl<T: AsReadonly> Schema<T> {
    /// Readonly set of service instances.
    pub fn service_instances(&self) -> ProofMapIndex<T::Readonly, String, InstanceState> {
        self.access.as_readonly().get_proof_map(INSTANCES)
    }
}

impl Schema<&Fork> {
    /// Adds artifact specification to the set of the pending artifacts.
    pub(super) fn add_pending_artifact(&mut self, spec: ArtifactSpec) -> Result<(), Error> {
        // Check that the artifact is absent among the deployed artifacts.
        if self.artifacts().contains(&spec.artifact.name) {
            return Err(Error::ArtifactAlreadyDeployed);
        }
        // Add artifact to pending artifacts queue.
        self.pending_artifacts().push(spec.clone());
        // Add artifact to registry with pending status.
        let artifact_name = spec.artifact.name.clone();
        self.artifacts().put(
            &artifact_name,
            ArtifactState {
                spec,
                status: ArtifactStatus::Pending,
            },
        );
        Ok(())
    }

    /// Adds information about a pending service instance to the schema.
    pub(crate) fn add_pending_service(&mut self, spec: InstanceSpec) -> Result<(), Error> {
        let artifact_id = self
            .artifacts()
            .get(&spec.artifact.name)
            .ok_or(Error::ArtifactNotDeployed)?
            .spec
            .artifact;

        let mut instances = self.instances();
        let mut instance_ids = self.instance_ids();

        // Checks that runtime identifier is proper in instance.
        // TODO It seems that this error cannot be produced by the user code, thus we might
        // replace error by assertion. [ECR-3743]
        if artifact_id != spec.artifact {
            return Err(Error::IncorrectRuntime);
        }
        // Checks that instance name doesn't exist.
        if instances.contains(&spec.name) {
            return Err(Error::ServiceNameExists);
        }
        // Checks that instance identifier doesn't exist.
        // TODO: revise dispatcher integrity checks [ECR-3743]
        if instance_ids.contains(&spec.id) {
            return Err(Error::ServiceIdExists);
        }

        let id = spec.id;
        let name = spec.name.clone();
        instances.put(
            &name,
            InstanceState {
                spec: spec.clone(),
                status: InstanceStatus::Pending,
            },
        );
        instance_ids.put(&id, name);
        self.pending_instances().push(spec);
        Ok(())
    }

    // Make pending artifacts and instances active.
    pub(super) fn activate_pending(&mut self) {
        // Activate pending artifacts.
        let mut artifacts = self.artifacts();
        for spec in &self.pending_artifacts() {
            let name = spec.artifact.name.clone();
            artifacts.put(&name, ArtifactState::new(spec, ArtifactStatus::Active));
        }
        // Activate pending instances.
        let mut instances = self.instances();
        for spec in &self.pending_instances() {
            let name = spec.name.clone();
            instances.put(&name, InstanceState::new(spec, InstanceStatus::Active));
        }
    }

    /// Takes pending artifacts from queue.
    pub(super) fn take_pending_artifacts(&mut self) -> impl IntoIterator<Item = ArtifactSpec> {
        let mut index = self.pending_artifacts();
        let pending_artifacts = index.iter().collect::<Vec<_>>();
        index.clear();
        pending_artifacts
    }

    /// Takes pending service instances from queue.
    pub(super) fn take_pending_instances(&mut self) -> impl IntoIterator<Item = InstanceSpec> {
        let mut index = self.pending_instances();
        let pending_instances = index.iter().collect::<Vec<_>>();
        index.clear();
        pending_instances
    }
}
