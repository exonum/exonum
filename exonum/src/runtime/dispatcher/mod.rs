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

pub use self::{error::Error, schema::Schema};

use exonum_merkledb::{Fork, Patch, Snapshot};
use futures::{
    future::{self, Either},
    Future,
};

use std::{
    collections::{BTreeMap, HashMap},
    fmt, panic,
};

use crate::{
    blockchain::{Blockchain, CallInBlock, Schema as CoreSchema},
    crypto::Hash,
    helpers::ValidateInput,
    messages::{AnyTx, Verified},
    runtime::{ArtifactStatus, InstanceDescriptor, InstanceQuery, InstanceStatus, RuntimeInstance},
};

use super::{
    error::{CallSite, CallType, ErrorKind, ExecutionError},
    ArtifactId, Caller, ExecutionContext, InstanceId, InstanceSpec, Runtime,
};

mod error;
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
    instances: HashMap<InstanceId, ServiceInfo>,
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
    ) -> Option<(InstanceDescriptor<'_>, InstanceStatus)> {
        let (id, info) = match id.into() {
            InstanceQuery::Id(id) => (id, self.instances.get(&id)?),

            InstanceQuery::Name(name) => {
                let id = *self.instance_names.get(name)?;
                (id, self.instances.get(&id)?)
            }
        };
        let name = info.name.as_str();
        Some((InstanceDescriptor { id, name }, info.status))
    }

    fn active_instances<'a>(&'a self) -> impl Iterator<Item = (InstanceId, u32)> + 'a {
        self.instances.iter().filter_map(|(&id, info)| {
            if info.status.is_active() {
                Some((id, info.runtime_id))
            } else {
                None
            }
        })
    }
}

