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

//! Information schema for the runtimes dispatcher.

use exonum_merkledb::{Entry, IndexAccess, KeySetIndex, MapIndex, ObjectHash, ProofMapIndex};

use super::{ArtifactId, Error, InstanceSpec, MAX_BUILTIN_INSTANCE_ID};
use crate::{crypto::Hash, proto::Any, runtime::ServiceInstanceId};

#[derive(Debug, Clone)]
pub struct Schema<T: IndexAccess> {
    access: T,
}

impl<T: IndexAccess> Schema<T> {
    /// Constructs information schema for the given `access`.
    pub fn new(access: T) -> Self {
        Self { access }
    }

    /// Artifacts registry where key is artifact name, value is runtime identifier.
    pub fn artifacts(&self) -> ProofMapIndex<T, String, u32> {
        ProofMapIndex::new("core.dispatcher.artifacts", self.access.clone())
    }

    /// Additional information needed to deploy artifacts.
    pub fn artifact_specs(&self) -> MapIndex<T, ArtifactId, Any> {
        MapIndex::new("core.dispatcher.artifact_specs", self.access.clone())
    }

    /// Set of service instances.
    // TODO Get rid of data duplication in information schema. [ECR-3222]
    pub fn service_instances(&self) -> ProofMapIndex<T, String, InstanceSpec> {
        ProofMapIndex::new("core.dispatcher.service_instances", self.access.clone())
    }

    /// Internal index to store identifiers of service instances.
    fn service_instance_ids(&self) -> KeySetIndex<T, ServiceInstanceId> {
        KeySetIndex::new("core.dispatcher.service_instance_ids", self.access.clone())
    }

    /// Vacant identifier for user service instances.
    fn vacant_instance_id(&self) -> Entry<T, ServiceInstanceId> {
        Entry::new("core.dispatcher.vacant_instance_id", self.access.clone())
    }

    /// Adds artifact specification to the set of deployed artifacts.
    pub(crate) fn add_artifact(&mut self, artifact: ArtifactId, spec: Any) -> Result<(), Error> {
        // Checks that we have not already deployed this artifact.
        if self.artifacts().contains(&artifact.name) {
            return Err(Error::ArtifactAlreadyDeployed);
        }

        self.artifacts().put(&artifact.name, artifact.runtime_id);
        self.artifact_specs().put(&artifact, spec);
        Ok(())
    }

    /// Assigns unique identifier for instance.
    pub(crate) fn assign_instance_id(&mut self) -> ServiceInstanceId {
        let id = self
            .vacant_instance_id()
            .get()
            .unwrap_or(MAX_BUILTIN_INSTANCE_ID);
        self.vacant_instance_id().set(id + 1);
        id
    }

    /// Adds information about started service instance to the schema.
    pub(crate) fn add_service_instance(&mut self, spec: InstanceSpec) -> Result<(), Error> {
        let runtime_id = self
            .artifacts()
            .get(&spec.artifact.name)
            .ok_or(Error::ArtifactNotDeployed)?;
        // Checks that runtime identifier is proper in instance.
        if runtime_id != spec.artifact.runtime_id {
            return Err(Error::IncorrectRuntime);
        }
        // Checks that instance name doesn't exist.
        if self.service_instances().contains(&spec.name) {
            return Err(Error::ServiceNameExists);
        }
        // Checks that instance identifier doesn't exist.
        if self.service_instance_ids().contains(&spec.id) {
            return Err(Error::ServiceIdExists);
        }

        let name = spec.name.clone();
        self.service_instance_ids().insert(spec.id);
        self.service_instances().put(&name, spec);
        Ok(())
    }

    pub fn artifacts_with_spec(&self) -> impl IntoIterator<Item = (ArtifactId, Any)> {
        // TODO remove reallocation [ECR-3222]
        self.artifact_specs().into_iter().collect::<Vec<_>>()
    }

    /// Returns the `state_hash` table for this information schema.
    pub fn state_hash(&self) -> Vec<Hash> {
        vec![
            self.artifacts().object_hash(),
            self.service_instances().object_hash(),
        ]
    }
}
