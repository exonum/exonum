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

use std::collections::HashMap;

use super::{
    error::{DeployError, ExecutionError, InitError},
    ArtifactSpec, CallInfo, DeployStatus, EnvContext, InstanceInitData, RuntimeEnvironment,
    ServiceInstanceId,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RuntimeIdentifier {
    Rust,
    Java,
}

impl From<ArtifactSpec> for RuntimeIdentifier {
    fn from(spec: ArtifactSpec) -> Self {
        match spec {
            ArtifactSpec::Rust(..) => RuntimeIdentifier::Rust,
            ArtifactSpec::Java => RuntimeIdentifier::Java,
        }
    }
}

#[derive(Default)]
struct DispatcherBuilder {
    runtimes: HashMap<RuntimeIdentifier, Box<dyn RuntimeEnvironment>>,
}

impl DispatcherBuilder {
    pub fn with_runtime(
        mut self,
        runtime_id: RuntimeIdentifier,
        runtime: Box<dyn RuntimeEnvironment>,
    ) -> Self {
        self.runtimes.insert(runtime_id, runtime);

        self
    }

    pub fn finalize(self) -> Dispatcher {
        Dispatcher::new(self.runtimes)
    }
}

#[derive(Default)]
struct Dispatcher {
    runtimes: HashMap<RuntimeIdentifier, Box<dyn RuntimeEnvironment>>,
    runtime_lookup: HashMap<ServiceInstanceId, RuntimeIdentifier>,
}

impl Dispatcher {
    pub fn new(runtimes: HashMap<RuntimeIdentifier, Box<dyn RuntimeEnvironment>>) -> Self {
        Self {
            runtimes,
            runtime_lookup: Default::default(),
        }
    }

    fn notify_service_started(&mut self, service_id: ServiceInstanceId, artifact: ArtifactSpec) {
        let runtime_id = artifact.into();

        self.runtime_lookup.insert(service_id, runtime_id);
    }
}

impl RuntimeEnvironment for Dispatcher {
    fn start_deploy(&self, artifact: ArtifactSpec) -> Result<(), DeployError> {
        let runtime_id = artifact.clone().into();

        if let Some(runtime) = self.runtimes.get(&runtime_id) {
            runtime.start_deploy(artifact)
        } else {
            Err(DeployError::WrongRuntime)
        }
    }

    fn check_deploy_status(&self, artifact: ArtifactSpec) -> Result<DeployStatus, DeployError> {
        let runtime_id = artifact.clone().into();

        if let Some(runtime) = self.runtimes.get(&runtime_id) {
            runtime.check_deploy_status(artifact)
        } else {
            Err(DeployError::WrongRuntime)
        }
    }

    fn init_service(
        &mut self,
        ctx: &mut EnvContext,
        artifact: ArtifactSpec,
        init: &InstanceInitData,
    ) -> Result<(), InitError> {
        let runtime_id = artifact.clone().into();

        if let Some(runtime) = self.runtimes.get_mut(&runtime_id) {
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
        context: &mut EnvContext,
        call_info: CallInfo,
        payload: &[u8],
    ) -> Result<(), ExecutionError> {
        let runtime_id = self.runtime_lookup.get(&call_info.instance_id);

        if runtime_id.is_none() {
            return Err(ExecutionError::with_description(0x00, "Wrong runtime"));
        }

        if let Some(runtime) = self.runtimes.get(&runtime_id.unwrap()) {
            runtime.execute(context, call_info, payload)
        } else {
            // TODO: Execution error code should be determined.
            Err(ExecutionError::with_description(0x00, "Wrong runtime"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{rust::RustArtifactSpec, MethodId};
    use super::*;
    use crate::storage::{Database, MemoryDB};

    struct SampleRuntime {
        pub runtime_type: RuntimeIdentifier,
        pub instance_id: ServiceInstanceId,
        pub method_id: MethodId,
    }

    impl SampleRuntime {
        pub fn new(
            runtime_type: RuntimeIdentifier,
            instance_id: ServiceInstanceId,
            method_id: MethodId,
        ) -> Self {
            Self {
                runtime_type,
                instance_id,
                method_id,
            }
        }
    }

    impl RuntimeEnvironment for SampleRuntime {
        fn start_deploy(&self, artifact: ArtifactSpec) -> Result<(), DeployError> {
            let runtime_type: RuntimeIdentifier = artifact.into();
            if runtime_type == self.runtime_type {
                Ok(())
            } else {
                Err(DeployError::WrongRuntime)
            }
        }
        fn check_deploy_status(&self, artifact: ArtifactSpec) -> Result<DeployStatus, DeployError> {
            let runtime_type: RuntimeIdentifier = artifact.into();
            if runtime_type == self.runtime_type {
                Ok(DeployStatus::Deployed)
            } else {
                Err(DeployError::WrongRuntime)
            }
        }

        fn init_service(
            &mut self,
            _: &mut EnvContext,
            artifact: ArtifactSpec,
            _: &InstanceInitData,
        ) -> Result<(), InitError> {
            let runtime_type: RuntimeIdentifier = artifact.into();
            if runtime_type == self.runtime_type {
                Ok(())
            } else {
                Err(InitError::WrongRuntime)
            }
        }

        fn execute(
            &self,
            _: &mut EnvContext,
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
        let runtime_a = Box::new(SampleRuntime::new(
            RuntimeIdentifier::Rust,
            0,
            "".to_owned(),
        ));

        let runtime_b = Box::new(SampleRuntime::new(
            RuntimeIdentifier::Java,
            1,
            "".to_owned(),
        ));

        let dispatcher = DispatcherBuilder::default()
            .with_runtime(runtime_a.runtime_type.clone(), runtime_a)
            .with_runtime(runtime_b.runtime_type.clone(), runtime_b)
            .finalize();

        assert!(dispatcher.runtimes.get(&RuntimeIdentifier::Rust).is_some());
        assert!(dispatcher.runtimes.get(&RuntimeIdentifier::Java).is_some());
    }

    #[test]
    fn test_dispatcher() {
        const RUST_SERVICE_ID: ServiceInstanceId = 0;
        const JAVA_SERVICE_ID: ServiceInstanceId = 1;
        const RUST_METHOD_NAME: &str = "a";
        const JAVA_METHOD_NAME: &str = "b";

        // Create dispatcher and test data.
        let db = MemoryDB::new();

        let runtime_a = Box::new(SampleRuntime::new(
            RuntimeIdentifier::Rust,
            RUST_SERVICE_ID,
            RUST_METHOD_NAME.to_owned(),
        ));
        let runtime_b = Box::new(SampleRuntime::new(
            RuntimeIdentifier::Java,
            JAVA_SERVICE_ID,
            JAVA_METHOD_NAME.to_owned(),
        ));

        let mut dispatcher = DispatcherBuilder::default()
            .with_runtime(runtime_a.runtime_type.clone(), runtime_a)
            .with_runtime(runtime_b.runtime_type.clone(), runtime_b)
            .finalize();

        let sample_rust_spec = ArtifactSpec::Rust(RustArtifactSpec {
            name: "artifact".to_owned(),
            version: (0, 1, 0),
        });
        let sample_java_spec = ArtifactSpec::Java;

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
                .check_deploy_status(sample_rust_spec.clone())
                .unwrap(),
            DeployStatus::Deployed
        );
        assert_eq!(
            dispatcher
                .check_deploy_status(sample_java_spec.clone())
                .unwrap(),
            DeployStatus::Deployed
        );

        // Check if we can init services.
        let mut fork = db.fork();
        let mut context = EnvContext::from_fork(&mut fork);

        let rust_init_data = InstanceInitData {
            instance_id: RUST_SERVICE_ID,
            constructor_data: None,
        };
        dispatcher
            .init_service(&mut context, sample_rust_spec.clone(), &rust_init_data)
            .expect("init_service failed for rust");

        let java_init_data = InstanceInitData {
            instance_id: JAVA_SERVICE_ID,
            constructor_data: None,
        };
        dispatcher
            .init_service(&mut context, sample_java_spec.clone(), &java_init_data)
            .expect("init_service failed for java");

        // Check if we can execute transactions.
        let tx_payload = [0x00_u8; 1];

        dispatcher
            .execute(
                &mut context,
                CallInfo::new(RUST_SERVICE_ID, RUST_METHOD_NAME.to_owned()),
                &tx_payload,
            )
            .expect("Correct tx rust");

        dispatcher
            .execute(
                &mut context,
                CallInfo::new(RUST_SERVICE_ID, JAVA_METHOD_NAME.to_owned()),
                &tx_payload,
            )
            .expect_err("Incorrect tx rust");

        dispatcher
            .execute(
                &mut context,
                CallInfo::new(JAVA_SERVICE_ID, JAVA_METHOD_NAME.to_owned()),
                &tx_payload,
            )
            .expect("Correct tx java");

        dispatcher
            .execute(
                &mut context,
                CallInfo::new(JAVA_SERVICE_ID, RUST_METHOD_NAME.to_owned()),
                &tx_payload,
            )
            .expect_err("Incorrect tx java");
    }

    #[test]
    fn test_dispatcher_no_service() {
        const RUST_SERVICE_ID: ServiceInstanceId = 0;
        const RUST_METHOD_NAME: &str = "a";

        // Create dispatcher and test data.
        let db = MemoryDB::new();

        let mut dispatcher = DispatcherBuilder::default().finalize();

        let sample_rust_spec = ArtifactSpec::Rust(RustArtifactSpec {
            name: "artifact".to_owned(),
            version: (0, 1, 0),
        });

        // Check deploy.
        assert_eq!(
            dispatcher
                .start_deploy(sample_rust_spec.clone())
                .expect_err("start_deploy succeed"),
            DeployError::WrongRuntime
        );

        assert_eq!(
            dispatcher
                .check_deploy_status(sample_rust_spec.clone())
                .expect_err("check_deploy_status succeed"),
            DeployError::WrongRuntime
        );

        // Check if we can init services.
        let mut fork = db.fork();
        let mut context = EnvContext::from_fork(&mut fork);

        let rust_init_data = InstanceInitData {
            instance_id: RUST_SERVICE_ID,
            constructor_data: None,
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
                CallInfo::new(RUST_SERVICE_ID, RUST_METHOD_NAME.to_owned()),
                &tx_payload,
            )
            .expect_err("execute succeed");
    }
}
