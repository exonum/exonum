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

use exonum_merkledb::{Fork, Snapshot};
use futures::{future::Future, sink::Sink, sync::mpsc};

use std::collections::HashMap;

use crate::{
    api::ServiceApiBuilder,
    events::InternalRequest,
    node::ApiSender,
    {
        crypto::{Hash, PublicKey, SecretKey},
        messages::CallInfo,
    },
};

use super::{
    error::{DeployError, ExecutionError, StartError, WRONG_RUNTIME},
    rust::{service::ServiceFactory, RustRuntime},
    ArtifactSpec, DeployStatus, Runtime, RuntimeContext, ServiceConstructor, ServiceInstanceId,
    ServiceInstanceSpec,
};

pub struct Dispatcher {
    runtimes: HashMap<u32, Box<dyn Runtime>>,
    runtime_lookup: HashMap<ServiceInstanceId, u32>,
    inner_requests_tx: mpsc::Sender<InternalRequest>,
}

impl std::fmt::Debug for Dispatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Dispatcher entity")
    }
}

impl Dispatcher {
    pub fn new(inner_requests_tx: mpsc::Sender<InternalRequest>) -> Self {
        Self::with_runtimes(Default::default(), inner_requests_tx)
    }

    pub fn with_runtimes(
        runtimes: HashMap<u32, Box<dyn Runtime>>,
        inner_requests_tx: mpsc::Sender<InternalRequest>,
    ) -> Self {
        Self {
            runtimes,
            runtime_lookup: Default::default(),
            inner_requests_tx,
        }
    }

    pub fn add_runtime(&mut self, id: u32, runtime: impl Into<Box<dyn Runtime>>) {
        self.runtimes.insert(id, runtime.into());
    }

    /// Sends restart API message.
    pub(crate) fn restart_api(&self) {
        let _ = self
            .inner_requests_tx
            .clone()
            .send(InternalRequest::RestartApi)
            .wait()
            .map_err(|e| error!("Failed to request API restart: {}", e));
    }

    pub(crate) fn notify_service_started(
        &mut self,
        service_id: ServiceInstanceId,
        artifact: ArtifactSpec,
    ) {
        self.runtime_lookup.insert(service_id, artifact.runtime_id);
    }

    pub fn begin_deploy(&mut self, artifact: &ArtifactSpec) -> Result<(), DeployError> {
        self.runtimes
            .get_mut(&artifact.runtime_id)
            .ok_or(DeployError::WrongRuntime)
            .and_then(|runtime| runtime.begin_deploy(artifact))
    }

    pub fn check_deploy_status(
        &self,
        artifact: &ArtifactSpec,
        cancel_if_not_complete: bool,
    ) -> Result<DeployStatus, DeployError> {
        self.runtimes
            .get(&artifact.runtime_id)
            .ok_or(DeployError::WrongRuntime)
            .and_then(|runtime| runtime.check_deploy_status(artifact, cancel_if_not_complete))
    }

    pub fn start_service(
        &mut self,
        context: &mut RuntimeContext,
        spec: ServiceInstanceSpec,
        constructor: &ServiceConstructor,
    ) -> Result<(), StartError> {
        // Tries to start and configure service instance.
        self.runtimes
            .get_mut(&spec.artifact.runtime_id)
            .ok_or(StartError::WrongRuntime)
            .and_then(|runtime| {
                runtime.start_service(&spec)?;
                // Tries to configure a started instance of the service, otherwise it stops.
                runtime
                    .configure_service(context, &spec, constructor)
                    .or_else(|_| runtime.stop_service(&spec))?; // TODO Should we emit panic if revert failed? [ECR-3222]
                Ok(())
            })?;
        self.notify_service_started(spec.id, spec.artifact);
        Ok(())
    }

    pub fn execute(
        &mut self,
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
            runtime.execute(context, call_info, payload)?;
            // Executes pending dispatcher actions.
            context
                .take_dispatcher_actions()
                .into_iter()
                .try_for_each(|action| action.execute(self, context))
        } else {
            Err(ExecutionError::with_description(
                WRONG_RUNTIME,
                "Wrong runtime",
            ))
        }
    }

    pub fn state_hashes(&self, snapshot: &dyn Snapshot) -> Vec<(ServiceInstanceId, Vec<Hash>)> {
        self.runtimes
            .iter()
            .map(|(_, runtime)| runtime.state_hashes(snapshot))
            .flatten()
            .collect::<Vec<_>>()
    }

    pub fn before_commit(&self, fork: &mut Fork) {
        for runtime in self.runtimes.values() {
            runtime.before_commit(fork);
        }
    }

    pub fn after_commit(
        &self,
        snapshot: Box<dyn Snapshot>,
        service_keypair: &(PublicKey, SecretKey),
        tx_sender: &ApiSender,
    ) {
        self.runtimes.values().for_each(|runtime| {
            runtime.after_commit(snapshot.as_ref(), &service_keypair, &tx_sender)
        });
    }

    pub fn services_api(&self) -> Vec<(String, ServiceApiBuilder)> {
        self.runtimes
            .iter()
            .fold(Vec::new(), |mut api, (_, runtime)| {
                api.append(&mut runtime.services_api());
                api
            })
    }
}