/// A collection of `Runtime`s capable of modifying the blockchain state.
#[derive(Debug)]
pub struct Dispatcher {
    runtimes: BTreeMap<u32, Box<dyn Runtime>>,
    service_infos: CommittedServices,
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
        };
        for runtime in this.runtimes.values_mut() {
            runtime.initialize(blockchain);
        }
        this
    }

    /// Restore the dispatcher from the state which was saved in the specified snapshot.
    pub(crate) fn restore_state(&mut self, snapshot: &dyn Snapshot) -> Result<(), ExecutionError> {
        let schema = Schema::new(snapshot);
        // Restore information about the deployed services.
        for (artifact, state) in schema.artifacts().iter() {
            debug_assert_eq!(
                state.status,
                ArtifactStatus::Active,
                "BUG: Artifact should not be in pending state."
            );
            self.deploy_artifact(artifact, state.deploy_spec).wait()?;
        }
        // Restart active service instances.
        for state in schema.instances().values() {
            let status = state
                .status
                .expect("BUG: Stored service instance should have a determined state.");
            self.update_service_status(snapshot, &state.spec, status)?;
        }
        // Notify runtimes about the end of initialization process.
        for runtime in self.runtimes.values_mut() {
            runtime.on_resume();
        }

        Ok(())
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
        ExecutionContext::new(self, fork, Caller::Blockchain)
            .initiate_adding_service(spec, constructor)?;
        Ok(())
    }

    /// Starts all the built-in instances, creating a `Patch` with persisted changes.
    pub(crate) fn start_builtin_instances(&mut self, fork: Fork) -> Patch {
        // Mark services as active.
        self.activate_pending(&fork);
        // Start pending services.
        let mut schema = Schema::new(&fork);
        let pending_instances = schema.take_modified_instances();
        let patch = fork.into_patch();
        for (spec, status) in pending_instances {
            debug_assert_eq!(
                status,
                InstanceStatus::Active,
                "BUG: The built-in service instance must have an active status at startup."
            );
            self.update_service_status(&patch, &spec, status)
                .expect("Cannot start service");
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
    ) -> impl Future<Item = (), Error = ExecutionError> {
        // TODO: revise dispatcher integrity checks [ECR-3743]
        debug_assert!(artifact.validate().is_ok());

        if let Some(runtime) = self.runtimes.get_mut(&artifact.runtime_id) {
            Either::A(runtime.deploy_artifact(artifact, payload))
        } else {
            Either::B(future::err(Error::IncorrectRuntime.into()))
        }
    }

    /// Commits to the artifact deployment. This means that the node will cease working
    /// if the block with built on top of `fork` is committed until the artifact is deployed.
    ///
    /// Until the block built on top of `fork` is committed (or if `fork` is discarded in
    /// favor of another block proposal), no blocking is performed.
    pub(crate) fn commit_artifact(
        fork: &Fork,
        artifact: ArtifactId,
        deploy_spec: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        // TODO: revise dispatcher integrity checks [ECR-3743]
        debug_assert!(artifact.validate().is_ok(), "{:?}", artifact.validate());
        Schema::new(fork)
            .add_pending_artifact(artifact, deploy_spec)
            .map_err(From::from)
    }

    /// Initiates stopping of an existing service instance in the blockchain. The stopping
    /// service is active (i.e., processes transactions and the `after_transactions` hook)
    /// until the block built on top of the provided `fork` is committed.
    pub(crate) fn initiate_stopping_service(
        fork: &Fork,
        instance_id: InstanceId,
    ) -> Result<(), ExecutionError> {
        Schema::new(fork)
            .initiate_stopping_service(instance_id)
            .map_err(From::from)
    }

    fn block_until_deployed(&mut self, artifact: ArtifactId, payload: Vec<u8>) {
        if !self.is_artifact_deployed(&artifact) {
            self.deploy_artifact(artifact, payload)
                .wait()
                .unwrap_or_else(|e| {
                    // In this case artifact deployment error is fatal because there are
                    // confirmation that this node can deploy this artifact.
                    panic!("Unable to deploy registered artifact. {}", e)
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
            .ok_or(Error::IncorrectInstanceId)?;

        match instance.status {
            Some(InstanceStatus::Active) => Ok(()),
            _ => Err(Error::ServiceNotActive.into()),
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
        tx_index: u64,
        tx: &Verified<AnyTx>,
    ) -> Result<(), ExecutionError> {
        let caller = Caller::Transaction {
            author: tx.author(),
            hash: tx_id,
        };
        let call_info = &tx.as_ref().call_info;
        let (runtime_id, runtime) = self
            .runtime_for_service(call_info.instance_id)
            .ok_or(Error::IncorrectInstanceId)?;
        let context = ExecutionContext::new(self, fork, caller);

        let mut res = runtime.execute(context, call_info, &tx.as_ref().arguments);
        if let Err(ref mut err) = res {
            fork.rollback();

            err.set_runtime_id(runtime_id).set_call_site(|| CallSite {
                instance_id: call_info.instance_id,
                call_type: CallType::Method {
                    interface: String::new(),
                    id: call_info.method_id,
                },
            });
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
        call_type: CallType,
    ) -> Vec<(CallInBlock, ExecutionError)> {
        self.service_infos
            .active_instances()
            .filter_map(|(instance_id, runtime_id)| {
                let context = ExecutionContext::new(self, fork, Caller::Blockchain);
                let call_fn = match &call_type {
                    CallType::BeforeTransactions => Runtime::before_transactions,
                    CallType::AfterTransactions => Runtime::after_transactions,
                    _ => unreachable!(),
                };

                let res = call_fn(self.runtimes[&runtime_id].as_ref(), context, instance_id);
                if let Err(mut err) = res {
                    fork.rollback();
                    err.set_runtime_id(runtime_id).set_call_site(|| CallSite {
                        instance_id,
                        call_type: call_type.clone(),
                    });

                    let call = match &call_type {
                        CallType::BeforeTransactions => {
                            CallInBlock::before_transactions(instance_id)
                        }
                        CallType::AfterTransactions => CallInBlock::after_transactions(instance_id),
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
        self.call_service_hooks(fork, CallType::BeforeTransactions)
    }

    /// Calls `after_transactions` for all currently active services, isolating each call.
    ///
    /// Changes the status of pending artifacts and services to active in the merkelized
    /// indexes of the dispatcher information scheme. Thus, these statuses will be equally
    /// calculated for precommit and actually committed block.
    pub(crate) fn after_transactions(&self, fork: &mut Fork) -> Vec<(CallInBlock, ExecutionError)> {
        let errors = self.call_service_hooks(fork, CallType::AfterTransactions);
        self.activate_pending(fork);
        errors
    }

    /// Commits to service instances and artifacts marked as pending in the provided `fork`.
    pub(crate) fn commit_block(&mut self, fork: Fork) -> Patch {
        let mut schema = Schema::new(&fork);
        let pending_artifacts = schema.take_pending_artifacts();
        let modified_instances = schema.take_modified_instances();
        let patch = fork.into_patch();

        // Block futures with pending deployments.
        for (artifact, deploy_spec) in pending_artifacts {
            self.block_until_deployed(artifact, deploy_spec);
        }
        // Notify runtime about changes in service instances.
        for (spec, status) in modified_instances {
            self.update_service_status(&patch, &spec, status)
                .expect("Cannot commit service status");
        }
        patch
    }

    /// Make pending artifacts and instances active.
    pub(crate) fn activate_pending(&self, fork: &Fork) {
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
    ) -> Option<InstanceDescriptor<'_>> {
        let (descriptor, status) = self.service_infos.get_instance(id)?;
        if status.is_active() {
            Some(descriptor)
        } else {
            None
        }
    }

    /// Notify the runtimes that it has to shutdown.
    pub(crate) fn shutdown(&mut self) {
        for runtime in self.runtimes.values_mut() {
            runtime.shutdown();
        }
    }

    /// Commits service instance status to the corresponding runtime.
    pub(super) fn update_service_status(
        &mut self,
        snapshot: &dyn Snapshot,
        instance: &InstanceSpec,
        status: InstanceStatus,
    ) -> Result<(), ExecutionError> {
        // Notify the runtime that the service has been committed.
        let runtime = self
            .runtimes
            .get_mut(&instance.artifact.runtime_id)
            .ok_or(Error::IncorrectRuntime)?;
        runtime.update_service_status(snapshot, instance, status)?;

        info!(
            "Committing service instance {:?} with status {}",
            instance, status
        );

        self.service_infos.insert(
            instance.id,
            ServiceInfo {
                runtime_id: instance.artifact.runtime_id,
                name: instance.name.to_owned(),
                status,
            },
        );
        Ok(())
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

type ExecutionFuture = Box<dyn Future<Item = (), Error = ExecutionError> + Send>;

pub enum Action {
    StartDeploy {
        artifact: ArtifactId,
        spec: Vec<u8>,
        and_then: Box<dyn FnOnce() -> ExecutionFuture + Send>,
    },
}

impl fmt::Debug for Action {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Action::StartDeploy { artifact, spec, .. } => formatter
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
            Action::StartDeploy {
                artifact,
                spec,
                and_then,
            } => {
                dispatcher
                    .deploy_artifact(artifact.clone(), spec)
                    .and_then(|()| and_then())
                    .wait()
                    .unwrap_or_else(|e| {
                        error!("Deploying artifact {:?} failed: {}", artifact, e);
                    });
            }
        }
    }
}
