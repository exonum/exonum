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

use exonum_crypto::Hash;
use exonum_derive::*;
use exonum_merkledb::{
    access::{Access, AccessExt, AsReadonly},
    Fork, KeySetIndex, MapIndex, ProofMapIndex,
};
use exonum_proto::ProtobufConvert;
use semver::Version;

use crate::{
    proto::schema::{
        self, details::ModifiedInstanceInfo_MigrationTransition as PbMigrationTransition,
    },
    runtime::{
        migrations::{InstanceMigration, MigrationStatus},
        ArtifactId, ArtifactState, ArtifactStatus, CoreError, ExecutionError, InstanceId,
        InstanceQuery, InstanceSpec, InstanceState, InstanceStatus,
    },
};

const ARTIFACTS: &str = "dispatcher_artifacts";
const PENDING_ARTIFACTS: &str = "dispatcher_pending_artifacts";
const INSTANCES: &str = "dispatcher_instances";
const PENDING_INSTANCES: &str = "dispatcher_pending_instances";
const LOCAL_MIGRATION_RESULTS: &str = "dispatcher_local_migration_results";
const INSTANCE_IDS: &str = "dispatcher_instance_ids";

/// Information about a modified service instance.
#[derive(Debug, ProtobufConvert, BinaryValue)]
#[protobuf_convert(source = "schema::details::ModifiedInstanceInfo")]
pub(super) struct ModifiedInstanceInfo {
    #[protobuf_convert(with = "MigrationTransition")]
    pub migration_transition: Option<MigrationTransition>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum MigrationTransition {
    Start,
    Commit,
    Rollback,
}

impl MigrationTransition {
    #[allow(clippy::wrong_self_convention, clippy::trivially_copy_pass_by_ref)]
    fn to_pb(value: &Option<Self>) -> PbMigrationTransition {
        use PbMigrationTransition::*;
        match value {
            None => NONE,
            Some(MigrationTransition::Start) => START,
            Some(MigrationTransition::Commit) => COMMIT,
            Some(MigrationTransition::Rollback) => ROLLBACK,
        }
    }

    fn from_pb(pb: PbMigrationTransition) -> Result<Option<Self>, failure::Error> {
        use PbMigrationTransition::*;
        Ok(match pb {
            NONE => None,
            START => Some(MigrationTransition::Start),
            COMMIT => Some(MigrationTransition::Commit),
            ROLLBACK => Some(MigrationTransition::Rollback),
        })
    }
}

#[derive(Debug, Clone, Copy)]
enum MigrationOutcome {
    Rollback,
    Commit(Hash),
}

impl From<MigrationOutcome> for MigrationTransition {
    fn from(value: MigrationOutcome) -> Self {
        match value {
            MigrationOutcome::Rollback => MigrationTransition::Rollback,
            MigrationOutcome::Commit(_) => MigrationTransition::Commit,
        }
    }
}

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
        self.access.get_proof_map(ARTIFACTS)
    }

    /// Returns a service instances registry indexed by the instance name.
    pub(crate) fn instances(&self) -> ProofMapIndex<T::Base, str, InstanceState> {
        self.access.get_proof_map(INSTANCES)
    }

    /// Returns a lookup table to map instance ID with the instance name.
    fn instance_ids(&self) -> MapIndex<T::Base, InstanceId, String> {
        self.access.get_map(INSTANCE_IDS)
    }

    /// Returns a pending artifacts queue used to notify the runtime about artifacts
    /// to be deployed.
    fn pending_artifacts(&self) -> KeySetIndex<T::Base, ArtifactId> {
        self.access.get_key_set(PENDING_ARTIFACTS)
    }

    /// Returns a pending instances queue used to notify the runtime about service instances
    /// to be updated.
    fn modified_instances(&self) -> MapIndex<T::Base, str, ModifiedInstanceInfo> {
        self.access.get_map(PENDING_INSTANCES)
    }

    pub(crate) fn local_migration_results(&self) -> MapIndex<T::Base, str, MigrationStatus> {
        self.access.get_map(LOCAL_MIGRATION_RESULTS)
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

            InstanceQuery::__NonExhaustive => unreachable!("Never actually constructed"),
        }
    }

    /// Returns information about an artifact by its identifier.
    pub fn get_artifact(&self, name: &ArtifactId) -> Option<ArtifactState> {
        self.artifacts().get(name)
    }

    /// Returns result of a locally completed migration for the specified service instance.
    ///
    /// This result is set once the migration script associated with the service instance completes
    /// and is cleared after the migration is flushed or rolled back.
    pub fn local_migration_result(&self, instance_name: &str) -> Option<MigrationStatus> {
        self.local_migration_results().get(instance_name)
    }
}

// `AsReadonly` specialization to ensure that we won't leak mutable schema access.
impl<T: AsReadonly> Schema<T> {
    /// Readonly set of artifacts.
    pub fn service_artifacts(&self) -> ProofMapIndex<T::Readonly, ArtifactId, ArtifactState> {
        self.access.as_readonly().get_proof_map(ARTIFACTS)
    }