#[derive(Debug)]
pub struct DispatcherBuilder {
    builtin_runtime: RustRuntime,
    dispatcher: Dispatcher,
}

#[derive(Debug)]
pub struct BuiltinService {
    pub factory: Box<dyn ServiceFactory>,
    pub instance_id: ServiceInstanceId,
    pub instance_name: String,
}

impl BuiltinService {
    pub fn instance_spec(&self) -> ServiceInstanceSpec {
        ServiceInstanceSpec {
            artifact: self.factory.artifact().into(),
            id: self.instance_id,
            name: self.instance_name.clone(),
        }
    }
}

impl DispatcherBuilder {
    pub fn new(requests: mpsc::Sender<InternalRequest>) -> Self {
        Self {
            dispatcher: Dispatcher::new(requests),
            builtin_runtime: RustRuntime::default(),
        }
    }

    /// Adds built-in service with predefined identifier, keep in mind that the initialize method
    /// of service will not be invoked and thus service must have and empty constructor.
    pub fn with_builtin_service(mut self, service: impl Into<BuiltinService>) -> Self {
        let service = service.into();
        // Registers service instance in runtime.
        let spec = service.instance_spec();
        // Deploys builtin service artifact.
        self.builtin_runtime.add_service_factory(service.factory);
        self.builtin_runtime
            .begin_deploy(&spec.artifact)
            .and_then(|_| {
                let status = self
                    .builtin_runtime
                    .check_deploy_status(&spec.artifact, false)?;
                assert_eq!(
                    status,
                    DeployStatus::Deployed,
                    "Builtin services must be deployed instantly."
                );
                Ok(())
            })
            .expect("Unable to deploy builtin service");
        // Starts builtin service instance.
        self.builtin_runtime
            .start_service(&spec)
            .expect("Unable to start builtin service instance");
        // Registers service instance in dispatcher.
        self.dispatcher
            .notify_service_started(service.instance_id, spec.artifact);
        self
    }

    /// Adds service factory to the Rust runtime.
    pub fn with_service_factory(
        mut self,
        service_factory: impl Into<Box<dyn ServiceFactory>>,
    ) -> Self {
        self.builtin_runtime
            .add_service_factory(service_factory.into());
        self
    }

    /// Adds given service factories to the Rust runtime.
    pub fn with_service_factories(
        mut self,
        service_factories: impl IntoIterator<Item = impl Into<Box<dyn ServiceFactory>>>,
    ) -> Self {
        for factory in service_factories {
            self.builtin_runtime.add_service_factory(factory.into());
        }
        self
    }

    /// Adds additional runtime.
    pub fn with_runtime(mut self, id: u32, runtime: impl Into<Box<dyn Runtime>>) -> Self {
        self.dispatcher.add_runtime(id, runtime);
        self
    }

    pub fn finalize(mut self) -> Dispatcher {
        self.dispatcher
            .add_runtime(RustRuntime::ID as u32, self.builtin_runtime);
        self.dispatcher
    }
}

// TODO Update action names in according with changes in runtime. [ECR-3222]
#[derive(Debug)]
pub enum Action {
    BeginDeploy {
        artifact: ArtifactSpec,
    },
    StartService {
        spec: ServiceInstanceSpec,
        constructor: ServiceConstructor,
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
                dispatcher.begin_deploy(&artifact).map_err(From::from)
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
        runtime::RuntimeIdentifier,
    };

    use super::*;

    enum SampleRuntimes {
        First = 2,
        Second = 3,
    }

    impl DispatcherBuilder {
        fn dummy() -> Self {
            Self::new(mpsc::channel(0).0)
        }
    }

    #[derive(Debug)]
    struct SampleRuntime {
        pub runtime_type: u32,
        pub instance_id: ServiceInstanceId,
        pub method_id: MethodId,
    }

    impl SampleRuntime {
        pub fn new(runtime_type: u32, instance_id: ServiceInstanceId, method_id: MethodId) -> Self {
            Self {
                runtime_type,
                instance_id,
                method_id,
            }
        }
    }

    impl Runtime for SampleRuntime {
        fn begin_deploy(&mut self, artifact: &ArtifactSpec) -> Result<(), DeployError> {
            if artifact.runtime_id == self.runtime_type {
                Ok(())
            } else {
                Err(DeployError::WrongRuntime)
            }
        }

        fn check_deploy_status(
            &self,
            artifact: &ArtifactSpec,
            _: bool,
        ) -> Result<DeployStatus, DeployError> {
            if artifact.runtime_id == self.runtime_type {
                Ok(DeployStatus::Deployed)
            } else {
                Err(DeployError::WrongRuntime)
            }
        }

