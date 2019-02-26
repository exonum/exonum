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

    fn notify_service_started(
        &mut self,
        service_id: ServiceInstanceId,
        artifact: ArtifactSpec,
    ) {
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
        &self,
        ctx: &mut EnvContext,
        artifact: ArtifactSpec,
        init: &InstanceInitData,
    ) -> Result<(), InitError> {
        let runtime_id = artifact.clone().into();

        if let Some(runtime) = self.runtimes.get(&runtime_id) {
            runtime.init_service(ctx, artifact, init)
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
        let runtime_id = self.runtime_lookup.get(&call_info.instance_id).unwrap();

        if let Some(runtime) = self.runtimes.get(&runtime_id) {
            runtime.execute(context, call_info, payload)
        } else {
            // TODO: Execution error code should be determined.
            Err(ExecutionError::with_description(0x00, "Wrong runtime"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{ MethodId, rust::RustArtifactSpec };
    use crate::storage::{Database, MemoryDB};

    struct SampleRuntime {
        pub runtime_type: RuntimeIdentifier, 
        pub instance_id: ServiceInstanceId,
        pub method_id: MethodId,
    }

    impl SampleRuntime {
        pub fn new(runtime_type: RuntimeIdentifier, instance_id: ServiceInstanceId, method_id: MethodId) -> Self {
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
            &self,
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

        fn execute(&self, _: &mut EnvContext, call_info: CallInfo, _: &[u8]) -> Result<(), ExecutionError> {
            if call_info.instance_id == self.instance_id && call_info.method_id == self.method_id {
                Ok(())
            } else {
                Err(ExecutionError::new(0xFF_u8))
            }
        }
    }

    #[test]
    fn test_builder() {
        let runtime_a = Box::new(SampleRuntime::new(RuntimeIdentifier::Rust, 0, "".to_owned()));

        let runtime_b = Box::new(SampleRuntime::new(RuntimeIdentifier::Java, 1, "".to_owned()));

        let dispatcher = DispatcherBuilder::default()
            .with_runtime(runtime_a.runtime_type.clone(), runtime_a)
            .with_runtime(runtime_b.runtime_type.clone(), runtime_b)
            .finalize();

        assert!(dispatcher.runtimes.get(&RuntimeIdentifier::Rust).is_some());
        assert!(dispatcher.runtimes.get(&RuntimeIdentifier::Java).is_some());
    }

    #[test]
    fn test_dispatcher() {
        let db = MemoryDB::new();

        let runtime_a = Box::new(SampleRuntime::new(RuntimeIdentifier::Rust, 0, "a".to_owned()));

        let runtime_b = Box::new(SampleRuntime::new(RuntimeIdentifier::Java, 1, "b".to_owned()));

        let dispatcher = DispatcherBuilder::default()
            .with_runtime(runtime_a.runtime_type.clone(), runtime_a)
            .with_runtime(runtime_b.runtime_type.clone(), runtime_b)
            .finalize();

        let sample_rust_spec = ArtifactSpec::Rust( RustArtifactSpec { name: "artifact".to_owned(), version: (0, 1, 0)} );
        let sample_java_spec = ArtifactSpec::Java;

        assert!(dispatcher.start_deploy(sample_rust_spec.clone()).is_ok());
        assert!(dispatcher.start_deploy(sample_java_spec.clone()).is_ok());

        assert!(dispatcher.check_deploy_status(sample_rust_spec.clone()).is_ok());
        assert!(dispatcher.check_deploy_status(sample_java_spec.clone()).is_ok());

        {

            let init_data = InstanceInitData {
                instance_id: 0,
                constructor_data: None,
            };
            let mut fork = db.fork();
            let mut context = EnvContext::from_fork(&mut fork);
            assert!(dispatcher.init_service(&mut context, sample_rust_spec.clone(), &init_data).is_ok());
        }
        {

            let init_data = InstanceInitData {
                instance_id: 1,
                constructor_data: None,
            };
            let mut fork = db.fork();
            let mut context = EnvContext::from_fork(&mut fork);
            assert!(dispatcher.init_service(&mut context, sample_java_spec.clone(), &init_data).is_ok());   
        }
    }
}
