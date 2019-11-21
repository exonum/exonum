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
    access::{Access, AccessExt, FromAccess},
    Fork, Lazy, ListIndex, MapIndex, ObjectHash, ProofMapIndex,
};

use crate::runtime::{DeployStatus, InstanceId, InstanceQuery};

use super::{
    types::{ArtifactState, ArtifactStatus, InstanceState},
    ArtifactId, ArtifactSpec, Error, InstanceSpec,
};

const ARTIFACTS: &str = "dispatcher_artifacts";
const PENDING_ARTIFACTS: &str = "dispatcher_pending_artifacts";
const INSTANCES: &str = "dispatcher_instances";
const PENDING_INSTANCES: &str = "dispatcher_pending_instances";
const INSTANCE_IDS: &str = "dispatcher_instance_ids";
const PENDING_INSTANCE_IDS: &str = "dispatcher_pending_instance_ids";

/// Schema of the dispatcher, used to store information about pending artifacts / service
/// instances, and to reload artifacts / instances on node restart.
// TODO: Add information about implemented interfaces [ECR-3747]
#[derive(Debug, Clone)]
pub struct Schema<T> {
    access: T,
}

impl<T: Access> Schema<T> {
    /// Constructs information schema for the given `access`.
    pub(crate) fn new(access: T) -> Self {
        Self { access }
    }

    /// Artifacts registry indexed by the artifact name.
    pub(crate) fn artifacts(&self) -> ProofMapIndex<T::Base, String, ArtifactSpec> {
        self.access.clone().get_proof_map(ARTIFACTS)
    }

    pub(super) fn pending_artifacts(&self) -> MapIndex<T::Base, String, ArtifactSpec> {
        self.access.clone().get_map(PENDING_ARTIFACTS)
    }

    /// Set of launched service instances.
    // TODO Get rid of data duplication in information schema. [ECR-3222]
    pub(crate) fn service_instances(&self) -> ProofMapIndex<T::Base, String, InstanceSpec> {
        self.access.clone().get_proof_map(INSTANCES)
    }

    /// Set of pending service instances.
    // TODO Get rid of data duplication in information schema. [ECR-3222]
    pub(super) fn pending_service_instances(&self) -> MapIndex<T::Base, String, InstanceSpec> {
        self.access.clone().get_map(PENDING_INSTANCES)
    }

    /// Identifiers of launched service instances.
    fn service_instance_ids(&self) -> MapIndex<T::Base, InstanceId, String> {
        self.access.clone().get_map(INSTANCE_IDS)
    }

    /// Identifiers of pending service instances.
    fn pending_instance_ids(&self) -> MapIndex<T::Base, InstanceId, String> {
        self.access.clone().get_map(PENDING_INSTANCE_IDS)
    }

    /// Returns the information about a service instance by its identifier.
    pub fn get_instance<'q>(
        &self,
        query: impl Into<InstanceQuery<'q>>,
    ) -> Option<(InstanceSpec, DeployStatus)> {
        match query.into() {
            InstanceQuery::Id(id) => {
                if let Some(instance_name) = self.service_instance_ids().get(&id) {
                    self.service_instances()
                        .get(&instance_name)
                        .map(|spec| (spec, DeployStatus::Active))
                } else if let Some(instance_name) = self.pending_instance_ids().get(&id) {
                    self.pending_service_instances()
                        .get(&instance_name)
                        .map(|spec| (spec, DeployStatus::Pending))
                } else {
                    None
                }
            }

            InstanceQuery::Name(instance_name) => self
                .service_instances()
                .get(instance_name)
                .map(|spec| (spec, DeployStatus::Active))
                .or_else(|| {
                    self.pending_service_instances()
                        .get(instance_name)
                        .map(|spec| (spec, DeployStatus::Pending))
                }),
        }
    }

    /// Returns information about an artifact by its identifier.
    pub fn get_artifact(&self, name: &str) -> Option<(ArtifactSpec, DeployStatus)> {
        self.artifacts()
            .get(name)
            .map(|spec| (spec, DeployStatus::Active))
            .or_else(|| {
                self.pending_artifacts()
                    .get(name)
                    .map(|spec| (spec, DeployStatus::Pending))
            })
    }

    /// Returns hashes for tables with proofs.
    #[allow(dead_code)]
    pub(crate) fn state_hash(&self) -> Vec<Hash> {
        vec![
            self.artifacts().object_hash(),
            self.service_instances().object_hash(),
        ]
    }
}

