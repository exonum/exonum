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

pub use self::schema::{remove_local_migration_result, Schema};

use exonum_merkledb::{
    migration::{flush_migration, rollback_migration, AbortHandle, MigrationHelper},
    Database, Fork, Patch, Snapshot,
};
use log::{error, info};
use semver::Version;

use std::{
    collections::{BTreeMap, HashMap},
    fmt, panic,
    sync::{mpsc, Arc},
    thread,
};

use crate::{
    blockchain::{Blockchain, CallInBlock, Schema as CoreSchema},
    crypto::Hash,
    helpers::ValidateInput,
    messages::{AnyTx, Verified},
    runtime::{
        ArtifactStatus, CoreError, InstanceDescriptor, InstanceQuery, InstanceStatus,
        RuntimeInstance,
    },
};

use self::schema::{ArtifactAction, MigrationTransition, ModifiedInstanceInfo};
use super::{
    error::{CallSite, CallType, CommonError, ErrorKind, ExecutionError, ExecutionFail},
    migrations::{
        InstanceMigration, MigrationContext, MigrationError, MigrationScript, MigrationStatus,
        MigrationType,
    },
    ArtifactId, ExecutionContext, InstanceId, InstanceSpec, InstanceState, Runtime, RuntimeFeature,
};
use crate::runtime::RuntimeIdentifier;

#[cfg(test)]
mod migration_tests;
mod schema;
#[cfg(test)]
mod tests;

#[derive(Debug)]
struct ServiceInfo {
    runtime_id: u32,
    name: String,
    status: InstanceStatus,
}

/// Lookup table for the committed service instances.
#[derive(Debug, Default)]
struct CommittedServices {
    instances: BTreeMap<InstanceId, ServiceInfo>,
    instance_names: HashMap<String, InstanceId>,
}

impl CommittedServices {
    fn insert(&mut self, id: InstanceId, info: ServiceInfo) {
        let name = info.name.clone();
        self.instances.insert(id, info);
        self.instance_names.insert(name, id);
    }

    fn get_runtime_id_for_active_instance(&self, id: InstanceId) -> Option<u32> {
        self.instances.get(&id).and_then(|info| {
            if info.status.is_active() {
                Some(info.runtime_id)
            } else {
                None
            }
        })
    }

    fn get_instance<'q>(
        &self,
        id: impl Into<InstanceQuery<'q>>,
    ) -> Option<(InstanceDescriptor, &InstanceStatus)> {
        let (id, info) = match id.into() {
            InstanceQuery::Id(id) => (id, self.instances.get(&id)?),

            InstanceQuery::Name(name) => {
                let resolved_id = *self.instance_names.get(name)?;
                (resolved_id, self.instances.get(&resolved_id)?)
            }
        };
        Some((InstanceDescriptor::new(id, &info.name), &info.status))
    }

    fn active_instances<'a>(&'a self) -> impl Iterator<Item = (InstanceDescriptor, u32)> + 'a {
        self.instances.iter().filter_map(|(&id, info)| {
            if info.status.is_active() {
                let descriptor = InstanceDescriptor::new(id, &info.name);
                Some((descriptor, info.runtime_id))
            } else {
                None
            }
        })
    }
}

#[derive(Debug)]
struct MigrationThread {
    handle: thread::JoinHandle<Result<Hash, MigrationError>>,
    abort_handle: AbortHandle,
}

impl MigrationThread {
    fn join(self) -> MigrationStatus {
        let result = match self.handle.join() {
            Ok(Ok(hash)) => Ok(hash),
            Ok(Err(MigrationError::Custom(description))) => Err(description),
            Ok(Err(MigrationError::Helper(e))) => {
                panic!("Migration terminated with database error: {}", e);
            }
            Err(e) => Err(ExecutionError::description_from_panic(e)),
        };
        MigrationStatus(result)
    }
}

#[derive(Debug)]
struct Migrations {
    db: Arc<dyn Database>,
    threads: HashMap<String, MigrationThread>,
}

impl Migrations {
    fn new(blockchain: &Blockchain) -> Self {
        Self {
            db: blockchain.database().to_owned(),
            threads: HashMap::new(),
        }
    }

