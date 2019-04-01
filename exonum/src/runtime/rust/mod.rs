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

use semver::Version;

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    panic
};

#[macro_use]
pub mod service;

#[cfg(test)]
pub mod tests;

use super::{
    error::{DeployError, ExecutionError, InitError, DISPATCH_ERROR},
    ArtifactSpec, DeployStatus, InstanceInitData, RuntimeContext, RuntimeEnvironment,
    RuntimeIdentifier, ServiceInstanceId,
};

use crate::crypto::{Hash, PublicKey};
use crate::messages::{BinaryForm, CallInfo};
use crate::proto::schema;
use crate::storage::{Fork, Error as StorageError};

use self::service::Service;

#[derive(Debug, Default)]
pub struct RustRuntime {
    // TODO: think about ways to share runtime.
    inner: RefCell<RustRuntimeInner>,
}

impl RustRuntime {
    fn get_artifact_spec(&self, artifact: ArtifactSpec) -> Option<RustArtifactSpec> {
        if artifact.runtime_id != RuntimeIdentifier::Rust as u32 {
            return None;
        }

        let rust_artifact_spec: RustArtifactSpec = BinaryForm::decode(&artifact.raw_spec).ok()?;

        Some(rust_artifact_spec)
    }

    fn add_service(&self, artifact: RustArtifactSpec, service: Box<dyn Service>) {
        self.inner.borrow_mut().services.insert(artifact, service);
    }
}

unsafe impl Send for RustRuntime {}

#[derive(Debug, Default)]
struct RustRuntimeInner {
    // TODO: Add link to dispatcher
    services: HashMap<RustArtifactSpec, Box<dyn Service>>,
    deployed: HashSet<RustArtifactSpec>,
    initialized: HashMap<ServiceInstanceId, Box<dyn Service>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, ProtobufConvert)]
#[exonum(pb = "schema::runtime::RustArtifactSpec", crate = "crate")]
pub struct RustArtifactSpec {
    pub name: String,
    pub version: Version,
}

impl RuntimeEnvironment for RustRuntime {
    fn start_deploy(&self, artifact: ArtifactSpec) -> Result<(), DeployError> {
        let artifact = self
            .get_artifact_spec(artifact)
            .ok_or(DeployError::WrongArtifact)?;

        let mut inner = self.inner.borrow_mut();

        if !inner.services.contains_key(&artifact) {
            return Err(DeployError::FailedToDeploy);
        }

        if !inner.deployed.insert(artifact) {
            return Err(DeployError::AlreadyDeployed);
        }
        Ok(())
    }

    fn check_deploy_status(
        &self,
        artifact: ArtifactSpec,
        _cancel_if_not_complete: bool,
    ) -> Result<DeployStatus, DeployError> {
        let artifact = self
            .get_artifact_spec(artifact)
            .ok_or(DeployError::WrongArtifact)?;

        let inner = self.inner.borrow();

        if inner.deployed.contains(&artifact) {
            Ok(DeployStatus::Deployed)
        } else {
            Err(DeployError::FailedToDeploy)
        }
    }

    fn init_service(
        &self,
        context: &mut RuntimeContext,
        artifact: ArtifactSpec,
        init: &InstanceInitData,
    ) -> Result<(), InitError> {
        let artifact = self
            .get_artifact_spec(artifact)
            .ok_or(InitError::WrongArtifact)?;

        let mut inner = self.inner.borrow_mut();

        if !inner.deployed.contains(&artifact) {
            return Err(InitError::NotDeployed);
        }

        if inner.initialized.contains_key(&init.instance_id) {
            return Err(InitError::ServiceIdExists);
        }

        let service = inner.services.remove(&artifact).unwrap();
        inner.initialized.insert(init.instance_id, service);

        let ctx = TransactionContext::new(context, self);
        inner
            .initialized
            .get_mut(&init.instance_id)
            .unwrap()
            .initialize(ctx, init.constructor_data.clone())
            .map_err(|e| InitError::ExecutionError(e))
    }

    fn execute(
        &self,
        context: &mut RuntimeContext,
        dispatch: CallInfo,
        payload: &[u8],
    ) -> Result<(), ExecutionError> {
        let inner = self.inner.borrow();
        let instance = inner.initialized.get(&dispatch.instance_id).unwrap();

        let ctx = TransactionContext::new(context, self);

        instance
            .call(dispatch.method_id, ctx, payload)
            .map_err(|e| {
                ExecutionError::with_description(DISPATCH_ERROR, format!("Dispatch error: {}", e))
            })?
    }

    fn before_commit(&self, fork: &mut Fork)
    {
        let inner = self.inner.borrow();

        for (_, service) in &inner.initialized {
            fork.checkpoint();
            match panic::catch_unwind(panic::AssertUnwindSafe(|| service.before_commit(fork))) {
                Ok(..) => fork.commit(),
                Err(err) => {
                    if err.is::<StorageError>() {
                        // Continue panic unwind if the reason is StorageError.
                        panic::resume_unwind(err);
                    }
                    fork.rollback();

                    // TODO add service name
                    error!(
                        "Service before_commit failed with error: {:?}",
                        err
                    );
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct TransactionContext<'a, 'c> {
    env_context: &'a mut RuntimeContext<'c>,
    runtime: &'a RustRuntime,
}

impl<'a, 'c> TransactionContext<'a, 'c> {
    fn new(env_context: &'a mut RuntimeContext<'c>, runtime: &'a RustRuntime) -> Self {
        Self {
            env_context,
            runtime,
        }
    }

    pub fn env_context(&mut self) -> &mut RuntimeContext<'c> {
        self.env_context
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
