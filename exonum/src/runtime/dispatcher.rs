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

use std::cell::RefCell;
use std::collections::HashMap;

use super::{
    error::{DeployError, ExecutionError, InitError, WRONG_RUNTIME},
    ArtifactSpec, DeployStatus, InstanceInitData, RuntimeContext, RuntimeEnvironment,
    ServiceInstanceId, RuntimeIdentifier,
};
use crate::messages::CallInfo;

#[derive(Default)]
pub struct DispatcherBuilder {
    runtimes: HashMap<u32, Box<dyn RuntimeEnvironment + Send>>,
}

impl std::fmt::Debug for DispatcherBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "DispatcherBuilder entity")
    }
}

impl DispatcherBuilder {
    pub fn with_runtime(
        mut self,
        runtime_id: u32,
        runtime: Box<dyn RuntimeEnvironment + Send>,
    ) -> Self {
        self.runtimes.insert(runtime_id, runtime);

        self
    }

    pub fn finalize(self) -> Dispatcher {
        Dispatcher::new(self.runtimes)
    }
}

#[derive(Default)]
pub struct Dispatcher {
    runtimes: HashMap<u32, Box<dyn RuntimeEnvironment + Send>>,
    // TODO Is RefCell enough here?
    runtime_lookup: RefCell<HashMap<ServiceInstanceId, u32>>,
}

impl std::fmt::Debug for Dispatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Dispatcher entity")
    }
}

impl Dispatcher {
    pub fn new(runtimes: HashMap<u32, Box<dyn RuntimeEnvironment + Send>>) -> Self {
        Self {
            runtimes,
            runtime_lookup: RefCell::new(Default::default()),
        }
    }

    fn notify_service_started(&self, service_id: ServiceInstanceId, artifact: ArtifactSpec) {
        self.runtime_lookup
            .borrow_mut()
            .insert(service_id, artifact.runtime_id);
    }
}

impl RuntimeEnvironment for Dispatcher {
    fn start_deploy(&self, artifact: ArtifactSpec) -> Result<(), DeployError> {
        if let Some(runtime) = self.runtimes.get(&artifact.runtime_id) {
            runtime.start_deploy(artifact)
        } else {
            Err(DeployError::WrongRuntime)
        }
    }

    fn check_deploy_status(
        &self,
        artifact: ArtifactSpec,
        cancel_if_not_complete: bool,
    ) -> Result<DeployStatus, DeployError> {
        if let Some(runtime) = self.runtimes.get(&artifact.runtime_id) {
            runtime.check_deploy_status(artifact, cancel_if_not_complete)
        } else {
            Err(DeployError::WrongRuntime)
        }
    }

    fn init_service(
        &self,
        ctx: &mut RuntimeContext,
        artifact: ArtifactSpec,
        init: &InstanceInitData,
    ) -> Result<(), InitError> {
        if let Some(runtime) = self.runtimes.get(&artifact.runtime_id) {
            let result = runtime.init_service(ctx, artifact.clone(), init);
            if result.is_ok() {
                self.notify_service_started(init.instance_id.clone(), artifact);
            }
            result
        } else {
            Err(InitError::WrongRuntime)
        }
    }