    fn add_migration(
        &mut self,
        instance_spec: InstanceSpec,
        data_version: Version,
        script: MigrationScript,
    ) {
        let db = Arc::clone(&self.db);
        let instance_name = instance_spec.name.clone();
        let script_name = script.name().to_owned();
        let (handle_tx, handle_rx) = mpsc::channel();

        let thread_fn = move || -> Result<Hash, MigrationError> {
            let script_name = script.name().to_owned();
            log::info!("Starting migration script {}", script_name);

            let (helper, abort_handle) =
                MigrationHelper::with_handle(Arc::clone(&db), &instance_spec.name);
            handle_tx.send(abort_handle).unwrap();
            let mut context = MigrationContext::new(helper, instance_spec, data_version);

            script.execute(&mut context)?;
            let migration_hash = context.helper.finish()?;
            log::info!(
                "Successfully finished migration script {} with hash {:?}",
                script_name,
                migration_hash
            );
            Ok(migration_hash)
        };

        let handle = thread::Builder::new()
            .name(script_name)
            .spawn(thread_fn)
            .expect("Cannot spawn thread for migration script");

        let prev_thread = self.threads.insert(
            instance_name.clone(),
            MigrationThread {
                handle,
                abort_handle: handle_rx.recv().unwrap(),
            },
        );
        debug_assert!(
            prev_thread.is_none(),
            "Attempt to run concurrent migrations for service `{}`",
            instance_name
        );
    }

    fn take_completed(&mut self) -> Vec<(String, MigrationStatus)> {
        let completed_names: Vec<_> = self
            .threads
            .iter()
            .filter_map(|(name, thread)| {
                if thread.abort_handle.is_finished() {
                    Some(name.to_owned())
                } else {
                    None
                }
            })
            .collect();

        completed_names
            .into_iter()
            .map(|name| {
                let thread = self.threads.remove(&name).unwrap();
                (name, thread.join())
            })
            .collect()
    }
}

/// A collection of `Runtime`s capable of modifying the blockchain state.
#[derive(Debug)]
pub struct Dispatcher {
    runtimes: BTreeMap<u32, Box<dyn Runtime>>,
    service_infos: CommittedServices,
    migrations: Migrations,
}

impl Dispatcher {
    /// Creates a new dispatcher with the specified runtimes.
    pub(crate) fn new(
        blockchain: &Blockchain,
        runtimes: impl IntoIterator<Item = RuntimeInstance>,
    ) -> Self {
        let mut this = Self {
            runtimes: runtimes
                .into_iter()
                .map(|runtime| (runtime.id, runtime.instance))
                .collect(),
            service_infos: CommittedServices::default(),
            migrations: Migrations::new(blockchain),
        };
        for runtime in this.runtimes.values_mut() {
            runtime.initialize(blockchain);
        }
        this
    }

    /// Restore the dispatcher from the state which was saved in the specified snapshot.
    ///
    /// # Panics
    ///
    /// This method panics if the stored state cannot be restored.
    pub(crate) fn restore_state(&mut self, snapshot: &dyn Snapshot) {
        let schema = Schema::new(snapshot);

        // Restore information about the deployed services.
        for (artifact, state) in schema.artifacts().iter() {
            debug_assert_eq!(
                state.status,
                ArtifactStatus::Active,
                "BUG: Artifact should not be in pending state."
            );

            self.deploy_artifact(artifact.clone(), state.deploy_spec)
                .unwrap_or_else(|err| {
                    panic!(
                        "BUG: Cannot restore blockchain state; artifact `{}` failed to deploy \
                         after successful previous deployment. Reported error: {}",
                        artifact, err
                    );
                });
        }

        // Restart active service instances.
        for state in schema.instances().values() {
            let data_version = state.data_version().to_owned();
            self.update_service_status(snapshot, &state);

            // Restart a migration script if it is not finished locally.
            let status = state
                .status
                .expect("BUG: Stored service instance should have a determined status.");
            if let Some(target) = status.ongoing_migration_target() {
                if schema.local_migration_result(&state.spec.name).is_none() {
                    self.start_migration_script(target, state.spec, data_version);
                }
            }
        }

        // Notify runtimes about the end of initialization process.
        for runtime in self.runtimes.values_mut() {
            runtime.on_resume();
        }
    }

