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
    ArtifactSpec, DeployStatus, CallInfo, EnvContext, InstanceInitData, RuntimeEnvironment,
};

#[derive(Default)]
struct DispatcherBuilder {
    runtimes: HashMap<String, Box<dyn RuntimeEnvironment>>,
}

impl DispatcherBuilder {
    pub fn with_runtime(
        mut self,
        runtime_name: String,
        runtime: Box<dyn RuntimeEnvironment>,
    ) -> Self {
        self.runtimes.insert(runtime_name, runtime);

        self
    }

    pub fn finalize(self) -> Dispatcher {
        Dispatcher {
            runtimes: self.runtimes,
        }
    }
}

#[derive(Default)]
struct Dispatcher {
    runtimes: HashMap<String, Box<dyn RuntimeEnvironment>>,
}

impl RuntimeEnvironment for Dispatcher {
    fn start_deploy(&self, _artifact: ArtifactSpec) -> Result<(), DeployError> {
        Ok(())
    }
    fn check_deploy_status(&self, _artifact: ArtifactSpec) -> Result<DeployStatus, DeployError> {
        Ok(DeployStatus::Deployed)
    }

    fn init_service(
        &self,
        _: &mut EnvContext,
        _artifact: ArtifactSpec,
        _init: &InstanceInitData,
    ) -> Result<(), InitError> {
        Ok(())
    }

    fn execute(
        &self,
        _context: &mut EnvContext,
        _dispatch: CallInfo,
        _payload: &[u8],
    ) -> Result<(), ExecutionError> {
        Ok(())
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

        fn execute(
            &self,
            _: &mut EnvContext,
            _: CallInfo,
            _: &[u8],
        ) -> Result<(), ExecutionError> {
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

        fn execute(
            &self,
            _: &mut EnvContext,
            _: CallInfo,
            _: &[u8],
        ) -> Result<(), ExecutionError> {
            Ok(())
        }
    }

    #[test]
    fn test_builder() {
        let runtime_a_name = String::from("Runtime_A");
        let runtime_a = Box::new(SampleRuntimeA::default());

        let runtime_b_name = String::from("Runtime_B");
        let runtime_b = Box::new(SampleRuntimeB::default());

        let dispatcher = DispatcherBuilder::default()
            .with_runtime(runtime_a_name, runtime_a)
            .with_runtime(runtime_b_name, runtime_b)
            .finalize();

        assert!(dispatcher.runtimes.get("Runtime_A").is_some());
        assert!(dispatcher.runtimes.get("Runtime_B").is_some());
        assert!(dispatcher.runtimes.get("Runtime_C").is_none());
    }
}
