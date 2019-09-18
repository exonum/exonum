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
    cell::RefCell,
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
    rust::{
        interfaces::{ConfigureCall, Initialize, INITIALIZE_METHOD_ID},
        Interface,
    },
    ApiChange, ArtifactId, ArtifactProtobufSpec, CallContext, CallInfo, Caller, ConfigChange,
    ExecutionContext, InstanceDescriptor, InstanceId, InstanceSpec, Runtime,
};

mod error;
mod schema;

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
        fork: &Fork,
        spec: InstanceSpec,
        constructor: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        assert!(
            spec.id < MAX_BUILTIN_INSTANCE_ID,
            "Instance identifier for builtin service should be lesser than {}",
            MAX_BUILTIN_INSTANCE_ID
        );
        // Built-in services should not have an additional specification.
        let artifact_spec = Vec::default();
        // Register service artifact in the runtime.
        // TODO Write test for such situations [ECR-3222]
        if !self.is_artifact_deployed(&spec.artifact) {
            self.deploy_and_register_artifact(fork, &spec.artifact, artifact_spec)?;
        }
        // Start the built-in service instance.
        self.start_service(fork, spec, constructor)
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

    /// Start and configure a new service instance. After that, write the information about the
    /// service instance to the dispatcher's information schema.
    ///
    /// # Panics
    ///
    /// * If instance spec contains invalid service name.
    pub(crate) fn start_service(
        &mut self,
        fork: &Fork,
        spec: InstanceSpec,
        constructor: impl BinaryValue,
    ) -> Result<(), ExecutionError> {
        debug_assert!(spec.validate().is_ok(), "{:?}", spec.validate());

        // Check that service doesn't use existing identifiers.
        if self.runtime_lookup.contains_key(&spec.id) {
            return Err(Error::ServiceIdExists.into());
        }
        // Try to start and initialize the service instance.
        self.runtimes
            .get_mut(&spec.artifact.runtime_id)
            .ok_or(Error::IncorrectRuntime)
            .map_err(ExecutionError::from)
            .and_then(|runtime| runtime.start_service(&spec))?;
        // Try to initialize the started instance of the service, otherwise stop.
        self.initialize_service(
            self.runtimes[&spec.artifact.runtime_id].as_ref(),
            fork,
            spec.as_descriptor(),
            constructor.into_bytes(),
        )
        .map_err(|e| {
            error!(
                "An error occurred while configuring the service {}: {}",
                spec.name, e
            );
            // TODO Find the way to avoid panic from the untrusted code. [ECR-3222]
            let runtime = self
                .runtimes
                .get_mut(&spec.artifact.runtime_id)
                .unwrap_or_else(|| {
                    panic!(FatalError::new(
                        "Unable to find runtime to rollback a broken service instance."
                    ))
                });
            if let Err(e) = runtime.stop_service(spec.as_descriptor()) {
                panic!(FatalError::new(e.to_string()))
            }
            e
        })?;
        // Add service instance to the dispatcher schema.
        self.register_running_service(&spec);
        Schema::new(fork).add_service_instance(spec)?;
        Ok(())
    }

    // TODO documentation [ECR-3275]
    pub(crate) fn execute(
        &mut self,
        fork: &mut Fork,
        tx_id: Hash,
        tx: &Verified<AnyTx>,
    ) -> Result<(), ExecutionError> {
        let dispatcher_ref = DispatcherRef::new(self);
        let context = ExecutionContext::new(
            &dispatcher_ref,
            fork,
            Caller::Transaction {
                author: tx.author(),
                hash: tx_id,
            },
        );
        self.call(&context, &tx.as_ref().call_info, &tx.as_ref().arguments)?;
        // Execute pending dispatcher actions.
        for action in context.dispatcher.take_actions() {
            action.execute(self, fork)?;
        }
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

    pub(crate) fn before_commit(&mut self, fork: &mut Fork) {
        let dispatcher_ref = DispatcherRef::new(self);
        for runtime in self.runtimes.values() {
            runtime.before_commit(&dispatcher_ref, fork);
        }
        // Execute pending dispatcher actions.
        for action in dispatcher_ref.take_actions() {
            let _ = action.execute(self, fork).map_err(|e| {
                error!(
                    "An error occurred while performing the dispatcher action. {}",
                    e
                )
            });
        }
    }

    pub(crate) fn after_commit(
        &mut self,
        snapshot: impl AsRef<dyn Snapshot>,
        service_keypair: &(PublicKey, SecretKey),
        tx_sender: &ApiSender,
    ) {
        let channel = DispatcherSender::new();
        self.runtimes.values().for_each(|runtime| {
            runtime.after_commit(&channel, snapshot.as_ref(), &service_keypair, &tx_sender)
        });

        for request in channel.take_deploy_requests() {
            match self
                .deploy_artifact(request.artifact.clone(), request.spec.clone())
                .wait()
            {
                Ok(_) => request.completed(),
                Err(e) => warn!(
                    "An error during deploy artifact {:?} occurred {:?}",
                    request.artifact, e
                ),
            }
        }
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

    /// Start a new service instance.
    fn restart_service(&mut self, instance: &InstanceSpec) -> Result<(), ExecutionError> {
        let runtime = self
            .runtimes
            .get_mut(&instance.artifact.runtime_id)
            .ok_or(Error::IncorrectRuntime)?;
        runtime.start_service(instance)?;
        self.register_running_service(&instance);
        Ok(())
    }

    // Try to configure the started instance of the service and catch panic if it occurs.
    fn initialize_service(
        &self,
        runtime: &dyn Runtime,
        fork: &Fork,
        descriptor: InstanceDescriptor,
        constructor: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        let constructor_is_empty = constructor.is_empty();

        let dispatcher_ref = DispatcherRef::new(self);
        let context = ExecutionContext {
            interface_name: <dyn Initialize<Params = ()> as Interface>::INTERFACE_NAME,
            ..ExecutionContext::new(&dispatcher_ref, fork, Caller::Blockchain {})
        };
        let call_info = CallInfo {
            instance_id: descriptor.id,
            method_id: INITIALIZE_METHOD_ID,
        };
        let args = constructor.into_bytes();

        catch_panic(|| {
            runtime
                .execute(&context, &call_info, args.as_ref())
                .or_else(|err| {
                    // Default behavior for case if the service does not implement an `Initialize`
                    // interface and constructor is empty.
                    if constructor_is_empty && err.kind == Error::NoSuchInterface.into() {
                        Ok(())
                    } else {
                        Err(err)
                    }
                })
        })
    }

    /// Perform a configuration update with the specified changes.
    fn update_config(
        &self,
        fork: &Fork,
        caller_instance_id: InstanceId,
        changes: Vec<ConfigChange>,
    ) -> Result<(), ExecutionError> {
        changes.into_iter().try_for_each(|change| match change {
            ConfigChange::Consensus(config) => {
                trace!("Updating consensus configuration {:?}", config);

                blockchain::Schema::new(fork)
                    .consensus_config_entry()
                    .set(config);
                Ok(())
            }

            ConfigChange::Service(config) => {
                trace!(
                    "Updating service instance configuration, instance ID is {}",
                    config.instance_id
                );

                let dispatcher_ref = DispatcherRef::new(self);

                let context = CallContext::new(
                    fork,
                    &dispatcher_ref,
                    caller_instance_id,
                    config.instance_id,
                );
                ConfigureCall::from(context).apply_config(config.params)
            }
        })
    }
}

#[derive(Debug)]
pub(crate) enum Action {
    /// Register the deployed artifact in the dispatcher.
    /// Make sure that you successfully complete the deploy artifact procedure.
    RegisterArtifact { artifact: ArtifactId, spec: Vec<u8> },
    /// Start the service instance with the specified params.
    /// Make sure that the artifact is deployed.
    StartService {
        artifact: ArtifactId,
        instance_name: String,
        config: Vec<u8>,
    },
    /// Perform a configuration update with the specified changes.
    /// Make sure that no errors occur when applying these changes.
    UpdateConfig {
        caller_instance_id: InstanceId,
        changes: Vec<ConfigChange>,
    },
}

impl Action {
    fn execute(self, dispatcher: &mut Dispatcher, fork: &Fork) -> Result<(), ExecutionError> {
        // TODO Take care about the graceful panics handling during the actions execution. [ECR-3222]
        catch_panic(|| match self {
            Action::RegisterArtifact { artifact, spec } => dispatcher
                .register_artifact(fork, &artifact, spec)
                .map_err(From::from),

            Action::StartService {
                artifact,
                instance_name,
                config,
            } => dispatcher
                .start_service(
                    fork,
                    InstanceSpec {
                        artifact,
                        name: instance_name,
                        id: Schema::new(fork).assign_instance_id(),
                    },
                    config,
                )
                .map_err(From::from),

            Action::UpdateConfig {
                caller_instance_id,
                changes,
            } => dispatcher.update_config(fork, caller_instance_id, changes),
        })
    }
}

struct DeployArtifactRequest {
    artifact: ArtifactId,
    spec: Vec<u8>,
    /// The operation to be performed if this request was successfully processed.
    and_then: Box<dyn FnOnce() + 'static>,
}

impl DeployArtifactRequest {
    /// Invoke request callback.
    fn completed(self) {
        (self.and_then)();
    }
}

impl std::fmt::Debug for DeployArtifactRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("DeployArtifactRequest")
            .field(&self.artifact)
            .finish()
    }
}