    /// Adds a built-in artifact to the dispatcher. Unlike artifacts added via `commit_artifact` +
    /// `deploy_artifact`, this method skips artifact commitment; the artifact
    /// is synchronously deployed and marked as `Active`.
    ///
    /// # Panics
    ///
    /// This method treats errors during artifact deployment as fatal and panics on them.
    pub(crate) fn add_builtin_artifact(
        &mut self,
        fork: &Fork,
        artifact: ArtifactId,
        payload: Vec<u8>,
    ) {
        Schema::new(fork)
            .add_active_artifact(&artifact, payload.clone())
            .unwrap_or_else(|err| {
                panic!("Cannot deploy a built-in artifact: {}", err);
            });
        self.deploy_artifact(artifact, payload)
            .unwrap_or_else(|err| panic!("Cannot deploy a built-in artifact: {}", err));
    }

    /// Add a built-in service with the predefined identifier.
    ///
    /// This method must be followed by the `start_builtin_instances()` call in order
    /// to persist information about deployed artifacts / services.
    /// Multiple `add_builtin_service()` calls can be covered by a single `start_builtin_instances()`.
    ///
    /// # Panics
    ///
    /// * If instance spec contains invalid service name or artifact id.
    pub(crate) fn add_builtin_service(
        &mut self,
        fork: &mut Fork,
        spec: InstanceSpec,
        constructor: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        // Start the built-in service instance.
        let name = spec.name.clone();

        let mut should_rollback = false;
        let mut res = ExecutionContext::for_block_call(
            self,
            fork,
            &mut should_rollback,
            InstanceDescriptor::new(spec.id, &name),
        )
        .initiate_adding_service(spec, constructor);

        if should_rollback && res.is_ok() {
            res = Err(CoreError::IncorrectCall.into());
        }
        res
    }

    /// Starts all the built-in instances, creating a `Patch` with persisted changes.
    pub(crate) fn start_builtin_instances(&mut self, fork: Fork) -> Patch {
        // Mark services as active.
        Self::activate_pending(&fork);
        // Start pending services.
        let mut schema = Schema::new(&fork);
        let pending_instances = schema.take_modified_instances();
        let patch = fork.into_patch();
        for (state, _) in pending_instances {
            debug_assert_eq!(
                state.status,
                Some(InstanceStatus::Active),
                "BUG: The built-in service instance must have an active status at startup"
            );
            self.update_service_status(&patch, &state);
        }
        patch
    }

    /// Initiate artifact deploy procedure in the corresponding runtime. If the deploy
    /// is successful, the artifact spec will be written into `artifact_sources`.
    ///
    /// # Panics
    ///
    /// * If artifact identifier is invalid.
    pub(crate) fn deploy_artifact(
        &mut self,
        artifact: ArtifactId,
        payload: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        // TODO: revise dispatcher integrity checks [ECR-3743]
        debug_assert!(artifact.validate().is_ok());

        if let Some(runtime) = self.runtimes.get_mut(&artifact.runtime_id) {
            let runtime_id = artifact.runtime_id;
            runtime
                .deploy_artifact(artifact, payload)
                .wait()
                .map_err(move |mut err| {
                    err.set_runtime_id(runtime_id);
                    err
                })
        } else {
            let msg = format!(
                "Cannot deploy an artifact `{}` depending on the unknown runtime with ID {}",
                artifact, artifact.runtime_id
            );
            Err(CoreError::IncorrectRuntime.with_description(msg))
        }
    }

    /// Commits to the artifact deployment. This means that the node will cease working
    /// if the block with built on top of `fork` is committed until the artifact is deployed.
    ///
    /// Until the block built on top of `fork` is committed (or if `fork` is discarded in
    /// favor of another block proposal), no blocking is performed.
    ///
    /// # Panics
    ///
    /// This method assumes that `deploy_artifact` was previously called for the corresponding
    /// `ArtifactId` and deployment was completed successfully.
    /// If any error happens within `commit_artifact`, it is considered either a bug in the
    /// `Supervisor` service or `Dispatcher` itself, and as a result, this method will panic.
    pub(crate) fn commit_artifact(fork: &Fork, artifact: &ArtifactId, deploy_spec: Vec<u8>) {
        debug_assert!(artifact.validate().is_ok(), "{:?}", artifact.validate());
        Schema::new(fork)
            .add_pending_artifact(artifact, deploy_spec)
            .unwrap_or_else(|err| panic!("BUG: Can't commit the artifact, error: {}", err));
    }

