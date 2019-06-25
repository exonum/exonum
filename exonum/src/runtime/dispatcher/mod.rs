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

pub use schema::Schema;

use exonum_merkledb::{Fork, IndexAccess, Snapshot};
use futures::{future::Future, sink::Sink, sync::mpsc};

use std::collections::HashMap;

use crate::{
    api::ServiceApiBuilder,
    blockchain::CORE_ID,
    events::InternalRequest,
    messages::AnyTx,
    node::ApiSender,
    {
        crypto::{Hash, PublicKey, SecretKey},
        messages::CallInfo,
    },
};

use super::{
    error::{DeployError, ExecutionError, StartError, WRONG_RUNTIME},
    ArtifactId, DeployStatus, InstanceSpec, Runtime, RuntimeContext, ServiceConfig,
    ServiceInstanceId,
};

mod schema;

pub struct Dispatcher {
    runtimes: HashMap<u32, Box<dyn Runtime>>,
    runtime_lookup: HashMap<ServiceInstanceId, u32>,
    inner_requests_tx: mpsc::Sender<InternalRequest>,
}

impl std::fmt::Debug for Dispatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("Dispatcher")
            .field("runtimes", &self.runtimes)
            .finish()
    }
}

impl Dispatcher {
    /// Creates a new dispatcher with the specified runtimes.
    pub fn with_runtimes(
        runtimes: impl IntoIterator<Item = (u32, Box<dyn Runtime>)>,
        inner_requests_tx: mpsc::Sender<InternalRequest>,
    ) -> Self {
        Self {
            runtimes: runtimes.into_iter().collect(),
            runtime_lookup: Default::default(),
            inner_requests_tx,
        }
    }

    /// Restores dispatcher from the state which saved in the specified snapshot.
    ///
    /// # Panics
    /// TODO [ECR-3275]
    pub fn restore_state(&mut self, snapshot: impl IndexAccess) {
        let schema = Schema::new(snapshot);
        // Restores information about the deployed services.
        for (name, runtime_id) in &schema.deployed_artifacts() {
            let artifact = ArtifactId { name, runtime_id };
            let status = self
                .deploy(&artifact)
                .expect("Unable to restore deployed artifacts");
            assert!(
                status.is_deployed(),
                "During the restart artifacts must be deployed instantly."
            );
        }
        // Restarts active service instances.
        for spec in schema.started_services().values() {
            self.restart_service(&spec)
                .expect("Unable to restart services");
        }
    }

    /// Adds built-in service with predefined identifier.
    // TODO rewrite to return error [ECR-3222]
    pub(crate) fn add_builtin_service(
        &mut self,
        fork: &Fork,
        spec: InstanceSpec,
        constructor: ServiceConfig,
    ) {
        // Registers service instance in runtime.
        self.deploy(&spec.artifact)
            .and_then(|status| {
                assert_eq!(
                    status,
                    DeployStatus::Deployed,
                    "Builtin artifacts must be deployed instantly."
                );
                Ok(())
            })
            .expect("Unable to deploy builtin service");
        self.register_artifact(fork, spec.artifact.clone())
            .expect("Unable to register builtin artifact");
        // Starts builtin service instance.
        self.start_service(
            &mut RuntimeContext::without_author(fork),
            spec,
            &constructor,
        )
        .expect("Unable to start builtin service instance");
    }

    pub fn check_deploy_status(
        &self,
        artifact: &ArtifactId,
        cancel_if_not_complete: bool,
    ) -> Result<DeployStatus, DeployError> {
        self.runtimes
            .get(&artifact.runtime_id)
            .ok_or(DeployError::WrongRuntime)
            .and_then(|runtime| runtime.check_deploy_status(artifact, cancel_if_not_complete))
    }

    pub fn state_hashes(&self, snapshot: &dyn Snapshot) -> Vec<(ServiceInstanceId, Vec<Hash>)> {
        self.runtimes
            .iter()
            .map(|(_, runtime)| runtime.state_hashes(snapshot))
            .flatten()
            .collect::<Vec<_>>()
    }

    pub(crate) fn services_api(&self) -> Vec<(String, ServiceApiBuilder)> {
        self.runtimes
            .iter()
            .fold(Vec::new(), |mut api, (_, runtime)| {
                api.append(&mut runtime.services_api());
                api
            })
    }

    /// Sends restart API message.
    fn restart_api(&self) {
        let _ = self
            .inner_requests_tx
            .clone()
            .send(InternalRequest::RestartApi)
            .wait()
            .map_err(|e| error!("Failed to request API restart: {}", e));
    }

    // TODO Implement proper pending deploy logic [ECR-3291]
    pub(crate) fn deploy(&mut self, artifact: &ArtifactId) -> Result<DeployStatus, DeployError> {
        self.runtimes
            .get_mut(&artifact.runtime_id)
            .ok_or(DeployError::WrongRuntime)
            .and_then(|runtime| runtime.begin_deploy(artifact))
    }