/// Reference to the underlying runtime dispatcher.
#[derive(Debug)]
pub struct DispatcherRef<'a> {
    /// List of dispatcher actions that will be performed after execution finishes.
    actions: RefCell<Vec<Action>>,
    /// Reference to the underlying runtime dispatcher.
    inner: &'a Dispatcher,
}

impl<'a> DispatcherRef<'a> {
    /// Create a new instance.
    pub(crate) fn new(dispatcher: &'a Dispatcher) -> Self {
        Self {
            inner: dispatcher,
            actions: RefCell::default(),
        }
    }

    /// Call the corresponding runtime method.
    pub(crate) fn call(
        &self,
        context: &ExecutionContext,
        call_info: &CallInfo,
        arguments: &[u8],
    ) -> Result<(), ExecutionError> {
        self.inner.call(context, call_info, arguments)
    }

    pub(crate) fn dispatch_action(&self, action: Action) {
        self.actions.borrow_mut().push(action);
    }

    pub(crate) fn take_actions(&self) -> Vec<Action> {
        self.actions.borrow_mut().drain(..).collect()
    }
}

// TODO Implement proper pending deploy logic [ECR-3291]

/// Channel to communicate with the dispatcher.
#[derive(Debug)]
pub struct DispatcherSender {
    deploy_request: RefCell<Vec<DeployArtifactRequest>>,
}

