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

use exonum_merkledb::{IndexAccess, KeySetIndex, ProofMapIndex};

use super::{ArtifactId, DeployError, InstanceSpec, StartError};
use crate::messages::ServiceInstanceId;

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

    /// Set of service instances.
    // TODO Get rid of data duplication in information schema. [ECR-3222]
    pub fn service_instances(&self) -> ProofMapIndex<T, String, InstanceSpec> {
        ProofMapIndex::new("core.dispatcher.service_instances", self.access.clone())
    }

    /// Internal index to store identifiers of service instances.
    fn service_instance_ids(&self) -> KeySetIndex<T, ServiceInstanceId> {
        KeySetIndex::new("core.dispatcher.service_instance_ids", self.access.clone())
    }

    /// Adds artifact specification to the set of deployed artifacts.
    pub fn add_artifact(&mut self, artifact: ArtifactId) -> Result<(), DeployError> {
        // Checks that we have not already deployed this artifact.
        if self.artifacts().contains(&artifact.name) {
            return Err(DeployError::AlreadyDeployed);
        }

        self.artifacts().put(&artifact.name, artifact.runtime_id);
        Ok(())
    }

    /// Adds information about started service instance to the schema.
    pub fn add_service_instance(&mut self, spec: InstanceSpec) -> Result<(), StartError> {
        let runtime_id = self
            .artifacts()
            .get(&spec.artifact.name)
            .ok_or(StartError::NotDeployed)?;
        // Checks that runtime identifier is proper in instance.
        if runtime_id != spec.artifact.runtime_id {
            return Err(StartError::WrongRuntime);
        }
        // Checks that instance name doesn't exist.
        if self.service_instances().contains(&spec.name) {
            return Err(StartError::ServiceNameExists);
        }
        // Checks that instance identifier doesn't exist.
        if self.service_instance_ids().contains(&spec.id) {
            return Err(StartError::ServiceIdExists);
        }

        let name = spec.name.clone();
        self.service_instance_ids().insert(spec.id);
        self.service_instances().put(&name, spec);
        Ok(())
    }

    /// Returns the smallest vacant identifier for service instance.
    pub fn vacant_instance_id(&self) -> ServiceInstanceId {
        // TODO O(n) optimize [ECR-3222]
        let latest_known_id = self
            .service_instance_ids()
            .iter()
            .last()
            .unwrap_or_default();
        latest_known_id + 1
    }
}
