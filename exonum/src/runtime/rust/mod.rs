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

pub use self::{
    error::Error,
    service::{
        AfterCommitContext, Service, ServiceDescriptor, ServiceFactory, Transaction,
        TransactionContext,
    },
};
pub use super::{dispatcher, error::ErrorKind, ArtifactInfo, CallInfo, ServiceInstanceId};

use exonum_merkledb::{Error as StorageError, Fork, Snapshot};
use futures::{future, Future, IntoFuture};
use semver::Version;

use std::{
    collections::{HashMap, HashSet},
    fmt, panic,
    str::FromStr,
};

use crate::{
    api::ServiceApiBuilder,
    crypto::{Hash, PublicKey, SecretKey},
    node::ApiSender,
    proto::Any,
};

use super::{
    dispatcher::DispatcherSender, error::ExecutionError, ArtifactId, Caller, ExecutionContext,
    InstanceSpec, Runtime, RuntimeIdentifier, StateHashAggregator,
};

pub mod error;
#[macro_use]
pub mod service;
#[cfg(test)]
pub mod tests;

#[derive(Debug, Default)]
pub struct RustRuntime {
    available_artifacts: HashMap<RustArtifactId, Box<dyn ServiceFactory>>,
    deployed_artifacts: HashSet<RustArtifactId>,
    started_services: HashMap<ServiceInstanceId, Instance>,
    started_services_by_name: HashMap<String, ServiceInstanceId>,
}

#[derive(Debug)]
struct Instance {
    id: ServiceInstanceId,
    name: String,
    service: Box<dyn Service>,
}

impl Instance {
    pub fn new(id: ServiceInstanceId, name: String, service: Box<dyn Service>) -> Self {
        Self { id, name, service }
    }

    pub fn descriptor(&self) -> ServiceDescriptor {
        ServiceDescriptor::new(self.id, &self.name)
    }

    pub fn state_hash(&self, snapshot: &dyn Snapshot) -> (ServiceInstanceId, Vec<Hash>) {
        (
            self.id,
            self.service.state_hash(self.descriptor(), snapshot),
        )
    }
}

impl AsRef<dyn Service + 'static> for Instance {
    fn as_ref(&self) -> &(dyn Service + 'static) {
        self.service.as_ref()
    }
}

impl AsMut<dyn Service + 'static> for Instance {
    fn as_mut(&mut self) -> &mut (dyn Service + 'static) {
        self.service.as_mut()
    }
}

impl RustRuntime {
    pub const ID: RuntimeIdentifier = RuntimeIdentifier::Rust;

    pub fn new() -> Self {
        Self::default()
    }

    fn parse_artifact(&self, artifact: &ArtifactId) -> Result<RustArtifactId, ExecutionError> {
        if artifact.runtime_id != RuntimeIdentifier::Rust as u32 {
            return Err(Error::IncorrectArtifactId.into());
        }
        artifact
            .name
            .parse()
            .map_err(|inner| (Error::IncorrectArtifactId, inner).into())
    }

    fn add_started_service(&mut self, instance: Instance) {
        self.started_services_by_name
            .insert(instance.name.clone(), instance.id);
        self.started_services.insert(instance.id, instance);
    }

    pub fn add_service_factory(&mut self, service_factory: Box<dyn ServiceFactory>) {
        let artifact = service_factory.artifact_id();
        trace!("Added available artifact {}", artifact);
        self.available_artifacts.insert(artifact, service_factory);
    }

    pub fn with_available_service(
        mut self,
        service_factory: impl Into<Box<dyn ServiceFactory>>,
    ) -> Self {
        self.add_service_factory(service_factory.into());
        self
    }

    fn deploy(&mut self, artifact: ArtifactId) -> Result<(), ExecutionError> {
        let artifact = self.parse_artifact(&artifact)?;

        if self.deployed_artifacts.contains(&artifact) {
            return Err(dispatcher::Error::ArtifactAlreadyDeployed.into());
        }

        if !self.available_artifacts.contains_key(&artifact) {
            return Err(Error::UnableToDeploy.into());
        }

        trace!("Deployed artifact: {}", artifact);
        self.deployed_artifacts.insert(artifact);
        Ok(())
    }
}

impl From<RustRuntime> for (u32, Box<dyn Runtime>) {
    fn from(r: RustRuntime) -> Self {
        (RustRuntime::ID as u32, Box::new(r))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RustArtifactId {
    pub name: String,
    pub version: Version,
}

impl RustArtifactId {
    pub fn new(name: &str, major: u64, minor: u64, patch: u64) -> Self {
        Self {
            name: name.to_owned(),
            version: Version::new(major, minor, patch),
        }
    }
}

impl From<RustArtifactId> for ArtifactId {
    fn from(inner: RustArtifactId) -> Self {
        ArtifactId {
            runtime_id: RustRuntime::ID as u32,
            name: inner.to_string(),
        }
    }
}

impl fmt::Display for RustArtifactId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}/{}", self.name, self.version)
    }
}

impl FromStr for RustArtifactId {
    type Err = failure::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let split = s.split('/').take(2).collect::<Vec<_>>();
        match &split[..] {
            [name, version] => {
                let version = Version::parse(version)?;
                Ok(Self {
                    name: name.to_string(),
                    version,
                })
            },
            _ => Err(failure::format_err!("Wrong rust artifact name format, it should be in form \"artifact_name/artifact_version\""))
        }
    }
}

