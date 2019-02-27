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

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
};

#[macro_use]
pub mod service;

#[cfg(test)]
pub mod tests;

use super::{
    error::{DeployError, ExecutionError, InitError},
    ArtifactSpec, CallInfo, DeployStatus, EnvContext, InstanceInitData, MethodId,
    RuntimeEnvironment, ServiceInstanceId,
};

use self::service::{ServiceDispatcher, SystemService};

#[derive(Default)]
struct RustRuntime {
    inner: RefCell<RustRuntimeInner>,
}

impl RustRuntime {
    fn add_service(&self, artifact: RustArtifactSpec, service: Box<dyn SystemService>) {
        self.inner.borrow_mut().services.insert(artifact, service);
    }
}

#[derive(Default)]
struct RustRuntimeInner {
    services: HashMap<RustArtifactSpec, Box<dyn SystemService>>,
    deployed: HashSet<RustArtifactSpec>,
    initialized: HashMap<ServiceInstanceId, Box<dyn SystemService>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RustArtifactSpec {
    pub name: String,
    pub version: (u32, u32, u32),
}

impl RuntimeEnvironment for RustRuntime {
    fn start_deploy(&self, artifact: ArtifactSpec) -> Result<(), DeployError> {
        let artifact = if let ArtifactSpec::Rust(artifact) = artifact {
            artifact
        } else {
            return Err(DeployError::WrongArtifact);
        };

        let mut inner = self.inner.borrow_mut();

        if !inner.services.contains_key(&artifact) {
            return Err(DeployError::FailedToDeploy);
        }

        if !inner.deployed.insert(artifact) {
            return Err(DeployError::AlreadyDeployed);
        }
        Ok(())
    }

    fn check_deploy_status(&self, artifact: ArtifactSpec) -> Result<DeployStatus, DeployError> {
        let artifact = if let ArtifactSpec::Rust(artifact) = artifact {
            artifact
        } else {
            return Err(DeployError::WrongArtifact);
        };

        let inner = self.inner.borrow();

        if inner.deployed.contains(&artifact) {
            Ok(DeployStatus::Deployed)
        } else {
            Err(DeployError::FailedToDeploy)
        }
    }

    fn init_service(
        &mut self,
        _: &mut EnvContext,
        artifact: ArtifactSpec,
        init: &InstanceInitData,
    ) -> Result<(), InitError> {
        let artifact = if let ArtifactSpec::Rust(artifact) = artifact {
            artifact
        } else {
            return Err(InitError::WrongArtifact);
        };

        let mut inner = self.inner.borrow_mut();

        if !inner.deployed.contains(&artifact) {
            return Err(InitError::NotDeployed);
        }

        if inner.initialized.contains_key(&init.instance_id) {
            return Err(InitError::ServiceIdExists);
        }

        let service = inner.services.remove(&artifact).unwrap();
        inner.initialized.insert(init.instance_id, service);
        Ok(())
    }

    fn execute(
        &self,
        context: &mut EnvContext,
        dispatch: CallInfo,
        payload: &[u8],
    ) -> Result<(), ExecutionError> {
        let inner = self.inner.borrow();
        let instance = inner.initialized.get(&dispatch.instance_id).unwrap();

        let mut ctx = TransactionContext::from_env_ctx(context);

        instance
            .call(dispatch.method_id, ctx, payload)
            .expect("Dispatch error")
    }
}

pub struct TransactionContext<'a> {
    _env_context: &'a EnvContext<'a>,
}

impl<'a> TransactionContext<'a> {
    fn from_env_ctx(env_context: &'a EnvContext<'a>) -> Self {
        Self {
            _env_context: env_context,
        }
    }
}