    pub(crate) fn unload_artifact(
        fork: &Fork,
        artifact: &ArtifactId,
    ) -> Result<(), ExecutionError> {
        Schema::new(fork).unload_artifact(artifact)
    }

    /// Initiates migration of an existing stopped service to a newer artifact.
    /// The migration script is started once the block corresponding to `fork`
    /// is committed.
    pub(crate) fn initiate_migration(
        &self,
        fork: &Fork,
        new_artifact: ArtifactId,
        service_name: &str,
    ) -> Result<MigrationType, ExecutionError> {
        let mut schema = Schema::new(fork);
        let instance_state = schema.check_migration_initiation(&new_artifact, service_name)?;
        let maybe_script =
            self.get_migration_script(&new_artifact, instance_state.data_version())?;
        let migration_type = if let Some(script) = maybe_script {
            let migration = InstanceMigration::new(new_artifact, script.end_version().to_owned());
            schema.add_pending_migration(instance_state, migration);
            MigrationType::Async
        } else {
            // No migration script means that the service instance may be immediately updated to
            // the new artifact version.
            schema.fast_forward_migration(instance_state, new_artifact);
            MigrationType::FastForward
        };
        Ok(migration_type)
    }

    /// Initiates migration rollback. The rollback will actually be performed once
    /// the block corresponding to `fork` is committed.
    pub(crate) fn rollback_migration(
        fork: &Fork,
        service_name: &str,
    ) -> Result<(), ExecutionError> {
        Schema::new(fork)
            .add_migration_rollback(service_name)
            .map_err(From::from)
    }

    /// Makes the node block on the specified migration after the block corresponding
    /// to `fork` is committed. After the block is committed, all nodes in the network
    /// are guaranteed to have migration data prepared for flushing.
    pub(crate) fn commit_migration(
        fork: &Fork,
        service_name: &str,
        migration_hash: Hash,
    ) -> Result<(), ExecutionError> {
        Schema::new(fork)
            .add_migration_commit(service_name, migration_hash)
            .map_err(From::from)
    }

    pub(crate) fn flush_migration(
        fork: &mut Fork,
        service_name: &str,
    ) -> Result<(), ExecutionError> {
        Schema::new(&*fork).complete_migration(service_name)?;
        flush_migration(fork, service_name);
        Ok(())
    }

    /// Initiates stopping an existing service instance in the blockchain. The stopping
    /// service is active (i.e., processes transactions and the `after_transactions` hook)
    /// until the block built on top of the provided `fork` is committed.
    pub(crate) fn initiate_stopping_service(
        fork: &Fork,
        instance_id: InstanceId,
    ) -> Result<(), ExecutionError> {
        Schema::new(fork)
            .initiate_simple_service_transition(instance_id, InstanceStatus::Stopped)
            .map_err(From::from)
    }

    pub(crate) fn initiate_freezing_service(
        &self,
        fork: &Fork,
        instance_id: InstanceId,
    ) -> Result<(), ExecutionError> {
        let mut schema = Schema::new(fork);
        let instance_state = schema.get_instance(instance_id).ok_or_else(|| {
            let msg = format!("Cannot freeze unknown service {}", instance_id);
            CoreError::IncorrectInstanceId.with_description(msg)
        })?;

        let runtime_id = instance_state.spec.artifact.runtime_id;
        let runtime = self.runtime_by_id(runtime_id).unwrap_or_else(|| {
            panic!(
                "BUG: runtime absent for an artifact `{}` associated with service `{}`",
                instance_state.spec.artifact,
                instance_state.spec.as_descriptor()
            );
        });

        if !runtime.is_supported(&RuntimeFeature::FreezingServices) {
            let runtime_description = RuntimeIdentifier::transform(runtime_id).ok().map_or_else(
                || format!("Runtime with ID {}", runtime_id),
                |id| id.to_string(),
            );
            let msg = format!("{} does not support freezing services", runtime_description);
            return Err(CommonError::FeatureNotSupported.with_description(msg));
        }

        schema
            .initiate_simple_service_transition(instance_id, InstanceStatus::Frozen)
            .map_err(From::from)
    }