impl Runtime for RustRuntime {
    fn deploy_artifact(
        &mut self,
        artifact: ArtifactId,
        spec: Any,
    ) -> Box<dyn Future<Item = (), Error = ExecutionError>> {
        if !spec.is_null() && spec != ().into() {
            // Spec for rust artifacts should be empty.
            return Box::new(future::err(Error::IncorrectArtifactId.into()));
        }
        Box::new(self.deploy(artifact).into_future())
    }

    fn artifact_info(&self, id: &ArtifactId) -> Option<ArtifactInfo> {
        let id = self.parse_artifact(id).ok()?;

        if !self.deployed_artifacts.contains(&id) {
            None
        } else {
            self.available_artifacts
                .get(&id)
                .map(|service_factory| service_factory.artifact_info())
        }
    }

    fn start_service(&mut self, spec: &InstanceSpec) -> Result<(), ExecutionError> {
        let artifact = self.parse_artifact(&spec.artifact)?;

        trace!("New service {} instance with id: {}", artifact, spec.id);

        // Implement ensure like macro to reduce amount of boiler-plate code. [ECR-3222]

        if !self.deployed_artifacts.contains(&artifact) {
            return Err(dispatcher::Error::ArtifactNotDeployed.into());
        }
        if self.started_services.contains_key(&spec.id) {
            return Err(dispatcher::Error::ServiceIdExists.into());
        }
        if self.started_services_by_name.contains_key(&spec.name) {
            return Err(dispatcher::Error::ServiceNameExists.into());
        }

        let service = self
            .available_artifacts
            .get(&artifact)
            .unwrap()
            .create_instance();
        self.add_started_service(Instance::new(spec.id, spec.name.clone(), service));
        Ok(())
    }

    fn configure_service(
        &self,
        fork: &Fork,
        spec: &InstanceSpec,
        parameters: Any,
    ) -> Result<(), ExecutionError> {
        let artifact = self.parse_artifact(&spec.artifact)?;

        trace!(
            "Configure service {} instance with id: {}",
            artifact,
            spec.id
        );

        let service_instance = self
            .started_services
            .get(&spec.id)
            .ok_or(dispatcher::Error::ServiceNotStarted)?;
        service_instance
            .as_ref()
            .configure(service_instance.descriptor(), fork, parameters)
    }

    fn stop_service(&mut self, spec: &InstanceSpec) -> Result<(), ExecutionError> {
        let artifact = self.parse_artifact(&spec.artifact)?;

        trace!("Stop service {} instance with id: {}", artifact, spec.id);

        self.started_services
            .remove(&spec.id)
            .ok_or(dispatcher::Error::ServiceNotStarted)
            .map(drop)
            .map_err(From::from)
    }

    fn execute(
        &self,
        dispatcher: &super::dispatcher::Dispatcher,
        runtime_context: &mut ExecutionContext,
        call_info: CallInfo,
        payload: &[u8],
    ) -> Result<(), ExecutionError> {
        let service_instance = self.started_services.get(&call_info.instance_id).unwrap();
        service_instance
            .as_ref()
            .call(
                call_info.method_id,
                TransactionContext {
                    service_descriptor: service_instance.descriptor(),
                    runtime_context,
                    dispatcher,
                },
                payload,
            )
            .map_err(|e| (Error::UnspecifiedError, e))?
    }

    fn state_hashes(&self, snapshot: &dyn Snapshot) -> StateHashAggregator {
        StateHashAggregator {
            runtime: Vec::new(),
            instances: self
                .started_services
                .values()
                .map(|service| service.state_hash(snapshot))
                .collect(),
        }
    }

    fn before_commit(&self, dispatcher: &super::dispatcher::Dispatcher, fork: &mut Fork) {
        for service in self.started_services.values() {
            match panic::catch_unwind(panic::AssertUnwindSafe(|| {
                service.as_ref().before_commit(TransactionContext {
                    dispatcher,
                    runtime_context: &mut ExecutionContext::new(fork, Caller::Blockchain),
                    service_descriptor: service.descriptor(),
                })
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

    fn after_commit(
        &self,
        dispatcher: &DispatcherSender,
        snapshot: &dyn Snapshot,
        service_keypair: &(PublicKey, SecretKey),
        tx_sender: &ApiSender,
    ) {
        for service in self.started_services.values() {
            service.as_ref().after_commit(AfterCommitContext::new(
                dispatcher,
                service.descriptor(),
                snapshot,
                service_keypair,
                tx_sender,
            ));
        }
    }

    fn services_api(&self) -> Vec<(String, ServiceApiBuilder)> {
        self.started_services
            .values()
            .map(|service_instance| {
                let mut builder = ServiceApiBuilder::new();
                service_instance
                    .as_ref()
                    .wire_api(service_instance.descriptor(), &mut builder);
                (service_instance.name.clone(), builder)
            })
            .collect()
    }
}

#[test]
fn parse_rust_artifact_id_correct() {
    RustArtifactId::from_str("my-service/1.0.0").unwrap();
}
