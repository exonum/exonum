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
use futures::{future, Future};

use std::{
    collections::{BTreeMap, HashMap},
    panic,
};

use crate::{
    api::ApiBuilder,
    blockchain::{self, FatalError, IndexCoordinates, IndexOwner},
    crypto::{Hash, PublicKey, SecretKey},
    helpers::ValidateInput,
    merkledb::BinaryValue,
    messages::{AnyTx, Verified},
    node::ApiSender,
};

use super::{
    api::ApiContext,
    error::{catch_panic, ExecutionError},
    // TODO dispatcher shouldn't rely on the rust implementation
    rust::{interfaces::ConfigureCall, CallContext},
    ApiChange,
    ArtifactId,
    ArtifactProtobufSpec,
    BlockchainMailbox,
    CallInfo,
    Caller,
    CommunicationChannel,
    CommunicationChannelContext,
    ConfigChange,
    ExecutionContext,
    InstanceId,
    InstanceSpec,
    Runtime,
};

mod error;
mod schema;
#[cfg(test)]
mod tests;

/// Max instance identifier for builtin service.
///
/// By analogy with the privileged ports of the network, we use a range 0..1023 of instance
/// identifiers for built-in services which can be created only during the blockchain genesis
/// block creation.
pub const MAX_BUILTIN_INSTANCE_ID: InstanceId = 1024;

#[derive(Default)]
pub struct Dispatcher {
    runtimes: BTreeMap<u32, Box<dyn Runtime>>,
    runtime_lookup: HashMap<InstanceId, u32>,
    api_changes: BTreeMap<u32, Vec<ApiChange>>,
}

impl std::fmt::Debug for Dispatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("Dispatcher")
            .field("runtimes", &self.runtimes)
            .finish()
    }
}

impl Dispatcher {
    /// Create a new dispatcher with the specified runtimes.
    pub(crate) fn with_runtimes(
        runtimes: impl IntoIterator<Item = (u32, Box<dyn Runtime>)>,
    ) -> Self {
        Self {
            runtimes: runtimes.into_iter().collect(),
            runtime_lookup: HashMap::default(),
            api_changes: BTreeMap::new(),
        }
    }

    /// Restore the dispatcher from the state which was saved in the specified snapshot.
    pub(crate) fn restore_state(&mut self, snapshot: &dyn Snapshot) -> Result<(), ExecutionError> {
        let schema = Schema::new(snapshot);
        // Restore information about the deployed services.
        for (artifact, spec) in schema.artifacts_with_spec() {
            self.deploy_artifact(artifact.clone(), spec).wait()?;
        }
        // Restart active service instances.
        for instance in schema.service_instances().values() {
            self.restart_service(&instance)?;
        }
        Ok(())
    }

    /// Add a built-in service with the predefined identifier.
    ///
    /// # Panics
    ///
    /// * If instance spec contains invalid service name or artifact id.
    /// * If instance id is greater than [`MAX_BUILTIN_INSTANCE_ID`]
    ///
    /// [`MAX_BUILTIN_INSTANCE_ID`]: constant.MAX_BUILTIN_INSTANCE_ID.html
    pub(crate) fn add_builtin_service(
        &mut self,
        fork: &mut Fork,
        spec: InstanceSpec,
        artifact_spec: impl BinaryValue,
        constructor: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        assert!(
            spec.id < MAX_BUILTIN_INSTANCE_ID,
            "Instance identifier for builtin service should be lesser than {}",
            MAX_BUILTIN_INSTANCE_ID
        );
        // Register service artifact in the runtime.
        // TODO Write test for such situations [ECR-3222]
        if !self.is_artifact_deployed(&spec.artifact) {
            self.deploy_and_register_artifact(fork, &spec.artifact, artifact_spec)?;
        }
        // Start the built-in service instance.
        self.add_service(fork, spec, constructor)
    }

    pub(crate) fn state_hash(
        &self,
        access: &dyn Snapshot,
    ) -> impl IntoIterator<Item = (IndexCoordinates, Hash)> {
        let mut aggregator = HashMap::new();
        aggregator.extend(
            // Inserts state hashes for the dispatcher.
            IndexCoordinates::locate(IndexOwner::Dispatcher, Schema::new(access).state_hash()),
        );
        // Inserts state hashes for the runtimes.
        for (runtime_id, runtime) in &self.runtimes {
            let state = runtime.state_hashes(access);
            aggregator.extend(
                // Runtime state hash.
                IndexCoordinates::locate(IndexOwner::Runtime(*runtime_id), state.runtime),
            );
            for (instance_id, instance_hashes) in state.instances {
                aggregator.extend(
                    // Instance state hashes.
                    IndexCoordinates::locate(IndexOwner::Service(instance_id), instance_hashes),
                );
            }
        }
        aggregator
    }