    /// Registers deployed artifact in the dispatcher's information schema.
    pub(crate) fn register_artifact(
        &self,
        fork: &Fork,
        artifact: ArtifactId,
    ) -> Result<(), DeployError> {
        Schema::new(fork).add_deployed_artifact(artifact)
    }

    /// Registers service instance in the runtime lookup table.
    fn register_running_service(&mut self, spec: &InstanceSpec) {
        self.runtime_lookup
            .insert(spec.id, spec.artifact.runtime_id);
    }

    /// Just starts a new service instance.
    fn restart_service(&mut self, spec: &InstanceSpec) -> Result<(), StartError> {
        self.runtimes
            .get_mut(&spec.artifact.runtime_id)
            .ok_or(StartError::WrongRuntime)
            .and_then(|runtime| runtime.start_service(spec))
    }

    fn identifier_exists(&self, id: ServiceInstanceId) -> bool {
        id == u32::from(CORE_ID) || self.runtime_lookup.contains_key(&id)
    }

    /// Starts and configures a new service instance. After that it writes information about
    /// service instance to the dispatcher's information schema.
    pub(crate) fn start_service(
        &mut self,
        context: &mut RuntimeContext,
        spec: InstanceSpec,
        constructor: &ServiceConfig,
    ) -> Result<(), StartError> {
        // Check that service doesn't use existing identifiers.
        if self.identifier_exists(spec.id) {
            return Err(StartError::ServiceIdExists);
        }
        // Tries to start and configure service instance.
        self.runtimes
            .get_mut(&spec.artifact.runtime_id)
            .ok_or(StartError::WrongRuntime)
            .and_then(|runtime| {
                runtime.start_service(&spec)?;
                // Tries to configure a started instance of the service, otherwise it stops.
                runtime
                    .configure_service(context.fork, &spec, constructor)
                    .or_else(|_| runtime.stop_service(&spec))?; // TODO Should we emit panic if revert failed? [ECR-3222]
                Ok(())
            })?;
        self.register_running_service(&spec);
        // Adds service instance to the dispatcher schema.
        Schema::new(context.fork).add_started_service(spec)?;
        Ok(())
    }

    /// Executes transaction. // TODO documentation [ECR-3275]
    pub(crate) fn execute(
        &mut self,
        context: &mut RuntimeContext,
        transaction: &AnyTx,
    ) -> Result<(), ExecutionError> {
        self.call(context, transaction.call_info, &transaction.payload)?;
        // Executes pending dispatcher actions.
        context
            .take_dispatcher_actions()
            .into_iter()
            .try_for_each(|action| action.execute(self, context))
    }

    /// Calls the corresponding runtime method.
    pub(crate) fn call(
        &self,
        context: &mut RuntimeContext,
        call_info: CallInfo,
        payload: &[u8],
    ) -> Result<(), ExecutionError> {
        let runtime_id = self.runtime_lookup.get(&call_info.instance_id);

        if runtime_id.is_none() {
            return Err(ExecutionError::with_description(
                WRONG_RUNTIME,
                "Wrong runtime",
            ));
        }

        if let Some(runtime) = self.runtimes.get(&runtime_id.unwrap()) {
            runtime.execute(self, context, call_info, payload)?;
            Ok(())
        } else {
            Err(ExecutionError::with_description(
                WRONG_RUNTIME,
                "Wrong runtime",
            ))
        }
    }

    pub(crate) fn before_commit(&self, fork: &mut Fork) {
        for runtime in self.runtimes.values() {
            runtime.before_commit(self, fork);
        }
    }

    pub(crate) fn after_commit(
        &self,
        snapshot: Box<dyn Snapshot>,
        service_keypair: &(PublicKey, SecretKey),
        tx_sender: &ApiSender,
    ) {
        self.runtimes.values().for_each(|runtime| {
            runtime.after_commit(self, snapshot.as_ref(), &service_keypair, &tx_sender)
        });
    }
}

// TODO Update action names in according with changes in runtime. [ECR-3222]
#[derive(Debug)]
pub enum Action {
    BeginDeploy {
        artifact: ArtifactId,
    },
    StartService {
        spec: InstanceSpec,
        constructor: ServiceConfig,
    },
}