    /// Readonly set of service instances.
    pub fn service_instances(&self) -> ProofMapIndex<T::Readonly, str, InstanceState> {
        self.access.as_readonly().get_proof_map(INSTANCES)
    }
}

impl Schema<&Fork> {
    /// Adds artifact specification to the set of the pending artifacts.
    pub(super) fn add_pending_artifact(
        &mut self,
        artifact: &ArtifactId,
        deploy_spec: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        // Check that the artifact is absent among the deployed artifacts.
        if self.artifacts().contains(artifact) {
            return Err(CoreError::ArtifactAlreadyDeployed.into());
        }
        // Add artifact to registry with pending status.
        self.artifacts().put(
            artifact,
            ArtifactState::new(deploy_spec, ArtifactStatus::Pending),
        );
        // Add artifact to pending artifacts queue.
        self.pending_artifacts().insert(artifact);
        Ok(())
    }

    /// Checks preconditions for migration initiation.
    pub(super) fn check_migration_initiation(
        &self,
        new_artifact: &ArtifactId,
        old_service: &str,
    ) -> Result<InstanceState, CoreError> {
        // The service should exist.
        let instance_state = self
            .instances()
            .get(old_service)
            .ok_or(CoreError::IncorrectInstanceId)?;

        // The service should be stopped. Note that this also checks that
        // the service is not being migrated.
        if instance_state.status != Some(InstanceStatus::Stopped) {
            return Err(CoreError::ServiceNotStopped);
        }
        // There should be no pending status for the service.
        if instance_state.pending_status.is_some() {
            return Err(CoreError::ServicePending);
        }

        // The new artifact should exist.
        let artifact_state = self
            .artifacts()
            .get(&new_artifact)
            .ok_or(CoreError::UnknownArtifactId)?;
        // The new artifact should be deployed.
        if artifact_state.status != ArtifactStatus::Active {
            return Err(CoreError::ArtifactNotDeployed);
        }

        // The new artifact should refer a newer version of the service artifact.
        if !new_artifact.is_upgrade_of(&instance_state.spec.artifact) {
            return Err(CoreError::CannotUpgradeService);
        }
        Ok(instance_state)
    }

    /// Marks the start of data migration for a service. This method does not perform
    /// consistency checks assuming that this call is preceded by `check_migration_initiation`.
    pub(super) fn add_pending_migration(
        &mut self,
        instance_state: InstanceState,
        migration: InstanceMigration,
    ) {
        let pending_status = InstanceStatus::migrating(migration);
        self.add_pending_status(
            instance_state,
            pending_status,
            Some(MigrationTransition::Start),
        )
        .expect("BUG: Cannot add pending service status during migration initialization");
        // Since we've checked in `check_migration_initiation` that the service
        // has no pending status, we assume that it will be added successfully here.
    }

    /// Fast-forwards data migration by bumping the recorded service version.
    /// The entire migration workflow is skipped in this case; the service remains
    /// with the `Stopped` status and no pending status is added.
    pub(super) fn fast_forward_migration(
        &mut self,
        mut instance_state: InstanceState,
        new_version: Version,
    ) {
        debug_assert!(*instance_state.data_version() < new_version);
        instance_state.data_version = Some(new_version);
        let instance_name = instance_state.spec.name.clone();
        self.instances().put(&instance_name, instance_state);
    }

    fn add_pending_status(
        &mut self,
        mut instance_state: InstanceState,
        pending_status: InstanceStatus,
        migration_transition: Option<MigrationTransition>,
    ) -> Result<(), CoreError> {
        if instance_state.pending_status.is_some() {
            return Err(CoreError::ServicePending);
        }
        instance_state.pending_status = Some(pending_status);
        let instance_name = instance_state.spec.name.clone();
        let modified_info = ModifiedInstanceInfo {
            migration_transition,
        };
        self.instances().put(&instance_name, instance_state);
        self.modified_instances().put(&instance_name, modified_info);
        Ok(())
    }

    fn resolve_ongoing_migration(
        &mut self,
        instance_name: &str,
        outcome: MigrationOutcome,
    ) -> Result<(), CoreError> {
        let instance_state = self
            .instances()
            .get(instance_name)
            .ok_or(CoreError::IncorrectInstanceId)?;
        let migration = match instance_state.status {
            Some(InstanceStatus::Migrating(ref migration)) if !migration.is_completed() => {
                migration
            }
            _ => return Err(CoreError::NoMigration),
        };
        let new_status = match outcome {
            MigrationOutcome::Rollback => InstanceStatus::Stopped,
            MigrationOutcome::Commit(hash) => {
                let mut migration = migration.to_owned();
                migration.completed_hash = Some(hash);
                InstanceStatus::Migrating(migration)
            }
        };

        self.add_pending_status(instance_state, new_status, Some(outcome.into()))?;
        Ok(())
    }

    /// Saves migration rollback to the database. Returns an error if the rollback breaks
    /// invariants imposed by the migration workflow.
    pub(super) fn add_migration_rollback(&mut self, instance_name: &str) -> Result<(), CoreError> {
        self.resolve_ongoing_migration(instance_name, MigrationOutcome::Rollback)?;
        self.local_migration_results().remove(instance_name);
        Ok(())
    }