    pub(crate) fn api_endpoints(
        &self,
        context: &ApiContext,
    ) -> impl IntoIterator<Item = (String, ApiBuilder)> {
        self.runtimes
            .values()
            .map(|runtime| {
                runtime
                    .api_endpoints(context)
                    .into_iter()
                    .map(|(service_name, builder)| (service_name, ApiBuilder::from(builder)))
            })
            .flatten()
            .collect::<Vec<_>>()
    }

    /// Initiate artifact deploy procedure in the corresponding runtime.
    ///
    /// # Panics
    ///
    /// * If artifact identifier is invalid.
    pub(crate) fn deploy_artifact(
        &mut self,
        artifact: ArtifactId,
        spec: impl BinaryValue,
    ) -> Box<dyn Future<Item = (), Error = ExecutionError>> {
        debug_assert!(artifact.validate().is_ok());

        if let Some(runtime) = self.runtimes.get_mut(&artifact.runtime_id) {
            runtime.deploy_artifact(artifact, spec.into_bytes())
        } else {
            Box::new(future::err(Error::IncorrectRuntime.into()))
        }
    }

    /// Register deployed artifact in the dispatcher's information schema.
    /// Make sure that you successfully complete the deploy artifact procedure.
    ///
    /// # Panics
    ///
    /// * If artifact identifier is invalid.
    /// * If artifact was not deployed.
    pub(crate) fn register_artifact(
        &mut self,
        fork: &Fork,
        artifact: &ArtifactId,
        spec: impl BinaryValue,
    ) -> Result<(), ExecutionError> {
        debug_assert!(artifact.validate().is_ok(), "{:?}", artifact.validate());

        // If for some reasons the artifact is not deployed, deploy it again.
        let spec = spec.into_bytes();
        if !self.is_artifact_deployed(&artifact) {
            self.deploy_artifact(artifact.clone(), spec.clone())
                .wait()
                .unwrap_or_else(|e| {
                    // In this case artifact deployment error is fatal because there are
                    // confirmation that this node can deploy this artifact.
                    panic!(FatalError::new(format!(
                        "Unable to deploy registered artifact. {}",
                        e
                    )))
                });
        }

        Schema::new(fork).add_artifact(artifact, spec)?;
        info!(
            "Registered artifact {} in runtime with id {}",
            artifact.name, artifact.runtime_id
        );
        Ok(())
    }

    pub(crate) fn deploy_and_register_artifact(
        &mut self,
        fork: &Fork,
        artifact: &ArtifactId,
        spec: impl BinaryValue,
    ) -> Result<(), ExecutionError> {
        let spec = spec.into_bytes();
        self.deploy_artifact(artifact.clone(), spec.clone())
            .wait()?;
        self.register_artifact(fork, &artifact, spec)
    }

    /// Add a new service instance. After that, write the information about the
    /// service instance to the dispatcher's information schema.
    ///
    /// # Panics
    ///
    /// * If instance spec contains invalid service name.
    pub(crate) fn add_service(
        &mut self,
        fork: &mut Fork,
        spec: InstanceSpec,
        constructor: impl BinaryValue,
    ) -> Result<(), ExecutionError> {
        debug_assert!(spec.validate().is_ok(), "{:?}", spec.validate());

        // Check that service doesn't use existing identifiers.
        if self.runtime_lookup.contains_key(&spec.id) {
            return Err(Error::ServiceIdExists.into());
        }
        // Try to add the service instance.
        let runtime = self
            .runtimes
            .get_mut(&spec.artifact.runtime_id)
            .ok_or(Error::IncorrectRuntime)?;
        runtime.add_service(fork, &spec, constructor.into_bytes())?;
        // Add service instance to the dispatcher schema.
        self.register_running_service(&spec);
        Schema::new(fork as &Fork)
            .add_service_instance(spec)
            .map_err(From::from)
    }

    // TODO documentation [ECR-3275]
    pub(crate) fn execute(
        &mut self,
        fork: &mut Fork,
        mailbox: &BlockchainMailbox,
        tx_id: Hash,
        tx: &Verified<AnyTx>,
    ) -> Result<(), ExecutionError> {
        let communication_channel =
            CommunicationChannel::new(CommunicationChannelContext::Tx, mailbox, self);

        let context = ExecutionContext::new(
            &communication_channel,
            fork,
            Caller::Transaction {
                author: tx.author(),
                hash: tx_id,
            },
        );
        self.call(&context, &tx.as_ref().call_info, &tx.as_ref().arguments)?;
        Ok(())
    }

    /// Call the corresponding runtime method.
    pub(crate) fn call(
        &self,
        context: &ExecutionContext,
        call_info: &CallInfo,
        arguments: &[u8],
    ) -> Result<(), ExecutionError> {
        let runtime_id = self
            .runtime_lookup
            .get(&call_info.instance_id)
            .ok_or(Error::IncorrectInstanceId)?;

        let runtime = self
            .runtimes
            .get(&runtime_id)
            .ok_or(Error::IncorrectRuntime)?;

        runtime.execute(context, call_info, arguments)
    }