impl DispatcherSender {
    /// Create a new instance.
    fn new() -> Self {
        Self {
            deploy_request: RefCell::default(),
        }
    }

    /// Request an artifact deployment and invoke the callback if the deployment
    /// was successfully completed.
    pub(super) fn request_deploy_artifact<F>(
        &self,
        artifact: ArtifactId,
        spec: Vec<u8>,
        and_then: F,
    ) where
        F: FnOnce() + 'static,
    {
        self.deploy_request
            .borrow_mut()
            .push(DeployArtifactRequest {
                artifact,
                spec,
                and_then: Box::new(and_then),
            })
    }

    /// Take requests from this channel.
    fn take_deploy_requests(self) -> Vec<DeployArtifactRequest> {
        self.deploy_request.into_inner()
    }
}

#[cfg(test)]
mod tests {
    use exonum_merkledb::{Database, TemporaryDB};
    use futures::{sync::mpsc, IntoFuture};

    use std::sync::{
        mpsc::{channel, Sender},
        Arc,
    };

    use crate::{
        crypto::{self, PublicKey},
        node::ApiSender,
        runtime::{
            dispatcher::Error as DispatcherError,
            rust::{Error as RustRuntimeError, RustRuntime},
            ApiChange, ArtifactProtobufSpec, InstanceId, MethodId, RuntimeIdentifier,
            StateHashAggregator,
        },
    };