    /// Saves migration commit to the database. Returns an error if the commit breaks
    /// invariants imposed by the migration workflow. Note that an error is *not* returned
    /// if the local migration result contradicts the commit (this is only checked on block commit).
    pub(super) fn add_migration_commit(
        &mut self,
        instance_name: &str,
        hash: Hash,
    ) -> Result<(), CoreError> {
        self.resolve_ongoing_migration(instance_name, MigrationOutcome::Commit(hash))
    }

    /// Saves local migration result to the database.
    pub(super) fn add_local_migration_result(
        &mut self,
        instance_name: &str,
        result: MigrationStatus,
    ) {
        self.local_migration_results().put(instance_name, result);
    }

    /// Adds information about a pending service instance to the schema.
    pub(crate) fn initiate_adding_service(&mut self, spec: InstanceSpec) -> Result<(), CoreError> {
        self.artifacts()
            .get(&spec.artifact)
            .ok_or(CoreError::ArtifactNotDeployed)?;

        // Check that instance name doesn't exist.
        if self.instances().contains(&spec.name) {
            return Err(CoreError::ServiceNameExists);
        }
        // Check that instance identifier doesn't exist.
        // TODO: revise dispatcher integrity checks [ECR-3743]
        let mut instance_ids = self.instance_ids();
        if instance_ids.contains(&spec.id) {
            return Err(CoreError::ServiceIdExists);
        }
        instance_ids.put(&spec.id, spec.name.clone());

        let new_instance = InstanceState::from_raw_parts(spec, None, None, None);
        self.add_pending_status(new_instance, InstanceStatus::Active, None)
    }

    /// Adds information about stopping service instance to the schema.
    pub(crate) fn initiate_stopping_service(
        &mut self,
        instance_id: InstanceId,
    ) -> Result<(), CoreError> {
        let instance_name = self
            .instance_ids()
            .get(&instance_id)
            .ok_or(CoreError::IncorrectInstanceId)?;

        let state = self
            .instances()
            .get(&instance_name)
            .expect("BUG: Instance identifier exists but the corresponding instance is missing.");

        if state.status == Some(InstanceStatus::Active) {
            self.add_pending_status(state, InstanceStatus::Stopped, None)
        } else {
            Err(CoreError::ServiceNotActive)
        }
    }

    /// Adds information about resuming service instance to the schema.
    pub(crate) fn initiate_resuming_service(
        &mut self,
        instance_id: InstanceId,
        artifact: ArtifactId,
    ) -> Result<(), CoreError> {
        let instance_name = self
            .instance_ids()
            .get(&instance_id)
            .ok_or(CoreError::IncorrectInstanceId)?;

        let mut state = self
            .instances()
            .get(&instance_name)
            .expect("BUG: Instance identifier exists but the corresponding instance is missing.");

        if state.spec.artifact.name != artifact.name {
            return Err(CoreError::CannotResumeService);
        }
        if state.data_version() != &artifact.version {
            return Err(CoreError::CannotResumeService);
        }

        if state.status == Some(InstanceStatus::Stopped) {
            state.spec.artifact = artifact;
            self.add_pending_status(state, InstanceStatus::Active, None)
        } else {
            Err(CoreError::ServiceNotStopped)
        }
    }

    /// Makes pending artifacts and instances active.
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
        for instance in self.modified_instances().keys() {
            let mut state = instances
                .get(&instance)
                .expect("BUG: Instance marked as modified is not saved in `instances`");
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

    /// Takes modified service instances from queue. This method should be called
    /// after new service statuses are committed (e.g., in `commit_block`).
    pub(super) fn take_modified_instances(&mut self) -> Vec<(InstanceState, ModifiedInstanceInfo)> {
        let mut modified_instances = self.modified_instances();
        let instances = self.instances();

        let output = modified_instances
            .iter()
            .map(|(instance_name, info)| {
                let state = instances
                    .get(&instance_name)
                    .expect("BUG: Instance marked as modified is not saved in `instances`");
                (state, info)
            })
            .collect();
        modified_instances.clear();

        output
    }

    /// Marks a service migration as completed. This sets the service status from `Migrating`
    /// to `Stopped`, bumps its artifact version and removes the local migration result.
    pub(super) fn complete_migration(&mut self, instance_name: &str) -> Result<(), CoreError> {
        let mut instance_state = self
            .instances()
            .get(instance_name)
            .ok_or(CoreError::IncorrectInstanceId)?;
        let end_version = match instance_state.status {
            Some(InstanceStatus::Migrating(ref migration)) if migration.is_completed() => {
                migration.end_version.clone()
            }
            _ => return Err(CoreError::NoMigration),
        };

        self.local_migration_results().remove(instance_name);
        debug_assert!(*instance_state.data_version() < end_version);
        instance_state.data_version = Some(end_version);
        self.add_pending_status(instance_state, InstanceStatus::Stopped, None)
    }
}

/// Removes local migration result for specified service.
#[doc(hidden)]
pub fn remove_local_migration_result(fork: &Fork, service_name: &str) {
    Schema::new(fork)
        .local_migration_results()
        .remove(service_name);
}