    fn execute(
        &self,
        context: &mut RuntimeContext,
        call_info: CallInfo,
        payload: &[u8],
    ) -> Result<(), ExecutionError> {
        let lookup = self.runtime_lookup.borrow();
        let runtime_id = lookup.get(&call_info.instance_id);

        if runtime_id.is_none() {
            return Err(ExecutionError::with_description(
                WRONG_RUNTIME,
                "Wrong runtime",
            ));
        }

        if let Some(runtime) = self.runtimes.get(&runtime_id.unwrap()) {
            runtime.execute(context, call_info, payload)
        } else {
            Err(ExecutionError::with_description(
                WRONG_RUNTIME,
                "Wrong runtime",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::rust::RustArtifactSpec;
    use super::*;
    use crate::messages::{MethodId, ServiceInstanceId};
    use crate::storage::{Database, MemoryDB};

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

    impl RuntimeEnvironment for SampleRuntime {
        fn start_deploy(&self, artifact: ArtifactSpec) -> Result<(), DeployError> {
            if artifact.runtime_id == self.runtime_type {
                Ok(())
            } else {
                Err(DeployError::WrongRuntime)
            }
        }

        fn check_deploy_status(
            &self,
            artifact: ArtifactSpec,
            _: bool,
        ) -> Result<DeployStatus, DeployError> {
            if artifact.runtime_id == self.runtime_type {
                Ok(DeployStatus::Deployed)
            } else {
                Err(DeployError::WrongRuntime)
            }
        }

        fn init_service(
            &self,
            _: &mut RuntimeContext,
            artifact: ArtifactSpec,
            _: &InstanceInitData,
        ) -> Result<(), InitError> {
            if artifact.runtime_id == self.runtime_type {
                Ok(())
            } else {
                Err(InitError::WrongRuntime)
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
    }

    #[test]
    fn test_builder() {
        let runtime_a = Box::new(SampleRuntime::new(RuntimeIdentifier::Rust as u32, 0, 0));

        let runtime_b = Box::new(SampleRuntime::new(RuntimeIdentifier::Java as u32, 1, 0));

        let dispatcher = DispatcherBuilder::default()
            .with_runtime(runtime_a.runtime_type, runtime_a)
            .with_runtime(runtime_b.runtime_type, runtime_b)
            .finalize();

        assert!(dispatcher
            .runtimes
            .get(&(RuntimeIdentifier::Rust as u32))
            .is_some());
        assert!(dispatcher
            .runtimes
            .get(&(RuntimeIdentifier::Java as u32))
            .is_some());
    }

    #[test]
    fn test_dispatcher() {
        const RUST_SERVICE_ID: ServiceInstanceId = 0;
        const JAVA_SERVICE_ID: ServiceInstanceId = 1;
        const RUST_METHOD_ID: MethodId = 0;
        const JAVA_METHOD_ID: MethodId = 1;

        // Create dispatcher and test data.
        let db = MemoryDB::new();

        let runtime_a = Box::new(SampleRuntime::new(
            RuntimeIdentifier::Rust as u32,
            RUST_SERVICE_ID,
            RUST_METHOD_ID,
        ));
        let runtime_b = Box::new(SampleRuntime::new(
            RuntimeIdentifier::Java as u32,
            JAVA_SERVICE_ID,
            JAVA_METHOD_ID,
        ));

        let dispatcher = DispatcherBuilder::default()
            .with_runtime(runtime_a.runtime_type, runtime_a)
            .with_runtime(runtime_b.runtime_type, runtime_b)
            .finalize();

        let sample_rust_spec = ArtifactSpec {
            runtime_id: RuntimeIdentifier::Rust as u32,
            raw_spec: Default::default(),
        };
        let sample_java_spec = ArtifactSpec {
            runtime_id: RuntimeIdentifier::Java as u32,
            raw_spec: Default::default(),
        };

        // Check deploy.
        dispatcher
            .start_deploy(sample_rust_spec.clone())
            .expect("start_deploy failed for rust");
        dispatcher
            .start_deploy(sample_java_spec.clone())
            .expect("start_deploy failed for java");

        // Check deploy status
        assert_eq!(
            dispatcher
                .check_deploy_status(sample_rust_spec.clone(), false)
                .unwrap(),
            DeployStatus::Deployed
        );
        assert_eq!(
            dispatcher
                .check_deploy_status(sample_java_spec.clone(), false)
                .unwrap(),
            DeployStatus::Deployed
        );

        // Check if we can init services.
        let mut fork = db.fork();
        let mut context = RuntimeContext::from_fork(&mut fork);

        let rust_init_data = InstanceInitData {
            instance_id: RUST_SERVICE_ID,
            constructor_data: Default::default(),
        };
        dispatcher
            .init_service(&mut context, sample_rust_spec.clone(), &rust_init_data)
            .expect("init_service failed for rust");

        let java_init_data = InstanceInitData {
            instance_id: JAVA_SERVICE_ID,
            constructor_data: Default::default(),
        };
        dispatcher
            .init_service(&mut context, sample_java_spec.clone(), &java_init_data)
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
    fn test_dispatcher_no_service() {
        const RUST_SERVICE_ID: ServiceInstanceId = 0;
        const RUST_METHOD_ID: MethodId = 0;

        // Create dispatcher and test data.
        let db = MemoryDB::new();

        let dispatcher = DispatcherBuilder::default().finalize();

        let sample_rust_spec = ArtifactSpec {
            runtime_id: RuntimeIdentifier::Rust as u32,
            raw_spec: Default::default(),
        };

        // Check deploy.
        assert_eq!(
            dispatcher
                .start_deploy(sample_rust_spec.clone())
                .expect_err("start_deploy succeed"),
            DeployError::WrongRuntime
        );

        assert_eq!(
            dispatcher
                .check_deploy_status(sample_rust_spec.clone(), false)
                .expect_err("check_deploy_status succeed"),
            DeployError::WrongRuntime
        );

        // Check if we can init services.
        let mut fork = db.fork();
        let mut context = RuntimeContext::from_fork(&mut fork);

        let rust_init_data = InstanceInitData {
            instance_id: RUST_SERVICE_ID,
            constructor_data: Default::default(),
        };
        assert_eq!(
            dispatcher
                .init_service(&mut context, sample_rust_spec.clone(), &rust_init_data)
                .expect_err("init_service succeed"),
            InitError::WrongRuntime
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
