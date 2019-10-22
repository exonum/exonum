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

use exonum_merkledb::{Entry, IndexAccess, MapIndex};

use super::{ArtifactId, ArtifactSpec, Error, InstanceSpec, MAX_BUILTIN_INSTANCE_ID};
use crate::runtime::{DeployStatus, InstanceId, InstanceQuery};

/// Schema of the dispatcher, used to store information about pending artifacts / service
/// instances, and to reload artifacts / instances on node restart.
#[derive(Debug, Clone)]
pub struct Schema<T: IndexAccess> {
    access: T,
}

impl<T: IndexAccess> Schema<T> {
    /// Constructs information schema for the given `access`.
    pub fn new(access: T) -> Self {
        Self { access }
    }

    /// Artifacts registry indexed by the artifact name.
    pub(crate) fn artifacts(&self) -> MapIndex<T, String, ArtifactSpec> {
        MapIndex::new("core.dispatcher.artifacts", self.access.clone())
    }

    pub(super) fn pending_artifacts(&self) -> MapIndex<T, String, ArtifactSpec> {
        MapIndex::new("core.dispatcher.pending_artifacts", self.access.clone())
    }

    /// Set of launched service instances.
    // TODO Get rid of data duplication in information schema. [ECR-3222]
    pub(crate) fn service_instances(&self) -> MapIndex<T, String, InstanceSpec> {
        MapIndex::new("core.dispatcher.service_instances", self.access.clone())
    }

    /// Set of pending service instances.
    // TODO Get rid of data duplication in information schema. [ECR-3222]
    pub(super) fn pending_service_instances(&self) -> MapIndex<T, String, InstanceSpec> {
        MapIndex::new(
            "core.dispatcher.pending_service_instances",
            self.access.clone(),
        )
    }

    /// Identifiers of launched service instances.
    fn service_instance_ids(&self) -> MapIndex<T, InstanceId, String> {
        MapIndex::new("core.dispatcher.service_instance_ids", self.access.clone())
    }

    /// Identifiers of pending service instances.
    fn pending_instance_ids(&self) -> MapIndex<T, InstanceId, String> {
        MapIndex::new("core.dispatcher.pending_instance_ids", self.access.clone())
    }

    /// Vacant identifier for user service instances.
    fn vacant_instance_id(&self) -> Entry<T, InstanceId> {
        Entry::new("core.dispatcher.vacant_instance_id", self.access.clone())
    }

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
        let artifact_id = self
            .artifacts()
            .get(&spec.artifact.name)
            .or_else(|| self.pending_artifacts().get(&spec.artifact.name))
            .ok_or(Error::ArtifactNotDeployed)?
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

    /// Assign unique identifier for an instance.
    pub(crate) fn assign_instance_id(&mut self) -> InstanceId {
        let id = self
            .vacant_instance_id()
            .get()
            .unwrap_or(MAX_BUILTIN_INSTANCE_ID);
        self.vacant_instance_id().set(id + 1);
        id
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

    /// Returns the information about a service instance by its identifier.
    pub fn get_instance<'s>(
        &'s self,
        query: impl Into<InstanceQuery<'s>>,
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
}