impl Schema<&Fork> {
    /// Adds artifact specification to the set of the pending artifacts.
    pub(super) fn add_pending_artifact(
        &mut self,
        artifact: ArtifactId,
        spec: Vec<u8>,
    ) -> Result<(), Error> {
        // Check that the artifact is absent among the deployed artifacts.
        if self.artifacts().contains(&artifact.name)
            || self.pending_artifacts().contains(&artifact.name)
        {
            return Err(Error::ArtifactAlreadyDeployed);
        }

        let name = artifact.name.clone();
        self.pending_artifacts().put(
            &name,
            ArtifactSpec {
                artifact,
                payload: spec,
            },
        );
        Ok(())
    }

    /// Add artifact specification to the set of the deployed artifacts.
    pub(super) fn add_artifact(&mut self, artifact: ArtifactId, spec: Vec<u8>) {
        // We use an assertion here since `add_pending_artifact` should have been called
        // with the same params before.
        debug_assert!(!self.artifacts().contains(&artifact.name));

        let name = artifact.name.clone();
        self.artifacts().put(
            &name,
            ArtifactSpec {
                artifact,
                payload: spec,
            },
        );
    }

    /// Adds information about a pending service instance to the schema.
    pub(crate) fn add_pending_service(&mut self, spec: InstanceSpec) -> Result<(), Error> {
        let artifact_id = SchemaNg::new(self.access)
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
        if self.service_instances().contains(&spec.name)
            || self.pending_service_instances().contains(&spec.name)
        {
            return Err(Error::ServiceNameExists);
        }
        // Checks that instance identifier doesn't exist.
        // TODO: revise dispatcher integrity checks [ECR-3743]
        if self.service_instance_ids().contains(&spec.id)
            || self.pending_instance_ids().contains(&spec.id)
        {
            return Err(Error::ServiceIdExists);
        }

        let id = spec.id;
        let name = spec.name.clone();
        self.pending_service_instances().put(&name, spec);
        self.pending_instance_ids().put(&id, name);
        Ok(())
    }

    /// Adds information about started service instance to the schema.
    pub(super) fn add_service(&mut self, spec: InstanceSpec) {
        debug_assert!(!self.service_instances().contains(&spec.name));
        debug_assert!(!self.service_instance_ids().contains(&spec.id));

        let id = spec.id;
        let name = spec.name.clone();
        self.service_instances().put(&name, spec);
        self.service_instance_ids().put(&id, name);
    }
}

/// Schema of the dispatcher, used to store information about pending artifacts / service
/// instances, and to reload artifacts / instances on node restart.
// TODO: Add information about implemented interfaces [ECR-3747]
#[derive(Debug)]
pub struct SchemaNg<T: Access> {
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

impl<T: Access> SchemaNg<T> {
    /// Constructs information schema for the given `access`.
    pub(crate) fn new(access: T) -> Self {
        Self {
            artifacts: construct(&access, ARTIFACTS),
            instances: construct(&access, INSTANCES),
            instances_by_id: construct(&access, PENDING_INSTANCE_IDS),
            pending_artifacts: construct(&access, PENDING_ARTIFACTS),
            pending_instances: construct(&access, PENDING_INSTANCES),
        }
    }

    /// Returns hashes for tables with proofs.
    #[allow(dead_code)]
    pub(crate) fn state_hash(&self) -> Vec<Hash> {
        vec![self.artifacts.object_hash(), self.instances.object_hash()]
    }
}

impl SchemaNg<&Fork> {
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

    /// Takes pending artifacts from queue.
    pub(super) fn take_pending_artifacts(&mut self) -> impl IntoIterator<Item = ArtifactSpec> {
        let pending_artifacts = self.pending_artifacts.iter().collect::<Vec<_>>();
        self.pending_artifacts.clear();
        pending_artifacts
    }
}

/// Creates an index given its name and access object.
fn construct<T: Access, U: FromAccess<T>>(access: &T, index_name: &str) -> U {
    FromAccess::from_access(access.clone(), format!("ng_{}", index_name).into()).unwrap()
}
