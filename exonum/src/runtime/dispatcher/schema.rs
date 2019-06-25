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

use exonum_merkledb::{IndexAccess, ProofMapIndex};

use super::{ArtifactId, DeployError, InstanceSpec, StartError};

#[derive(Debug, Clone)]
pub struct Schema<T: IndexAccess> {
    access: T,
}

impl<T: IndexAccess> Schema<T> {
    /// Constructs information schema for the given `access`.
    pub fn new(access: T) -> Self {
        Self { access }
    }

    /// Set of deployed artifacts: Key is artifact name, Value is runtime identifier.
    pub fn deployed_artifacts(&self) -> ProofMapIndex<T, String, u32> {
        ProofMapIndex::new("core.dispatcher.deployed_artifacts", self.access.clone())
    }

    /// Set of running services instances.
    // TODO Get rid of data duplication in information schema. [ECR-3222]
    pub fn started_services(&self) -> ProofMapIndex<T, String, InstanceSpec> {
        ProofMapIndex::new("core.dispatcher.started_services", self.access.clone())
    }

    /// Adds artifact specification to the set of deployed artifacts.
    pub fn add_deployed_artifact(&mut self, artifact: ArtifactId) -> Result<(), DeployError> {
        // Checks that we have not already deployed this artifact.
        if self.deployed_artifacts().contains(&artifact.name) {
            return Err(DeployError::AlreadyDeployed);
        }

        self.deployed_artifacts()
            .put(&artifact.name, artifact.runtime_id);

        Ok(())
    }

    /// Adds information about started service instance to the schema.
    /// Note that method doesn't check that service identifier is free.
    pub fn add_started_service(&mut self, spec: InstanceSpec) -> Result<(), StartError> {
        let runtime_id = self
            .deployed_artifacts()
            .get(&spec.artifact.name)
            .ok_or(StartError::NotDeployed)?;
        // Checks that runtime identifier is proper in instance.
        if runtime_id != spec.artifact.runtime_id {
            return Err(StartError::WrongRuntime);
        }
        let name = spec.name.clone();
        self.started_services().put(&name, spec);
        Ok(())
    }
}