    fn block_until_deployed(&mut self, artifact: ArtifactId, payload: Vec<u8>) {
        if !self.is_artifact_deployed(&artifact) {
            self.deploy_artifact(artifact, payload).unwrap_or_else(|e| {
                // In this case artifact deployment error is fatal because the deploy
                // was committed on the network level.
                panic!("Unable to deploy registered artifact. {}", e);
            });
        }
    }

    /// Performs several shallow checks that transaction is correct.
    ///
    /// Returned `Ok(())` value doesn't necessarily mean that transaction is correct and will be
    /// executed successfully, but returned `Err(..)` value means that this transaction is
    /// **obviously** incorrect and should be declined as early as possible.
    pub(crate) fn check_tx(
        snapshot: &dyn Snapshot,
        tx: &Verified<AnyTx>,
    ) -> Result<(), ExecutionError> {
        // Currently the only check is that destination service exists, but later
        // functionality of this method can be extended.
        let call_info = &tx.as_ref().call_info;
        let instance = Schema::new(snapshot)
            .get_instance(call_info.instance_id)
            .ok_or_else(|| {
                let msg = format!(
                    "Cannot dispatch transaction to unknown service with ID {}",
                    call_info.instance_id
                );
                CoreError::IncorrectInstanceId.with_description(msg)
            })?;

        match instance.status {
            Some(InstanceStatus::Active) => Ok(()),
            status => {
                let status_str = status.map_or_else(|| "none".to_owned(), |st| st.to_string());
                let msg = format!(
                    "Cannot dispatch transaction to non-active service `{}` (status: {})",
                    instance.spec.as_descriptor(),
                    status_str
                );
                Err(CoreError::ServiceNotActive.with_description(msg))
            }
        }
    }

    fn report_error(err: &ExecutionError, fork: &Fork, call: CallInBlock) {
        let height = CoreSchema::new(fork).next_height();
        if err.kind() == ErrorKind::Unexpected {
            log::error!(
                "{} at {:?} resulted in unexpected error: {:?}",
                call,
                height,
                err
            );
        } else {
            log::info!("{} at {:?} failed: {:?}", call, height, err);
        }
    }

    /// Executes transaction with the specified ID with fork isolation.
    pub(crate) fn execute(
        &self,
        fork: &mut Fork,
        tx_id: Hash,
        tx_index: u32,
        tx: &Verified<AnyTx>,
    ) -> Result<(), ExecutionError> {
        let call_info = &tx.as_ref().call_info;
        let (runtime_id, runtime) =
            self.runtime_for_service(call_info.instance_id)
                .ok_or_else(|| {
                    let msg = format!(
                        "Cannot dispatch transaction to unknown service with ID {}",
                        call_info.instance_id
                    );
                    CoreError::IncorrectInstanceId.with_description(msg)
                })?;

        let instance = self.get_service(call_info.instance_id).ok_or_else(|| {
            let msg = format!(
                "Cannot dispatch transaction to inactive service with ID {}",
                call_info.instance_id
            );
            CoreError::IncorrectInstanceId.with_description(msg)
        })?;

        let mut should_rollback = false;
        let context = ExecutionContext::for_transaction(
            self,
            fork,
            &mut should_rollback,
            instance,
            tx.author(),
            tx_id,
        );

        let mut res = runtime.execute(context, call_info.method_id, &tx.as_ref().arguments);
        if should_rollback && res.is_ok() {
            res = Err(CoreError::IncorrectCall.into());
        }

        if let Err(ref mut err) = res {
            fork.rollback();

            err.set_runtime_id(runtime_id)
                .set_call_site(|| CallSite::from_call_info(call_info, ""));
            Self::report_error(err, fork, CallInBlock::transaction(tx_index));
        } else {
            fork.flush();
        }
        res
    }

