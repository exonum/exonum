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
    access::{Access, FromAccess},
    Fork, ListIndex, MapIndex, ObjectHash, ProofMapIndex,
};

use crate::runtime::{InstanceId, InstanceQuery};

use super::{
    types::{ArtifactState, ArtifactStatus, InstanceState, InstanceStatus},
    ArtifactSpec, Error, InstanceSpec,
};

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
    /// Artifacts registry indexed by the artifact name.
    pub artifacts: ProofMapIndex<T::Base, String, ArtifactState>,
    /// Service instances registry indexed by the instance name.
    pub instances: ProofMapIndex<T::Base, String, InstanceState>,
    /// Lookup table to map instance ID with the instance name.
    instances_by_id: MapIndex<T::Base, InstanceId, String>,
    /// A pending artifact queue used to notify the runtime about artifacts
    /// to be deployed.
    pending_artifacts: ListIndex<T::Base, ArtifactSpec>,

    pending_instances: ListIndex<T::Base, InstanceSpec>,
}

impl<T: Access> Schema<T> {
    /// Constructs information schema for the given `access`.
    pub(crate) fn new(access: T) -> Self {
        Self {
            artifacts: construct(&access, ARTIFACTS),
            instances: construct(&access, INSTANCES),
            instances_by_id: construct(&access, INSTANCE_IDS),
            pending_artifacts: construct(&access, PENDING_ARTIFACTS),
            pending_instances: construct(&access, PENDING_INSTANCES),
        }
    }

    /// Returns the information about a service instance by its identifier.
    pub fn get_instance<'q>(
        &self,
        query: impl Into<InstanceQuery<'q>>,
    ) -> Option<(InstanceSpec, InstanceStatus)> {
        match query.into() {
            InstanceQuery::Id(id) => self
                .instances_by_id
                .get(&id)
                .and_then(|instance_name| self.instances.get(&instance_name)),

            InstanceQuery::Name(instance_name) => self.instances.get(instance_name),
        }
        .map(|state| (state.spec, state.status))
    }

    /// Returns information about an artifact by its identifier.
    pub fn get_artifact(&self, name: &str) -> Option<(ArtifactSpec, ArtifactStatus)> {
        self.artifacts
            .get(name)
            .map(|state| (state.spec, state.status))
    }

    /// Returns hashes for tables with proofs.
    #[allow(dead_code)]
    pub(crate) fn state_hash(&self) -> Vec<Hash> {
        vec![self.artifacts.object_hash(), self.instances.object_hash()]
    }
}

impl Schema<&Fork> {
    /// Adds artifact specification to the set of the pending artifacts.
    pub(super) fn add_pending_artifact(&mut self, spec: ArtifactSpec) -> Result<(), Error> {
        // Check that the artifact is absent among the deployed artifacts.
        if self.artifacts.contains(&spec.artifact.name) {
            return Err(Error::ArtifactAlreadyDeployed);
        }
        // Add artifact to pending artifacts queue.
        self.pending_artifacts.push(spec.clone());
        // Add artifact to registry with pending status.
        let artifact_name = spec.artifact.name.clone();
        self.artifacts.put(
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
            .artifacts
            .get(&spec.artifact.name)
            .ok_or(Error::ArtifactNotDeployed)?
            .spec
            .artifact;

        // Checks that runtime identifier is proper in instance.
        if artifact_id != spec.artifact {
            return Err(Error::IncorrectRuntime);
        }
        // Checks that instance name doesn't exist.
        if self.instances.contains(&spec.name) {
            return Err(Error::ServiceNameExists);
        }
        // Checks that instance identifier doesn't exist.
        // TODO: revise dispatcher integrity checks [ECR-3743]
        if self.instances_by_id.contains(&spec.id) {
            return Err(Error::ServiceIdExists);
        }

        let id = spec.id;
        let name = spec.name.clone();
        self.instances.put(
            &name,
            InstanceState {
                spec: spec.clone(),
                status: InstanceStatus::Pending,
            },
        );
        self.instances_by_id.put(&id, name);
        self.pending_instances.push(spec);
        Ok(())
    }

    // Marks pending artifacts as deployed.
    pub(super) fn mark_pending_artifacts_as_active(&mut self) {
        for spec in &self.pending_artifacts {
            self.artifacts.put(
                &spec.artifact.name.clone(),
                ArtifactState {
                    spec,
                    status: ArtifactStatus::Active,
                },
            );
        }
    }

    /// Marks pending instances as active.
    pub(super) fn mark_pending_instances_as_active(&mut self) {
        for spec in &self.pending_instances {
            self.instances.put(
                &spec.name.clone(),
                InstanceState {
                    spec,
                    status: InstanceStatus::Active,
                },
            );
        }
    }

    /// Takes pending artifacts from queue.
    pub(super) fn take_pending_artifacts(&mut self) -> impl IntoIterator<Item = ArtifactSpec> {
        let pending_artifacts = self.pending_artifacts.iter().collect::<Vec<_>>();
        self.pending_artifacts.clear();
        pending_artifacts
    }

    /// Takes pending service instances from queue.
    pub(super) fn take_pending_instances(&mut self) -> impl IntoIterator<Item = InstanceSpec> {
        let pending_instances = self.pending_instances.iter().collect::<Vec<_>>();
        self.pending_instances.clear();
        pending_instances
    }
}

/// Creates an index given its name and access object.
fn construct<T: Access, U: FromAccess<T>>(access: &T, index_name: &str) -> U {
    FromAccess::from_access(access.clone(), index_name.into()).unwrap()
}