        fn start_service(&mut self, spec: &ServiceInstanceSpec) -> Result<(), StartError> {
            if spec.artifact.runtime_id == self.runtime_type {
                Ok(())
            } else {
                Err(StartError::WrongRuntime)
            }
        }

        fn stop_service(&mut self, spec: &ServiceInstanceSpec) -> Result<(), StartError> {
            if spec.artifact.runtime_id == self.runtime_type {
                Ok(())
            } else {
                Err(StartError::WrongRuntime)
            }
        }

        fn configure_service(
            &self,
            _context: &mut RuntimeContext,
            spec: &ServiceInstanceSpec,
            _parameters: &ServiceConstructor,
        ) -> Result<(), StartError> {
            if spec.artifact.runtime_id == self.runtime_type {
                Ok(())
            } else {
                Err(StartError::WrongRuntime)
            }
        }

        fn execute(
            &self,
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

        fn before_commit(&self, _: &mut Fork) {}

        fn after_commit(
            &self,
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

        let dispatcher = DispatcherBuilder::dummy()
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
    fn test_dispatcher() {
        const RUST_SERVICE_ID: ServiceInstanceId = 0;
        const JAVA_SERVICE_ID: ServiceInstanceId = 1;
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

        let mut dispatcher = DispatcherBuilder::dummy()
            .with_runtime(runtime_a.runtime_type, runtime_a)
            .with_runtime(runtime_b.runtime_type, runtime_b)
            .finalize();

        let sample_rust_spec = ArtifactSpec {
            runtime_id: SampleRuntimes::First as u32,
            raw_spec: Default::default(),
        };
        let sample_java_spec = ArtifactSpec {
            runtime_id: SampleRuntimes::Second as u32,
            raw_spec: Default::default(),
        };

        // Check deploy.
        dispatcher
            .begin_deploy(&sample_rust_spec)
            .expect("start_deploy failed for rust");
        dispatcher
            .begin_deploy(&sample_java_spec)
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

        // Check if we can init services.
        let fork = db.fork();
        let mut context = RuntimeContext::new(&fork, PublicKey::zero(), Hash::zero());

        dispatcher
            .start_service(
                &mut context,
                ServiceInstanceSpec {
                    artifact: sample_rust_spec.clone(),
                    id: RUST_SERVICE_ID,
                    name: RUST_SERVICE_NAME.into(),
                },
                &ServiceConstructor::default(),
            )
            .expect("init_service failed for rust");

        dispatcher
            .start_service(
                &mut context,
                ServiceInstanceSpec {
                    artifact: sample_java_spec.clone(),
                    id: JAVA_SERVICE_ID,
                    name: JAVA_SERVICE_NAME.into(),
                },
                &ServiceConstructor::default(),
            )
            .expect("init_service failed for java");

        // Check if we can execute transactions.
        let tx_payload = [0x00_u8; 1];

        dispatcher
            .execute(
                &mut context,
                CallInfo::new(RUST_SERVICE_ID, RUST_METHOD_ID),
                &tx_payload,
            )
            .expect("Correct tx rust");

        dispatcher
            .execute(
                &mut context,
                CallInfo::new(RUST_SERVICE_ID, JAVA_METHOD_ID),
                &tx_payload,
            )
            .expect_err("Incorrect tx rust");

        dispatcher
            .execute(
                &mut context,
                CallInfo::new(JAVA_SERVICE_ID, JAVA_METHOD_ID),
                &tx_payload,
            )
            .expect("Correct tx java");

        dispatcher
            .execute(
                &mut context,
                CallInfo::new(JAVA_SERVICE_ID, RUST_METHOD_ID),
                &tx_payload,
            )
            .expect_err("Incorrect tx java");
    }

    #[test]
    fn test_dispatcher_rust_runtime_no_service() {
        const RUST_SERVICE_ID: ServiceInstanceId = 0;
        const RUST_SERVICE_NAME: &str = "rust-service";
        const RUST_METHOD_ID: MethodId = 0;

        // Create dispatcher and test data.
        let db = TemporaryDB::new();

        let mut dispatcher = DispatcherBuilder::dummy().finalize();

        let sample_rust_spec = ArtifactSpec {
            runtime_id: RuntimeIdentifier::Rust as u32,
            raw_spec: Default::default(),
        };

        // Check deploy.
        assert_eq!(
            dispatcher
                .begin_deploy(&sample_rust_spec)
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
                    ServiceInstanceSpec {
                        artifact: sample_rust_spec.clone(),
                        id: RUST_SERVICE_ID,
                        name: RUST_SERVICE_NAME.into()
                    },
                    &ServiceConstructor::default()
                )
                .expect_err("init_service succeed"),
            StartError::WrongArtifact
        );

        // Check if we can execute transactions.
        let tx_payload = [0x00_u8; 1];

        dispatcher
            .execute(
                &mut context,
                CallInfo::new(RUST_SERVICE_ID, RUST_METHOD_ID),
                &tx_payload,
            )
            .expect_err("execute succeed");
    }
}