    /// Calls service hooks of the specified type for all active services.
    fn call_service_hooks(
        &self,
        fork: &mut Fork,
        call_type: &CallType,
    ) -> Vec<(CallInBlock, ExecutionError)> {
        self.service_infos
            .active_instances()
            .filter_map(|(instance, runtime_id)| {
                let mut should_rollback = false;
                let context = ExecutionContext::for_block_call(
                    self,
                    fork,
                    &mut should_rollback,
                    instance.clone(),
                );
                let call_fn = match &call_type {
                    CallType::BeforeTransactions => Runtime::before_transactions,
                    CallType::AfterTransactions => Runtime::after_transactions,
                    _ => unreachable!(),
                };

                let mut res = call_fn(self.runtimes[&runtime_id].as_ref(), context);
                if should_rollback && res.is_ok() {
                    res = Err(CoreError::IncorrectCall.into());
                }

                if let Err(mut err) = res {
                    fork.rollback();
                    err.set_runtime_id(runtime_id)
                        .set_call_site(|| CallSite::new(instance.id, call_type.clone()));

                    let call = match &call_type {
                        CallType::BeforeTransactions => {
                            CallInBlock::before_transactions(instance.id)
                        }
                        CallType::AfterTransactions => CallInBlock::after_transactions(instance.id),
                        _ => unreachable!(),
                    };
                    Self::report_error(&err, fork, call);
                    Some((call, err))
                } else {
                    fork.flush();
                    None
                }
            })
            .collect()
    }

    /// Calls `before_transactions` for all currently active services, isolating each call.
    pub(crate) fn before_transactions(
        &self,
        fork: &mut Fork,
    ) -> Vec<(CallInBlock, ExecutionError)> {
        self.call_service_hooks(fork, &CallType::BeforeTransactions)
    }

    /// Calls `after_transactions` for all currently active services, isolating each call.
    ///
    /// Changes the status of pending artifacts and services to active in the merkelized
    /// indexes of the dispatcher information scheme. Thus, these statuses will be equally
    /// calculated for precommit and actually committed block.
    pub(crate) fn after_transactions(&self, fork: &mut Fork) -> Vec<(CallInBlock, ExecutionError)> {
        let errors = self.call_service_hooks(fork, &CallType::AfterTransactions);
        Self::activate_pending(fork);
        errors
    }

    /// Commits to service instances and artifacts marked as pending in the provided `fork`.
    pub(crate) fn commit_block(&mut self, mut fork: Fork) -> Patch {
        let mut schema = Schema::new(&fork);
        let pending_artifacts = schema.take_pending_artifacts();
        let modified_instances = schema.take_modified_instances();

        // Process migration commits and rollbacks.
        self.block_on_migrations(&modified_instances, &mut schema);
        self.rollback_migrations(&modified_instances, &mut fork);

        // Check if any migration scripts have completed locally. Record migration results in the DB.
        let results = self.migrations.take_completed();
        let mut schema = Schema::new(&fork);
        for (instance_name, result) in results {
            schema.add_local_migration_result(&instance_name, result);
        }

        let patch = fork.into_patch();

        // Process changed artifacts, blocking on futures with pending deployments.
        for (artifact, action) in pending_artifacts {
            match action {
                ArtifactAction::Deploy(deploy_spec) => {
                    self.block_until_deployed(artifact, deploy_spec);
                }
                ArtifactAction::Unload => {
                    let runtime = self
                        .runtimes
                        .get_mut(&artifact.runtime_id)
                        .expect("BUG: Cannot obtain runtime for an unloaded artifact");
                    runtime.unload_artifact(&artifact);
                }
            }
        }

        // Notify runtime about changes in service instances.
        for (state, modified_info) in modified_instances {
            let data_version = state.data_version().to_owned();
            let status = state
                .status
                .as_ref()
                .expect("BUG: Service status cannot be changed to `None`");

            self.update_service_status(&patch, &state);
            if modified_info.migration_transition == Some(MigrationTransition::Start) {
                let target = status
                    .ongoing_migration_target()
                    .expect("BUG: Migration target is not specified for ongoing migration");
                self.start_migration_script(target, state.spec, data_version);
            }
        }

        patch
    }

    fn get_migration_script(
        &self,
        new_artifact: &ArtifactId,
        data_version: &Version,
    ) -> Result<Option<MigrationScript>, ExecutionError> {
        let runtime = self.runtime_by_id(new_artifact.runtime_id).ok_or_else(|| {
            let msg = format!(
                "Cannot extract a migration script from artifact `{}` which corresponds to \
                 unknown runtime with ID {}",
                new_artifact, new_artifact.runtime_id,
            );
            CoreError::IncorrectRuntime.with_description(msg)
        })?;
        runtime
            .migrate(new_artifact, data_version)
            .map_err(From::from)
    }