    use super::*;

    enum SampleRuntimes {
        First = 5,
        Second = 6,
    }

    #[derive(Debug)]
    pub struct DispatcherBuilder {
        dispatcher: Dispatcher,
    }

    impl DispatcherBuilder {
        fn new() -> Self {
            Self {
                dispatcher: Dispatcher::default(),
            }
        }

        fn with_runtime(mut self, id: u32, runtime: impl Into<Box<dyn Runtime>>) -> Self {
            self.dispatcher.runtimes.insert(id, runtime.into());
            self
        }

        fn finalize(self) -> Dispatcher {
            self.dispatcher
        }
    }

    impl Default for DispatcherBuilder {
        fn default() -> Self {
            Self::new()
        }
    }

    #[derive(Debug, Clone)]
    struct SampleRuntime {
        runtime_type: u32,
        instance_id: InstanceId,
        method_id: MethodId,
        api_changes_sender: Sender<(u32, Vec<ApiChange>)>,
    }

    #[derive(Debug, IntoExecutionError)]
    #[exonum(crate = "crate")]
    enum SampleError {
        Foo = 15,
    }

    impl SampleRuntime {
        fn new(
            runtime_type: u32,
            instance_id: InstanceId,
            method_id: MethodId,
            api_changes_sender: Sender<(u32, Vec<ApiChange>)>,
        ) -> Self {
            Self {
                runtime_type,
                instance_id,
                method_id,
                api_changes_sender,
            }
        }
    }

    impl Runtime for SampleRuntime {
        fn deploy_artifact(
            &mut self,
            artifact: ArtifactId,
            _spec: Vec<u8>,
        ) -> Box<dyn Future<Item = (), Error = ExecutionError>> {
            Box::new(
                if artifact.runtime_id == self.runtime_type {
                    Ok(())
                } else {
                    Err(Error::IncorrectRuntime.into())
                }
                .into_future(),
            )
        }

        fn is_artifact_deployed(&self, id: &ArtifactId) -> bool {
            id.runtime_id == self.runtime_type
        }

        fn start_service(&mut self, spec: &InstanceSpec) -> Result<(), ExecutionError> {
            if spec.artifact.runtime_id == self.runtime_type {
                Ok(())
            } else {
                Err(Error::IncorrectRuntime.into())
            }
        }

        fn stop_service(&mut self, _instance: InstanceDescriptor) -> Result<(), ExecutionError> {
            Ok(())
        }

        fn execute(
            &self,
            context: &ExecutionContext,
            call_info: &CallInfo,
            _: &[u8],
        ) -> Result<(), ExecutionError> {
            // TODO Provide interface calls support for this runtime [ECR-3222]
            if !context.interface_name.is_empty() {
                return Err(DispatcherError::NoSuchInterface.into());
            }

            if call_info.instance_id == self.instance_id && call_info.method_id == self.method_id {
                Ok(())
            } else {
                Err(SampleError::Foo.into())
            }
        }

        fn state_hashes(&self, _snapshot: &dyn Snapshot) -> StateHashAggregator {
            StateHashAggregator::default()
        }

        fn before_commit(&self, _dispatcher: &DispatcherRef, _fork: &mut Fork) {}

        fn after_commit(
            &self,
            _dispatcher: &DispatcherSender,
            _snapshot: &dyn Snapshot,
            _service_keypair: &(PublicKey, SecretKey),
            _tx_sender: &ApiSender,
        ) {
        }