    pub(crate) fn before_commit(&mut self, mailbox: &BlockchainMailbox, fork: &mut Fork) {
        let communication_channel =
            CommunicationChannel::new(CommunicationChannelContext::BeforeCommit, mailbox, self);
        for runtime in self.runtimes.values() {
            runtime.before_commit(&communication_channel, fork);
        }
    }

    pub(crate) fn after_commit(
        &mut self,
        mailbox: &BlockchainMailbox,
        snapshot: impl AsRef<dyn Snapshot>,
        service_keypair: &(PublicKey, SecretKey),
        tx_sender: &ApiSender,
    ) {
        let communication_channel =
            CommunicationChannel::new(CommunicationChannelContext::AfterCommit, mailbox, self);
        self.runtimes.values().for_each(|runtime| {
            runtime.after_commit(
                &communication_channel,
                snapshot.as_ref(),
                &service_keypair,
                &tx_sender,
            )
        });
    }

    /// Return additional information about the artifact if it is deployed.
    pub(crate) fn artifact_protobuf_spec(&self, id: &ArtifactId) -> Option<ArtifactProtobufSpec> {
        self.runtimes
            .get(&id.runtime_id)?
            .artifact_protobuf_spec(id)
    }

    /// Return true if the artifact with the given identifier is deployed.
    pub(crate) fn is_artifact_deployed(&self, id: &ArtifactId) -> bool {
        if let Some(runtime) = self.runtimes.get(&id.runtime_id) {
            runtime.is_artifact_deployed(id)
        } else {
            false
        }
    }

    /// Notify the runtime about API changes and return true if there are such changes.
    pub(crate) fn notify_api_changes(&mut self, context: &ApiContext) -> bool {
        let api_changes = {
            let mut api_changes = BTreeMap::default();
            std::mem::swap(&mut api_changes, &mut self.api_changes);
            api_changes
        };

        let has_changes = !api_changes.is_empty();
        for (runtime_id, changes) in api_changes {
            self.runtimes[&runtime_id].notify_api_changes(context, &changes)
        }
        has_changes
    }

    /// Notify the runtimes that it has to shutdown.
    pub(crate) fn shutdown(&self) {
        for runtime in self.runtimes.values() {
            runtime.shutdown();
        }
    }

    /// Register the service instance in the runtime lookup table.
    fn register_running_service(&mut self, instance: &InstanceSpec) {
        info!("Running service instance {:?}", instance);
        self.runtime_lookup
            .insert(instance.id, instance.artifact.runtime_id);
        // Add service instance to the list of modified APIs.
        let runtime_changes = self
            .api_changes
            .entry(instance.artifact.runtime_id)
            .or_default();
        runtime_changes.push(ApiChange::InstanceAdded(instance.id));
    }

    /// Restart a new previously added service instance.
    fn restart_service(&mut self, instance: &InstanceSpec) -> Result<(), ExecutionError> {
        let runtime = self
            .runtimes
            .get_mut(&instance.artifact.runtime_id)
            .ok_or(Error::IncorrectRuntime)?;
        runtime.restart_service(instance)?;
        self.register_running_service(&instance);
        Ok(())
    }

    /// Perform a configuration update with the specified changes.
    pub(crate) fn update_config(
        &self,
        mailbox: &BlockchainMailbox,
        fork: &mut Fork,
        caller_instance_id: InstanceId,
        changes: Vec<ConfigChange>,
    ) {
        // An error while configuring one of the service instances should not affect others.
        changes.into_iter().for_each(|change| match change {
            ConfigChange::Consensus(config) => {
                trace!("Updating consensus configuration {:?}", config);

                blockchain::Schema::new(fork as &Fork)
                    .consensus_config_entry()
                    .set(config);
                fork.flush();
            }

            ConfigChange::Service(config) => {
                trace!(
                    "Updating service instance configuration, instance ID is {}",
                    config.instance_id
                );


                let communication_channel =
                    CommunicationChannel::new(CommunicationChannelContext::AfterCommit, mailbox, self);

                let configure_result = catch_panic(|| {
                    let context = CallContext::new(
                        fork,
                        &communication_channel,
                        caller_instance_id,
                        config.instance_id,
                    );
                    ConfigureCall::from(context).apply_config(config.params)
                });

                match configure_result {
                    Ok(_) => fork.flush(),
                    Err(e) => {
                        fork.rollback();
                        error!("An error occurred while performing the service configuration apply. {}", e);
                    }
                }
            }
        })
    }

    /// Assigns an instance identificator to the new service instance.
    pub(crate) fn assign_instance_id(&self, fork: &Fork) -> InstanceId {
        Schema::new(fork as &Fork).assign_instance_id()
    }
}