    fn start_migration_script(
        &mut self,
        new_artifact: &ArtifactId,
        old_service: InstanceSpec,
        data_version: Version,
    ) {
        let maybe_script = self
            .get_migration_script(new_artifact, &data_version)
            .unwrap_or_else(|err| {
                panic!(
                    "BUG: Cannot obtain migration script for migrating {:?} to new artifact {:?}, {}",
                    old_service, new_artifact, err
                );
            });
        let script = maybe_script.unwrap_or_else(|| {
            panic!(
                "BUG: Runtime returned no script for migrating {:?} to new artifact {:?}, \
                 although it earlier returned a script for the same migration",
                old_service, new_artifact
            );
        });
        self.migrations
            .add_migration(old_service, data_version, script);
    }

    /// Blocks until all committed migrations are completed with the expected outcome.
    /// The node will panic if the local outcome of a migration is unexpected.
    fn block_on_migrations(
        &mut self,
        modified_instances: &[(InstanceState, ModifiedInstanceInfo)],
        schema: &mut Schema<&Fork>,
    ) {
        let committed_migrations = modified_instances.iter().filter(|(_, modified_info)| {
            modified_info.migration_transition == Some(MigrationTransition::Commit)
        });
        for (state, _) in committed_migrations {
            let migration_hash = state
                .status
                .as_ref()
                .and_then(InstanceStatus::completed_migration_hash)
                .expect("BUG: No migration hash saved for committed migration");

            let instance_name = &state.spec.name;
            let local_result = schema.local_migration_result(instance_name);
            let local_result = self.block_on_migration(instance_name, migration_hash, local_result);
            schema.add_local_migration_result(instance_name, local_result);
        }
    }

    fn block_on_migration(
        &mut self,
        namespace: &str,
        global_hash: Hash,
        local_result: Option<MigrationStatus>,
    ) -> MigrationStatus {
        let local_result = if let Some(thread) = self.migrations.threads.remove(namespace) {
            // If the migration script hasn't finished locally, wait until it's finished.
            thread.join()
        } else {
            // If the local script has finished, the result should be recorded in the database.
            local_result.unwrap_or_else(|| {
                panic!(
                    "BUG: migration is marked as completed for service `{}`, but its result \
                     is missing from the database",
                    namespace
                );
            })
        };

        // Check if the local result agrees with the global one. Any deviation is considered
        // a consensus failure.
        let res = local_result.0.as_ref();
        let local_hash = *res.unwrap_or_else(|err| {
            panic!(
                "Migration for service `{}` is committed with migration hash {:?}, \
                 but locally it has finished with an error: {}. You can remove local \
                 migration result with CLI maintenance command `restart-migration`.",
                namespace, global_hash, err
            );
        });
        assert!(
            local_hash == global_hash,
            "Migration for service `{}` is committed with migration hash {:?}, \
             but locally it has finished with another hash {:?}. You can remove local \
             migration result with CLI maintenance command `restart-migration`.",
            namespace,
            global_hash,
            local_hash
        );

        local_result
    }

    fn rollback_migrations(
        &mut self,
        modified_instances: &[(InstanceState, ModifiedInstanceInfo)],
        fork: &mut Fork,
    ) {
        let migration_rollbacks = modified_instances.iter().filter(|(_, modified_info)| {
            modified_info.migration_transition == Some(MigrationTransition::Rollback)
        });
        for (state, _) in migration_rollbacks {
            // Remove the thread corresponding to the migration (if any). This will abort
            // the migration script since its `AbortHandle` is dropped.
            let namespace = &state.spec.name;
            self.migrations.threads.remove(namespace);
            rollback_migration(fork, namespace);
        }
    }

    /// Make pending artifacts and instances active.
    pub(crate) fn activate_pending(fork: &Fork) {
        Schema::new(fork).activate_pending()
    }

    /// Notifies runtimes about a committed block.
    pub(crate) fn notify_runtimes_about_commit(&mut self, snapshot: &dyn Snapshot) {
        let mut mailbox = Mailbox::default();
        for runtime in self.runtimes.values_mut() {
            runtime.after_commit(snapshot, &mut mailbox);
        }
        for action in mailbox.actions {
            action.execute(self);
        }
    }