        fn artifact_protobuf_spec(&self, _id: &ArtifactId) -> Option<ArtifactProtobufSpec> {
            Some(ArtifactProtobufSpec::default())
        }

        fn notify_api_changes(&self, _context: &ApiContext, changes: &[ApiChange]) {
            let changes = (self.runtime_type, changes.to_vec());
            self.api_changes_sender.send(changes).unwrap();
        }
    }

    #[test]
    fn test_builder() {
        let runtime_a = SampleRuntime::new(SampleRuntimes::First as u32, 0, 0, channel().0);
        let runtime_b = SampleRuntime::new(SampleRuntimes::Second as u32, 1, 0, channel().0);

        let dispatcher = DispatcherBuilder::new()
            .with_runtime(runtime_a.runtime_type, runtime_a)
            .with_runtime(runtime_b.runtime_type, runtime_b)
            .finalize();

        assert!(dispatcher
            .runtimes
            .get(&(SampleRuntimes::First as u32))
            .is_some());
        assert!(dispatcher
            .runtimes
            .get(&(SampleRuntimes::Second as u32))
            .is_some());
    }

    #[test]
    fn test_dispatcher_simple() {
        const RUST_SERVICE_ID: InstanceId = 2;
        const JAVA_SERVICE_ID: InstanceId = 3;
        const RUST_SERVICE_NAME: &str = "rust-service";
        const JAVA_SERVICE_NAME: &str = "java-service";
        const RUST_METHOD_ID: MethodId = 0;
        const JAVA_METHOD_ID: MethodId = 1;

        // Create dispatcher and test data.
        let db = Arc::new(TemporaryDB::new());

        let (changes_tx, changes_rx) = channel();
        let runtime_a = SampleRuntime::new(
            SampleRuntimes::First as u32,
            RUST_SERVICE_ID,
            RUST_METHOD_ID,
            changes_tx.clone(),
        );
        let runtime_b = SampleRuntime::new(
            SampleRuntimes::Second as u32,
            JAVA_SERVICE_ID,
            JAVA_METHOD_ID,
            changes_tx,
        );

        let mut dispatcher = DispatcherBuilder::new()
            .with_runtime(runtime_a.runtime_type, runtime_a.clone())
            .with_runtime(runtime_b.runtime_type, runtime_b.clone())
            .finalize();

        let sample_rust_spec = ArtifactId {
            runtime_id: SampleRuntimes::First as u32,
            name: "first".into(),
        };
        let sample_java_spec = ArtifactId {
            runtime_id: SampleRuntimes::Second as u32,
            name: "second".into(),
        };

        // Check if the services are ready for deploy.
        let fork = db.fork();
        dispatcher
            .deploy_and_register_artifact(&fork, &sample_rust_spec, Vec::default())
            .unwrap();
        dispatcher
            .deploy_and_register_artifact(&fork, &sample_java_spec, Vec::default())
            .unwrap();

        // Check if the services are ready for initiation.
        dispatcher
            .start_service(
                &fork,
                InstanceSpec {
                    artifact: sample_rust_spec.clone(),
                    id: RUST_SERVICE_ID,
                    name: RUST_SERVICE_NAME.into(),
                },
                Vec::default(),
            )
            .expect("start_service failed for rust");
        dispatcher
            .start_service(
                &fork,
                InstanceSpec {
                    artifact: sample_java_spec.clone(),
                    id: JAVA_SERVICE_ID,
                    name: JAVA_SERVICE_NAME.into(),
                },
                Vec::default(),
            )
            .expect("start_service failed for java");

        // Check if transactions are ready for execution.
        let tx_payload = [0x00_u8; 1];

        let dispatcher_ref = DispatcherRef::new(&dispatcher);
        let context =
            ExecutionContext::new(&dispatcher_ref, &fork, Caller::Service { instance_id: 1 });
        dispatcher
            .call(
                &context,
                &CallInfo::new(RUST_SERVICE_ID, RUST_METHOD_ID),
                &tx_payload,
            )
            .expect("Correct tx rust");

        dispatcher
            .call(
                &context,
                &CallInfo::new(RUST_SERVICE_ID, JAVA_METHOD_ID),
                &tx_payload,
            )
            .expect_err("Incorrect tx rust");

        dispatcher
            .call(
                &context,
                &CallInfo::new(JAVA_SERVICE_ID, JAVA_METHOD_ID),
                &tx_payload,
            )
            .expect("Correct tx java");

        dispatcher
            .call(
                &context,
                &CallInfo::new(JAVA_SERVICE_ID, RUST_METHOD_ID),
                &tx_payload,
            )
            .expect_err("Incorrect tx java");

        // Check that API changes in the dispatcher contain the started services.
        let context = ApiContext::new(
            db.clone(),
            crypto::gen_keypair(),
            ApiSender::new(mpsc::channel(0).0),
        );
        assert!(dispatcher.notify_api_changes(&context));
        let expected_api_changes = vec![
            (
                SampleRuntimes::First as u32,
                vec![ApiChange::InstanceAdded(RUST_SERVICE_ID)],
            ),
            (
                SampleRuntimes::Second as u32,
                vec![ApiChange::InstanceAdded(JAVA_SERVICE_ID)],
            ),
        ];
        assert_eq!(
            expected_api_changes,
            changes_rx.iter().take(2).collect::<Vec<_>>()
        );
        // Check that API changes are empty after the `notify_api_changes`.
        assert!(dispatcher.api_changes.is_empty());

        // Check that API changes in the dispatcher contain the started services after restart.
        db.merge(fork.into_patch()).unwrap();
        let mut dispatcher = DispatcherBuilder::new()
            .with_runtime(runtime_a.runtime_type, runtime_a)
            .with_runtime(runtime_b.runtime_type, runtime_b)
            .finalize();
        dispatcher.restore_state(&db.snapshot()).unwrap();
        dispatcher.notify_api_changes(&ApiContext::new(
            db.clone(),
            crypto::gen_keypair(),
            ApiSender::new(mpsc::channel(0).0),
        ));

        assert_eq!(
            expected_api_changes,
            changes_rx.iter().take(2).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_dispatcher_rust_runtime_no_service() {
        const RUST_SERVICE_ID: InstanceId = 2;
        const RUST_SERVICE_NAME: &str = "rust-service";
        const RUST_METHOD_ID: MethodId = 0;

        // Create dispatcher and test data.
        let db = TemporaryDB::new();

        let mut dispatcher = DispatcherBuilder::default()
            .with_runtime(RuntimeIdentifier::Rust as u32, RustRuntime::default())
            .finalize();

        let sample_rust_spec =
            ArtifactId::new(RuntimeIdentifier::Rust as u32, "foo:1.0.0").unwrap();

        // Check deploy.
        assert_eq!(
            dispatcher
                .deploy_artifact(sample_rust_spec.clone(), Vec::default())
                .wait()
                .expect_err("deploy artifact succeed"),
            RustRuntimeError::UnableToDeploy.into()
        );

        // Check if the services are ready to start.
        let fork = db.fork();

        assert_eq!(
            dispatcher
                .start_service(
                    &fork,
                    InstanceSpec {
                        artifact: sample_rust_spec.clone(),
                        id: RUST_SERVICE_ID,
                        name: RUST_SERVICE_NAME.into()
                    },
                    Vec::default()
                )
                .expect_err("start service succeed"),
            Error::ArtifactNotDeployed.into()
        );

        // Check if transactions are ready for execution.
        let tx_payload = [0x00_u8; 1];

        let dispatcher_ref = DispatcherRef::new(&dispatcher);
        let context =
            ExecutionContext::new(&dispatcher_ref, &fork, Caller::Service { instance_id: 15 });
        dispatcher
            .call(
                &context,
                &CallInfo::new(RUST_SERVICE_ID, RUST_METHOD_ID),
                &tx_payload,
            )
            .expect_err("execute succeed");
    }
}
