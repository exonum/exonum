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

use super::{
    ArtifactSpec, DispatchInfo, EnvContext, InstanceInitData, RuntimeEnvironment, DeployStatus,
    error::{DeployError, InitError, ExecutionError}
};

#[derive(Default)]
struct DispatcherBuilder {
    // runtimes: HashMap
}

impl DispatcherBuilder {
    pub fn with_runtime(self) -> Self
    {
        self
    }

    pub fn finalize(self) -> Dispatcher
    {
        Dispatcher::default()
    }
}

#[derive(Default)]
struct Dispatcher {
    // TODO add runtimes
}

impl Dispatcher {
    
}

impl RuntimeEnvironment for Dispatcher {
    fn start_deploy(&self, artifact: ArtifactSpec) -> Result<(), DeployError> {
        Ok(())
    }
    fn check_deploy_status(&self, artifact: ArtifactSpec) -> Result<DeployStatus, DeployError>
    {
        Ok(DeployStatus::Deployed)
    }

    fn init_service(
        &self,
        _: &mut EnvContext,
        artifact: ArtifactSpec,
        init: &InstanceInitData,
    ) -> Result<(), InitError> {
        Ok(())
    }

    fn execute(&self, context: &mut EnvContext, dispatch: DispatchInfo, payload: &[u8]) -> Result<(), ExecutionError> {
        Ok(())
    }
}
