// Copyright 2020 The Exonum Team
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

use exonum_merkledb::{
    access::{Access, AccessExt, AsReadonly},
    Fork, KeySetIndex, MapIndex, ProofMapIndex,
};

use super::{ArtifactId, Error, InstanceSpec};
use crate::runtime::{
    ArtifactState, ArtifactStatus, InstanceId, InstanceQuery, InstanceState, InstanceStatus,
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
    access: T,
}

impl<T: Access> Schema<T> {
    /// Constructs information schema for the given `access`.
    pub(crate) fn new(access: T) -> Self {
        Self { access }
    }

    /// Returns an artifacts registry indexed by the artifact name.
    pub(crate) fn artifacts(&self) -> ProofMapIndex<T::Base, ArtifactId, ArtifactState> {
        self.access.clone().get_proof_map(ARTIFACTS)
    }

    /// Returns a service instances registry indexed by the instance name.
    pub(crate) fn instances(&self) -> ProofMapIndex<T::Base, str, InstanceState> {
        self.access.clone().get_proof_map(INSTANCES)
    }

    /// Returns a lookup table to map instance ID with the instance name.
    fn instance_ids(&self) -> MapIndex<T::Base, InstanceId, String> {
        self.access.clone().get_map(INSTANCE_IDS)
    }

    /// Returns a pending artifacts queue used to notify the runtime about artifacts
    /// to be deployed.
    fn pending_artifacts(&self) -> KeySetIndex<T::Base, ArtifactId> {
        self.access.clone().get_key_set(PENDING_ARTIFACTS)
    }

    /// Returns a pending instances queue used to notify the runtime about service instances
    /// to be updated.
    fn modified_instances(&self) -> MapIndex<T::Base, str, InstanceStatus> {
        self.access.clone().get_map(PENDING_INSTANCES)
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
    pub fn get_artifact(&self, name: &ArtifactId) -> Option<ArtifactState> {
        self.artifacts().get(name)
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
    pub(super) fn add_pending_artifact(
        &mut self,
        artifact: ArtifactId,
        deploy_spec: Vec<u8>,
    ) -> Result<(), Error> {
        // Check that the artifact is absent among the deployed artifacts.
        if self.artifacts().contains(&artifact) {
            return Err(Error::ArtifactAlreadyDeployed);
        }
        // Add artifact to registry with pending status.
        self.artifacts().put(
            &artifact,
            ArtifactState {
                deploy_spec,
                status: ArtifactStatus::Pending,
            },
        );
        // Add artifact to pending artifacts queue.
        self.pending_artifacts().insert(artifact);
        Ok(())
    }

    /// Adds information about a pending service instance to the schema.
    pub(crate) fn initiate_adding_service(&mut self, spec: InstanceSpec) -> Result<(), Error> {
        self.artifacts()
            .get(&spec.artifact)
            .ok_or(Error::ArtifactNotDeployed)?;

        let mut instances = self.instances();
        let mut instance_ids = self.instance_ids();

        // Checks that instance name doesn't exist.
        if instances.contains(&spec.name) {
            return Err(Error::ServiceNameExists);
        }
        // Checks that instance identifier doesn't exist.
        // TODO: revise dispatcher integrity checks [ECR-3743]
        if instance_ids.contains(&spec.id) {
            return Err(Error::ServiceIdExists);
        }

        let instance_id = spec.id;
        let instance_name = spec.name.clone();
        let pending_status = InstanceStatus::Active;

        instances.put(
            &instance_name,
            InstanceState {
                spec,
                status: None,
                pending_status: Some(pending_status),
            },
        );
        self.modified_instances()
            .put(&instance_name, pending_status);
        instance_ids.put(&instance_id, instance_name);
        Ok(())
    }

    /// Adds information about stopping service instance to the schema.
    pub(crate) fn initiate_stopping_service(
        &mut self,
        instance_id: InstanceId,
    ) -> Result<(), Error> {
        let mut instances = self.instances();
        let mut modified_instances = self.modified_instances();

        let instance_name = self
            .instance_ids()
            .get(&instance_id)
            .ok_or(Error::IncorrectInstanceId)?;

        let mut state = instances
            .get(&instance_name)
            .expect("BUG: Instance identifier exists but the corresponding instance is missing.");

        match state.status {
            Some(InstanceStatus::Active) => {}
            _ => return Err(Error::ServiceNotActive),
        }

        if state.pending_status.is_some() {
            return Err(Error::ServicePending);
        }

        // Modify instance status.
        let pending_status = InstanceStatus::Stopped;
        // Because we guarantee that the stopping service will process all transactions and other
        // events in the block,  we cannot stop it immediately. But we must account these changes
        // in the state hash, therefore we use pending status.
        state.pending_status = Some(pending_status);
        modified_instances.put(&instance_name, pending_status);
        instances.put(&instance_name, state);
        Ok(())
    }

    /// Make pending artifacts and instances active.
    pub(super) fn activate_pending(&mut self) {
        // Activate pending artifacts.
        let mut artifacts = self.artifacts();
        for artifact in &self.pending_artifacts() {
            let mut state = artifacts
                .get(&artifact)
                .expect("Artifact marked as pending is not saved in `artifacts`");
            state.status = ArtifactStatus::Active;
            artifacts.put(&artifact, state);
        }
        // Commit new statuses for pending instances.
        let mut instances = self.instances();
        for (instance, status) in &self.modified_instances() {
            let mut state = instances
                .get(&instance)
                .expect("BUG: Instance marked as modified is not saved in `instances`");
            debug_assert_eq!(
                Some(status),
                state.pending_status,
                "BUG: Instance status in `modified_instances` should be same as `pending_status` \
                 in the instance state."
            );

            state.commit_pending_status();
            instances.put(&instance, state);
        }
    }

    /// Takes pending artifacts from queue.
    pub(super) fn take_pending_artifacts(&mut self) -> Vec<(ArtifactId, Vec<u8>)> {
        let mut index = self.pending_artifacts();
        let artifacts = self.artifacts();
        let pending_artifacts = index
            .iter()
            .map(|artifact| {
                let deploy_spec = artifacts
                    .get(&artifact)
                    .expect("Artifact marked as pending is not saved in `artifacts`")
                    .deploy_spec;
                (artifact, deploy_spec)
            })
            .collect();
        index.clear();
        pending_artifacts
    }

    /// Takes modified service instances from queue.
    pub(super) fn take_modified_instances(&mut self) -> Vec<(InstanceSpec, InstanceStatus)> {
        let mut modified_instances = self.modified_instances();
        let instances = self.instances();

        let output = modified_instances
            .iter()
            .map(|(instance_name, status)| {
                let state = instances
                    .get(&instance_name)
                    .expect("BUG: Instance marked as modified is not saved in `instances`");
                (state.spec, status)
            })
            .collect::<Vec<_>>();
        modified_instances.clear();

        output
    }
}
