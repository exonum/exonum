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
    ArtifactSpec, CallInfo, DeployStatus, EnvContext, InstanceInitData, RuntimeEnvironment,
    ServiceInstanceId,
};

use crate::crypto::{Hash, PublicKey};
use crate::storage::Fork;

use self::service::SystemService;

#[derive(Debug, Default)]
struct RustRuntime {
    inner: RefCell<RustRuntimeInner>,
}

impl RustRuntime {
    fn add_service(&self, artifact: RustArtifactSpec, service: Box<dyn SystemService>) {
        self.inner.borrow_mut().services.insert(artifact, service);
    }
}

#[derive(Debug, Default)]
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

        let ctx = TransactionContext::new(context, self);

        instance
            .call(dispatch.method_id, ctx, payload)
            .expect("Dispatch error")
    }
}

#[derive(Debug)]
pub struct TransactionContext<'a, 'c> {
    env_context: &'a mut EnvContext<'c>,
    runtime: &'a RustRuntime,
}

impl<'a, 'c> TransactionContext<'a, 'c> {
    fn new(env_context: &'a mut EnvContext<'c>, runtime: &'a RustRuntime) -> Self {
        Self {
            env_context,
            runtime,
        }
    }

    pub fn fork(&mut self) -> &mut Fork {
        self.env_context.fork
    }

    pub fn tx_hash(&self) -> Hash {
        self.env_context.tx_hash
    }

    pub fn author(&self) -> PublicKey {
        self.env_context.author
    }

    pub fn dispatch_call(
        &mut self,
        dispatch: CallInfo,
        payload: &[u8],
    ) -> Result<(), ExecutionError> {
        self.runtime.execute(self.env_context, dispatch, payload)
    }
}
