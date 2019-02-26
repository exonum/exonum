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

#[derive(Debug, PartialEq, Eq, Hash)]
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

    pub fn notify_service_started(
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

    #[derive(Default)]
    struct SampleRuntimeA {}

    impl RuntimeEnvironment for SampleRuntimeA {
        fn start_deploy(&self, _: ArtifactSpec) -> Result<(), DeployError> {
            Ok(())
        }
        fn check_deploy_status(&self, _: ArtifactSpec) -> Result<DeployStatus, DeployError> {
            Ok(DeployStatus::Deployed)
        }

        fn init_service(
            &self,
            _: &mut EnvContext,
            _: ArtifactSpec,
            _: &InstanceInitData,
        ) -> Result<(), InitError> {
            Ok(())
        }

        fn execute(&self, _: &mut EnvContext, _: CallInfo, _: &[u8]) -> Result<(), ExecutionError> {
            Ok(())
        }
    }

    #[derive(Default)]
    struct SampleRuntimeB {}

    impl RuntimeEnvironment for SampleRuntimeB {
        fn start_deploy(&self, _: ArtifactSpec) -> Result<(), DeployError> {
            Ok(())
        }
        fn check_deploy_status(&self, _: ArtifactSpec) -> Result<DeployStatus, DeployError> {
            Ok(DeployStatus::Deployed)
        }

        fn init_service(
            &self,
            _: &mut EnvContext,
            _: ArtifactSpec,
            _: &InstanceInitData,
        ) -> Result<(), InitError> {
            Ok(())
        }

        fn execute(&self, _: &mut EnvContext, _: CallInfo, _: &[u8]) -> Result<(), ExecutionError> {
            Ok(())
        }
    }

    #[test]
    fn test_builder() {
        let runtime_a = Box::new(SampleRuntimeA::default());

        let runtime_b = Box::new(SampleRuntimeB::default());

        let dispatcher = DispatcherBuilder::default()
            .with_runtime(RuntimeIdentifier::Rust, runtime_a)
            .with_runtime(RuntimeIdentifier::Java, runtime_b)
            .finalize();

        assert!(dispatcher.runtimes.get(&RuntimeIdentifier::Rust).is_some());
        assert!(dispatcher.runtimes.get(&RuntimeIdentifier::Java).is_some());
    }
}