    /// Performs the complete set of operations after committing a block. Returns a patch
    /// corresponding to the fork.
    ///
    /// This method should be called for all blocks except for the genesis block. For reasons
    /// described in `BlockchainMut::create_genesis_block()`, the processing of the genesis
    /// block is split into 2 parts.
    pub(crate) fn commit_block_and_notify_runtimes(&mut self, fork: Fork) -> Patch {
        let patch = self.commit_block(fork);
        self.notify_runtimes_about_commit(&patch);
        patch
    }

    /// Return true if the artifact with the given identifier is deployed.
    pub(crate) fn is_artifact_deployed(&self, id: &ArtifactId) -> bool {
        if let Some(runtime) = self.runtimes.get(&id.runtime_id) {
            runtime.is_artifact_deployed(id)
        } else {
            false
        }
    }

    /// Looks up a runtime by its identifier.
    pub(crate) fn runtime_by_id(&self, id: u32) -> Option<&dyn Runtime> {
        self.runtimes.get(&id).map(AsRef::as_ref)
    }

    /// Looks up the runtime for the specified service instance. Returns a reference to
    /// the runtime, or `None` if the service with the specified instance ID does not exist.
    pub(crate) fn runtime_for_service(
        &self,
        instance_id: InstanceId,
    ) -> Option<(u32, &dyn Runtime)> {
        let runtime_id = self
            .service_infos
            .get_runtime_id_for_active_instance(instance_id)?;
        let runtime = self.runtimes[&runtime_id].as_ref();
        Some((runtime_id, runtime))
    }

    /// Returns the service matching the specified query.
    pub(crate) fn get_service<'q>(
        &self,
        id: impl Into<InstanceQuery<'q>>,
    ) -> Option<InstanceDescriptor> {
        let (descriptor, status) = self.service_infos.get_instance(id)?;
        if status.is_active() {
            Some(descriptor)
        } else {
            None
        }
    }

    /// Commits service instance status to the corresponding runtime.
    ///
    /// # Panics
    ///
    /// This method assumes that it was previously checked if runtime can change the state
    /// of the service, and will panic if it cannot be done.
    fn update_service_status(&mut self, snapshot: &dyn Snapshot, instance: &InstanceState) {
        let runtime_id = instance.spec.artifact.runtime_id;
        // Notify the runtime that the service has been committed.
        let runtime = self.runtimes.get_mut(&runtime_id).expect(
            "BUG: `update_service_status` was invoked for incorrect runtime, \
             this should never happen because of preemptive checks.",
        );
        runtime.update_service_status(snapshot, instance);

        let status = instance
            .status
            .clone()
            .expect("BUG: instance status cannot change to `None`");
        info!(
            "Committing service instance {:?} with status {}",
            instance.spec, status
        );

        self.service_infos.insert(
            instance.spec.id,
            ServiceInfo {
                runtime_id,
                name: instance.spec.name.clone(),
                status,
            },
        );
    }
}

/// Mailbox accumulating `Action`s to be performed by the dispatcher.
#[derive(Debug, Default)]
pub struct Mailbox {
    actions: Vec<Action>,
}

impl Mailbox {
    /// Appends a new action to be performed by the dispatcher.
    pub fn push(&mut self, action: Action) {
        self.actions.push(action);
    }
}

/// The actions that will be performed after the deployment is finished.
pub type ThenFn = Box<dyn FnOnce(Result<(), ExecutionError>) -> Result<(), ExecutionError> + Send>;

/// Action to be performed by the dispatcher.
#[non_exhaustive]
pub enum Action {
    /// Start artifact deployment.
    StartDeploy {
        /// Information uniquely identifying the artifact.
        artifact: ArtifactId,
        /// Runtime-specific artifact payload.
        spec: Vec<u8>,
        /// The actions that will be performed after the deployment is finished.
        /// For example, this closure may create a transaction with the deployment confirmation.
        then: ThenFn,
    },
}

impl fmt::Debug for Action {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StartDeploy { artifact, spec, .. } => formatter
                .debug_struct("StartDeploy")
                .field("artifact", artifact)
                .field("spec", spec)
                .finish(),
        }
    }
}

impl Action {
    fn execute(self, dispatcher: &mut Dispatcher) {
        match self {
            Self::StartDeploy {
                artifact,
                spec,
                then,
            } => {
                then(dispatcher.deploy_artifact(artifact.clone(), spec)).unwrap_or_else(|e| {
                    error!("Deploying artifact {:?} failed: {}", artifact, e);
                });
            }
        }
    }
}
