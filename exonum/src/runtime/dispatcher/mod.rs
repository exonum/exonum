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

use exonum_merkledb::{Fork, Snapshot};
use futures::{
    future::{self, Either},
    Future,
};

use std::{
    collections::{BTreeMap, HashMap},
    fmt, panic,
};

use crate::{
    blockchain::{Blockchain, IndexCoordinates, SchemaOrigin},
    crypto::Hash,
    helpers::ValidateInput,
    merkledb::BinaryValue,
    messages::{AnyTx, Verified},
    runtime::{ArtifactStatus, InstanceDescriptor, InstanceQuery, InstanceStatus},
};

use super::{
    error::ExecutionError, ArtifactId, ArtifactSpec, Caller, ExecutionContext, InstanceId,
    InstanceSpec, Runtime,
};

mod error;
mod schema;
#[cfg(test)]
mod tests;

#[derive(Debug)]
struct ServiceInfo {
    runtime_id: u32,
    name: String,
}

/// A collection of `Runtime`s capable of modifying the blockchain state.
#[derive(Debug)]
pub struct Dispatcher {
    runtimes: BTreeMap<u32, Box<dyn Runtime>>,
    service_infos: BTreeMap<InstanceId, ServiceInfo>,
}

impl Dispatcher {
    /// Creates a new dispatcher with the specified runtimes.
    pub(crate) fn new(
        blockchain: &Blockchain,
        runtimes: impl IntoIterator<Item = (u32, Box<dyn Runtime>)>,
    ) -> Self {
        let mut this = Self {
            runtimes: runtimes.into_iter().collect(),
            service_infos: BTreeMap::new(),
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
        for state in schema.artifacts().values() {
            debug_assert_eq!(
                state.status,
                ArtifactStatus::Active,
                "BUG: Artifact should not be in pending state."
            );
            self.deploy_artifact(state.spec.artifact, state.spec.payload)
                .wait()?;
        }
        // Restart active service instances.
        for state in schema.instances().values() {
            debug_assert_eq!(
                state.status,
                InstanceStatus::Active,
                "BUG: Service instance should not be in pending state."
            );
            self.start_service(snapshot, &state.spec)?;
        }
        // Notify runtimes about the end of initialization process.
        for runtime in self.runtimes.values_mut() {
            runtime.on_resume();
        }

        Ok(())
    }

    /// Add a built-in service with the predefined identifier.
    ///
    /// This method must be followed by the `after_commit()` call in order to persist information
    /// about deployed artifacts / services. Multiple `add_builtin_service()` calls can be covered
    /// by a single `after_commit()`.
    ///
    /// # Panics
    ///
    /// * If instance spec contains invalid service name or artifact id.
    pub(crate) fn add_builtin_service(
        &mut self,
        fork: &mut Fork,
        spec: InstanceSpec,
        artifact_payload: impl BinaryValue,
        constructor: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        // Register service artifact in the runtime.
        // TODO Write test for such situations [ECR-3222]
        if !self.is_artifact_deployed(&spec.artifact) {
            Self::commit_artifact(fork, spec.artifact.clone(), artifact_payload.to_bytes())?;
            // Wait until the artifact is ready to instantiate the service instances.
            self.block_until_deployed(spec.artifact.clone(), artifact_payload.into_bytes());
        }
        // Start the built-in service instance.
        ExecutionContext::new(self, fork, Caller::Blockchain)
            .start_adding_service(spec, constructor)?;
        Ok(())
    }

    pub(crate) fn state_hash(
        &self,
        access: &dyn Snapshot,
    ) -> impl IntoIterator<Item = (IndexCoordinates, Hash)> {
        let mut aggregator = HashMap::new();
        // Insert state hash of Dispatcher schema.
        aggregator.extend(IndexCoordinates::locate(
            SchemaOrigin::Dispatcher,
            Schema::new(access).state_hash(),
        ));
        // Insert state hashes for the runtimes.
        for (runtime_id, runtime) in &self.runtimes {
            let state = runtime.state_hashes(access);
            aggregator.extend(
                // Runtime state hash.
                IndexCoordinates::locate(SchemaOrigin::Runtime(*runtime_id), state.runtime),
            );
            for (instance_id, instance_hashes) in state.instances {
                aggregator.extend(
                    // Instance state hashes.
                    IndexCoordinates::locate(SchemaOrigin::Service(instance_id), instance_hashes),
                );
            }
        }
        aggregator
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
        payload: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        // TODO: revise dispatcher integrity checks [ECR-3743]
        debug_assert!(artifact.validate().is_ok(), "{:?}", artifact.validate());
        Schema::new(fork)
            .add_pending_artifact(ArtifactSpec {
                artifact: artifact.clone(),
                payload: payload.clone(),
            })
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

    // TODO documentation [ECR-3275]
    pub(crate) fn execute(
        &self,
        fork: &mut Fork,
        tx_id: Hash,
        tx: &Verified<AnyTx>,
    ) -> Result<(), ExecutionError> {
        let caller = Caller::Transaction {
            author: tx.author(),
            hash: tx_id,
        };
        let call_info = &tx.as_ref().call_info;
        let runtime = self
            .runtime_for_service(call_info.instance_id)
            .ok_or(Error::IncorrectRuntime)?;
        let context = ExecutionContext::new(self, fork, caller);
        runtime.execute(context, call_info, &tx.as_ref().arguments)
    }

    /// Calls `before_commit` for all currently active services, isolating each call.
    ///
    /// Changes the status of pending artifacts and services to active in the merkelized
    /// indexes of the dispatcher information scheme. Thus, these statuses will be equally
    /// calculated for precommit and actually committed block.
    pub(crate) fn before_commit(&self, fork: &mut Fork) {
        for (&service_id, info) in &self.service_infos {
            let context = ExecutionContext::new(self, fork, Caller::Blockchain);
            if self.runtimes[&info.runtime_id]
                .before_commit(context, service_id)
                .is_ok()
            {
                fork.flush();
            } else {
                fork.rollback();
            }
        }
        self.activate_pending(fork);
    }

    /// Commits to service instances and artifacts marked as pending in the provided `fork`.
    ///
    /// **NB.** Changes made to the `fork` in this method MUST be the same for all nodes.
    /// This is not checked by the consensus algorithm as usual.
    pub(crate) fn commit_block(&mut self, fork: &mut Fork) {
        // If the fork is dirty, `snapshot` will be outdated, which can trip
        // `Runtime::start_service()` calls.
        fork.flush();
        let snapshot = fork.snapshot_without_unflushed_changes();

        // Block futures with pending deployments.
        let mut schema = Schema::new(&*fork);
        for spec in schema.take_pending_artifacts() {
            self.block_until_deployed(spec.artifact, spec.payload);
        }
        // Start pending services.
        for spec in schema.take_pending_instances() {
            self.start_service(snapshot, &spec)
                .expect("Cannot start service");
        }
    }

    // Make pending artifacts and instances active.
    pub(crate) fn activate_pending(&self, fork: &Fork) {
        Schema::new(&*fork).activate_pending()
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

    /// Performs the complete set of operations after committing a block.
    ///
    /// This method should be called for all blocks except for the genesis block. For reasons
    /// described in `BlockchainMut::create_genesis_block()`, the processing of the genesis
    /// block is split into 2 parts.
    pub(crate) fn commit_block_and_notify_runtimes(&mut self, fork: &mut Fork) {
        self.commit_block(fork);
        self.notify_runtimes_about_commit(fork.snapshot_without_unflushed_changes());
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
    pub(crate) fn runtime_for_service(&self, instance_id: InstanceId) -> Option<&dyn Runtime> {
        let ServiceInfo { runtime_id, .. } = self.service_infos.get(&instance_id)?;
        let runtime = self.runtimes[&runtime_id].as_ref();
        Some(runtime)
    }

    /// Returns the service matching the specified query.
    pub(crate) fn get_service<'s>(
        &'s self,
        fork: &Fork,
        id: impl Into<InstanceQuery<'s>>,
    ) -> Option<InstanceDescriptor<'s>> {
        match id.into() {
            InstanceQuery::Id(id) => {
                let name = self.service_infos.get(&id)?.name.as_str();
                Some(InstanceDescriptor { id, name })
            }

            InstanceQuery::Name(name) => {
                // TODO: This may be slow.
                let id = Schema::new(fork).instances().get(name)?.spec.id;
                Some(InstanceDescriptor { id, name })
            }
        }
    }

    /// Notify the runtimes that it has to shutdown.
    pub(crate) fn shutdown(&mut self) {
        for runtime in self.runtimes.values_mut() {
            runtime.shutdown();
        }
    }

    /// Start a previously committed service instance.
    fn start_service(
        &mut self,
        snapshot: &dyn Snapshot,
        instance: &InstanceSpec,
    ) -> Result<(), ExecutionError> {
        // Notify the runtime that the service has been committed.
        let runtime = self
            .runtimes
            .get_mut(&instance.artifact.runtime_id)
            .ok_or(Error::IncorrectRuntime)?;
        runtime.commit_service(snapshot, instance)?;

        info!("Running service instance {:?}", instance);
        self.service_infos.insert(
            instance.id,
            ServiceInfo {
                runtime_id: instance.artifact.runtime_id,
                name: instance.name.to_owned(),
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