impl Action {
    fn execute(
        self,
        dispatcher: &mut Dispatcher,
        context: &mut RuntimeContext,
    ) -> Result<(), ExecutionError> {
        match self {
            Action::BeginDeploy { artifact } => {
                let status = dispatcher.deploy(&artifact)?;
                // TODO Implement proper pending deploy logic [ECR-3291]
                if status.is_deployed() {
                    dispatcher.register_artifact(context.fork, artifact)?;
                }
                Ok(())
            }

            Action::StartService { spec, constructor } => {
                dispatcher.start_service(context, spec, &constructor)?;
                dispatcher.restart_api();
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use exonum_merkledb::{Database, TemporaryDB};

    use crate::{
        crypto::{Hash, PublicKey},
        messages::{MethodId, ServiceInstanceId},
        runtime::{rust::RustRuntime, RuntimeIdentifier},
    };

    use super::*;

    enum SampleRuntimes {
        First = 2,
        Second = 3,
    }

    #[derive(Debug)]
    pub struct DispatcherBuilder {
        dispatcher: Dispatcher,
    }

    impl DispatcherBuilder {
        fn new() -> Self {
            Self {
                dispatcher: Dispatcher::with_runtimes(Vec::new(), mpsc::channel(0).0),
            }
        }

        /// Adds additional runtime.
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

    #[derive(Debug)]
    struct SampleRuntime {
        runtime_type: u32,
        instance_id: ServiceInstanceId,
        method_id: MethodId,
    }

    impl SampleRuntime {
        fn new(runtime_type: u32, instance_id: ServiceInstanceId, method_id: MethodId) -> Self {
            Self {
                runtime_type,
                instance_id,
                method_id,
            }
        }
    }

    impl Runtime for SampleRuntime {
        fn begin_deploy(&mut self, artifact: &ArtifactId) -> Result<DeployStatus, DeployError> {
            if artifact.runtime_id == self.runtime_type {
                Ok(DeployStatus::Deployed)
            } else {
                Err(DeployError::WrongRuntime)
            }
        }

        fn check_deploy_status(
            &self,
            artifact: &ArtifactId,
            _: bool,
        ) -> Result<DeployStatus, DeployError> {
            if artifact.runtime_id == self.runtime_type {
                Ok(DeployStatus::Deployed)
            } else {
                Err(DeployError::WrongRuntime)
            }
        }

        fn start_service(&mut self, spec: &InstanceSpec) -> Result<(), StartError> {
            if spec.artifact.runtime_id == self.runtime_type {
                Ok(())
            } else {
                Err(StartError::WrongRuntime)
            }
        }

        fn stop_service(&mut self, spec: &InstanceSpec) -> Result<(), StartError> {
            if spec.artifact.runtime_id == self.runtime_type {
                Ok(())
            } else {
                Err(StartError::WrongRuntime)
            }
        }

        fn configure_service(
            &self,
            _fork: &Fork,
            spec: &InstanceSpec,
            _parameters: &ServiceConfig,
        ) -> Result<(), StartError> {
            if spec.artifact.runtime_id == self.runtime_type {
                Ok(())
            } else {
                Err(StartError::WrongRuntime)
            }
        }

        fn execute(
            &self,
            _: &Dispatcher,
            _: &mut RuntimeContext,
            call_info: CallInfo,
            _: &[u8],
        ) -> Result<(), ExecutionError> {
            if call_info.instance_id == self.instance_id && call_info.method_id == self.method_id {
                Ok(())
            } else {
                Err(ExecutionError::new(0xFF_u8))
            }
        }

        fn state_hashes(&self, _snapshot: &dyn Snapshot) -> Vec<(ServiceInstanceId, Vec<Hash>)> {
            vec![]
        }

        fn before_commit(&self, _dispatcher: &Dispatcher, _fork: &mut Fork) {}

        fn after_commit(
            &self,
            _dispatcher: &Dispatcher,
            _snapshot: &dyn Snapshot,
            _service_keypair: &(PublicKey, SecretKey),
            _tx_sender: &ApiSender,
        ) {
        }
    }

    #[test]
    fn test_builder() {
        let runtime_a = SampleRuntime::new(SampleRuntimes::First as u32, 0, 0);
        let runtime_b = SampleRuntime::new(SampleRuntimes::Second as u32, 1, 0);

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
        const RUST_SERVICE_ID: ServiceInstanceId = 2;
        const JAVA_SERVICE_ID: ServiceInstanceId = 3;
        const RUST_SERVICE_NAME: &str = "rust-service";
        const JAVA_SERVICE_NAME: &str = "java-service";
        const RUST_METHOD_ID: MethodId = 0;
        const JAVA_METHOD_ID: MethodId = 1;

        // Create dispatcher and test data.
        let db = TemporaryDB::new();

        let runtime_a = SampleRuntime::new(
            SampleRuntimes::First as u32,
            RUST_SERVICE_ID,
            RUST_METHOD_ID,
        );
        let runtime_b = SampleRuntime::new(
            SampleRuntimes::Second as u32,
            JAVA_SERVICE_ID,
            JAVA_METHOD_ID,
        );

        let mut dispatcher = DispatcherBuilder::new()
            .with_runtime(runtime_a.runtime_type, runtime_a)
            .with_runtime(runtime_b.runtime_type, runtime_b)
            .finalize();

        let sample_rust_spec = ArtifactId {
            runtime_id: SampleRuntimes::First as u32,
            name: "first".into(),
        };
        let sample_java_spec = ArtifactId {
            runtime_id: SampleRuntimes::Second as u32,
            name: "second".into(),
        };

        // Check deploy.
        dispatcher
            .deploy(&sample_rust_spec)
            .expect("start_deploy failed for rust");
        dispatcher
            .deploy(&sample_java_spec)
            .expect("start_deploy failed for java");

        // Check deploy status
        assert_eq!(
            dispatcher
                .check_deploy_status(&sample_rust_spec, false)
                .unwrap(),
            DeployStatus::Deployed
        );
        assert_eq!(
            dispatcher
                .check_deploy_status(&sample_java_spec, false)
                .unwrap(),
            DeployStatus::Deployed
        );

        let fork = db.fork();
        // Register artifacts
        dispatcher
            .register_artifact(&fork, sample_rust_spec.clone())
            .unwrap();
        dispatcher
            .register_artifact(&fork, sample_java_spec.clone())
            .unwrap();
        // Check if we can init services.
        let mut context = RuntimeContext::new(&fork, PublicKey::zero(), Hash::zero());

        dispatcher
            .start_service(
                &mut context,
                InstanceSpec {
                    artifact: sample_rust_spec.clone(),
                    id: RUST_SERVICE_ID,
                    name: RUST_SERVICE_NAME.into(),
                },
                &ServiceConfig::default(),
            )
            .expect("start_service failed for rust");

        dispatcher
            .start_service(
                &mut context,
                InstanceSpec {
                    artifact: sample_java_spec.clone(),
                    id: JAVA_SERVICE_ID,
                    name: JAVA_SERVICE_NAME.into(),
                },
                &ServiceConfig::default(),
            )
            .expect("start_service failed for java");

        // Check if we can execute transactions.
        let tx_payload = [0x00_u8; 1];

        dispatcher
            .call(
                &mut context,
                CallInfo::new(RUST_SERVICE_ID, RUST_METHOD_ID),
                &tx_payload,
            )
            .expect("Correct tx rust");

        dispatcher
            .call(
                &mut context,
                CallInfo::new(RUST_SERVICE_ID, JAVA_METHOD_ID),
                &tx_payload,
            )
            .expect_err("Incorrect tx rust");

        dispatcher
            .call(
                &mut context,
                CallInfo::new(JAVA_SERVICE_ID, JAVA_METHOD_ID),
                &tx_payload,
            )
            .expect("Correct tx java");

        dispatcher
            .call(
                &mut context,
                CallInfo::new(JAVA_SERVICE_ID, RUST_METHOD_ID),
                &tx_payload,
            )
            .expect_err("Incorrect tx java");
    }

    #[test]
    fn test_dispatcher_rust_runtime_no_service() {
        const RUST_SERVICE_ID: ServiceInstanceId = 2;
        const RUST_SERVICE_NAME: &str = "rust-service";
        const RUST_METHOD_ID: MethodId = 0;

        // Create dispatcher and test data.
        let db = TemporaryDB::new();

        let mut dispatcher = DispatcherBuilder::default()
            .with_runtime(RuntimeIdentifier::Rust as u32, RustRuntime::default())
            .finalize();

        let sample_rust_spec = ArtifactId::new(RuntimeIdentifier::Rust as u32, "foo");

        // Check deploy.
        assert_eq!(
            dispatcher
                .deploy(&sample_rust_spec)
                .expect_err("start_deploy succeed"),
            DeployError::WrongArtifact
        );

        assert_eq!(
            dispatcher
                .check_deploy_status(&sample_rust_spec, false)
                .expect_err("check_deploy_status succeed"),
            DeployError::WrongArtifact
        );

        // Checks if we can start services.
        let fork = db.fork();
        let mut context = RuntimeContext::new(&fork, PublicKey::zero(), Hash::zero());

        assert_eq!(
            dispatcher
                .start_service(
                    &mut context,
                    InstanceSpec {
                        artifact: sample_rust_spec.clone(),
                        id: RUST_SERVICE_ID,
                        name: RUST_SERVICE_NAME.into()
                    },
                    &ServiceConfig::default()
                )
                .expect_err("init_service succeed"),
            StartError::WrongArtifact
        );

        // Check if we can execute transactions.
        let tx_payload = [0x00_u8; 1];

        dispatcher
            .call(
                &mut context,
                CallInfo::new(RUST_SERVICE_ID, RUST_METHOD_ID),
                &tx_payload,
            )
            .expect_err("execute succeed");
    }
}
