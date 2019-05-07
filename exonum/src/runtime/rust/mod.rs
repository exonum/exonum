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
    collections::{HashMap, HashSet},
    panic,
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
use exonum_merkledb::{Error as StorageError, Fork, Snapshot};

use self::service::{Service, ServiceFactory};
use crate::api::ServiceApiBuilder;
use crate::runtime::configuration_new::{DEPLOY_METHOD_ID, INIT_METHOD_ID, SERVICE_ID};
use crate::runtime::dispatcher::Dispatcher;
use protobuf::well_known_types::Any;

#[derive(Debug)]
pub struct RustRuntime {
    inner: RustRuntimeInner,
    dispatcher: *mut Dispatcher,
}

impl RustRuntime {
    pub fn new(dispatcher: &mut Dispatcher) -> Self {
        Self {
            inner: Default::default(),
            dispatcher: &mut *dispatcher,
        }
    }
}

#[derive(Debug)]
pub struct InitializedService {
    id: ServiceInstanceId,
    service: Box<dyn Service>,
}

impl InitializedService {
    pub fn new(id: ServiceInstanceId, service: Box<dyn Service>) -> Self {
        Self { id, service }
    }

    pub fn state_hash(&self, snapshot: &dyn Snapshot) -> (ServiceInstanceId, Vec<Hash>) {
        (self.id, self.service.state_hash(snapshot))
    }
}

impl AsRef<dyn Service + 'static> for InitializedService {
    fn as_ref(&self) -> &(dyn Service + 'static) {
        self.service.as_ref()
    }
}

impl AsMut<dyn Service + 'static> for InitializedService {
    fn as_mut(&mut self) -> &mut (dyn Service + 'static) {
        self.service.as_mut()
    }
}

impl RustRuntime {
    fn get_artifact_spec(&self, artifact: ArtifactSpec) -> Option<RustArtifactSpec> {
        if artifact.runtime_id != RuntimeIdentifier::Rust as u32 {
            return None;
        }

        let rust_artifact_spec: RustArtifactSpec = BinaryForm::decode(&artifact.raw_spec).ok()?;

        Some(rust_artifact_spec)
    }

    pub fn add_service(&mut self, service_factory: Box<dyn ServiceFactory>) {
        self.inner
            .services
            .insert(service_factory.artifact(), service_factory);
    }

    #[allow(unsafe_code)]
    fn get_dispatcher(&self) -> &Dispatcher {
        unsafe {
            self.dispatcher
                .as_ref()
                .expect("*mut Dispatcher dereference.")
        }
    }
}

#[allow(unsafe_code)]
unsafe impl Send for RustRuntime {}

#[derive(Debug, Default)]
struct RustRuntimeInner {
    // TODO: Add link to dispatcher
    services: HashMap<RustArtifactSpec, Box<dyn ServiceFactory>>,
    deployed: HashSet<RustArtifactSpec>,
    initialized: HashMap<ServiceInstanceId, InitializedService>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, ProtobufConvert)]
#[exonum(pb = "schema::runtime::RustArtifactSpec", crate = "crate")]
pub struct RustArtifactSpec {
    pub name: String,
    pub version: Version,
}

impl RustArtifactSpec {
    pub fn new(name: &str, major: u64, minor: u64, patch: u64) -> Self {
        Self {
            name: name.to_owned(),
            version: Version::new(major, minor, patch),
        }
    }

    pub fn into_pb_any(&self) -> Any {
        let mut any = Any::new();
        any.set_value(self.encode().unwrap());
        any
    }
}

impl RuntimeEnvironment for RustRuntime {
    fn start_deploy(&mut self, artifact: ArtifactSpec) -> Result<(), DeployError> {
        let artifact = self
            .get_artifact_spec(artifact)
            .ok_or(DeployError::WrongArtifact)?;

        let inner = &mut self.inner;

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

        let inner = &self.inner;

        if inner.deployed.contains(&artifact) {
            Ok(DeployStatus::Deployed)
        } else {
            Err(DeployError::FailedToDeploy)
        }
    }

    fn init_service(
        &mut self,
        context: &mut RuntimeContext,
        artifact: ArtifactSpec,
        init: &InstanceInitData,
    ) -> Result<(), InitError> {
        let artifact = self
            .get_artifact_spec(artifact)
            .ok_or(InitError::WrongArtifact)?;

        if !self.inner.deployed.contains(&artifact) {
            return Err(InitError::NotDeployed);
        }

        if self.inner.initialized.contains_key(&init.instance_id) {
            return Err(InitError::ServiceIdExists);
        }

        let service = {
            let mut service = self.inner.services.get(&artifact).unwrap().new_instance();
            let ctx = TransactionContext::new(context, self);
            service
                .initialize(ctx, init.constructor_data.clone())
                .map_err(|e| InitError::ExecutionError(e))?;
            service
        };

        self.inner.initialized.insert(
            init.instance_id,
            InitializedService::new(init.instance_id, service),
        );

        Ok(())
    }

    fn execute(
        &self,
        context: &mut RuntimeContext,
        dispatch: CallInfo,
        payload: &[u8],
    ) -> Result<(), ExecutionError> {
        let inner = &self.inner;
        let instance = inner.initialized.get(&dispatch.instance_id).unwrap();

        let ctx = TransactionContext::new(context, self);

        instance
            .as_ref()
            .call(dispatch.method_id, ctx, payload)
            .map_err(|e| {
                ExecutionError::with_description(DISPATCH_ERROR, format!("Dispatch error: {}", e))
            })?
    }

    fn state_hashes(&self, snapshot: &dyn Snapshot) -> Vec<(ServiceInstanceId, Vec<Hash>)> {
        let inner = &self.inner;

        inner
            .initialized
            .iter()
            .map(|(_, service)| service.state_hash(snapshot))
            .collect()
    }

    fn before_commit(&self, fork: &mut Fork) {
        let inner = &self.inner;

        for (_, service) in &inner.initialized {
            match panic::catch_unwind(panic::AssertUnwindSafe(|| {
                service.as_ref().before_commit(fork)
            })) {
                Ok(..) => fork.flush(),
                Err(err) => {
                    if err.is::<StorageError>() {
                        // Continue panic unwind if the reason is StorageError.
                        panic::resume_unwind(err);
                    }
                    fork.rollback();

                    // TODO add service name
                    error!("Service before_commit failed with error: {:?}", err);
                }
            }
        }
    }

    fn after_commit(&self, fork: &mut Fork) {
        let inner = &self.inner;

        for (_, service) in &inner.initialized {
            service.as_ref().after_commit(fork);
        }
    }

    fn genesis_init(&self, fork: &mut Fork) -> Result<(), failure::Error> {
        let (deploy_txs, init_txs) = self.inner.services.iter().fold(
            (Vec::new(), Vec::new()),
            |(mut deploy_txs, mut init_txs), (_, s)| {
                let init_info = s.genesis_init_info();
                if !init_info.is_empty() {
                    deploy_txs.push(init_info[0].get_deploy_tx());
                }
                init_txs.extend(init_info.into_iter().map(|i| i.get_init_tx()));
                (deploy_txs, init_txs)
            },
        );

        let mut ctx = RuntimeContext::new(fork, &PublicKey::zero(), &Hash::zero());
        for deploy_tx in deploy_txs.into_iter() {
            self.get_dispatcher()
                .execute(
                    &mut ctx,
                    CallInfo::new(SERVICE_ID, DEPLOY_METHOD_ID),
                    &deploy_tx.encode().unwrap(),
                )
                .map_err(|e| format_err!("Rust runtime genesis deploy error: {:?}", e))?
        }
        for init_tx in init_txs.into_iter() {
            self.get_dispatcher()
                .execute(
                    &mut ctx,
                    CallInfo::new(SERVICE_ID, INIT_METHOD_ID),
                    &init_tx.encode().unwrap(),
                )
                .map_err(|e| format_err!("Rust runtime genesis init error: {:?}", e))?
        }
        Ok(())
    }

    fn get_services_api(&self) -> Vec<(String, ServiceApiBuilder)> {
        self.inner
            .initialized
            .iter()
            .map(|(id, service)| {
                let mut builder = ServiceApiBuilder::new();
                service.as_ref().wire_api(&mut builder);
                (format!("{}", id), builder)
            })
            .collect()
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

    pub fn fork(&self) -> &Fork {
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
