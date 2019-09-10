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
    interfaces::Initialize,
    service::{
        AfterCommitContext, BeforeCommitContext, Interface, Service, ServiceDispatcher,
        ServiceFactory, Transaction, TransactionContext,
    },
};

pub mod error;
pub mod interfaces;

use exonum_merkledb::{Fork, Snapshot};
use futures::{future, Future, IntoFuture};
use semver::Version;

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt,
    str::FromStr,
};

use crate::{
    crypto::{Hash, PublicKey, SecretKey},
    node::ApiSender,
    proto::Any,
};

use super::{
    api::{ApiContext, ServiceApiBuilder},
    dispatcher::{self, Dispatcher, DispatcherSender},
    error::{catch_panic, ExecutionError},
    ArtifactId, ArtifactProtobufSpec, CallInfo, ExecutionContext, InstanceDescriptor, InstanceId,
    InstanceSpec, Runtime, RuntimeIdentifier, StateHashAggregator,
};

mod service;
#[cfg(test)]
mod tests;

#[derive(Debug, Default)]
pub struct RustRuntime {
    available_artifacts: HashMap<RustArtifactId, Box<dyn ServiceFactory>>,
    deployed_artifacts: HashSet<RustArtifactId>,
    started_services: BTreeMap<InstanceId, Instance>,
    started_services_by_name: HashMap<String, InstanceId>,
}

#[derive(Debug)]
struct Instance {
    id: InstanceId,
    name: String,
    service: Box<dyn Service>,
}

impl Instance {
    pub fn new(id: InstanceId, name: String, service: Box<dyn Service>) -> Self {
        Self { id, name, service }
    }

    pub fn descriptor(&self) -> InstanceDescriptor {
        InstanceDescriptor {
            id: self.id,
            name: &self.name,
        }
    }

    pub fn state_hash(&self, snapshot: &dyn Snapshot) -> (InstanceId, Vec<Hash>) {
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

    fn deploy(&mut self, artifact: &ArtifactId) -> Result<(), ExecutionError> {
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

    fn deployed_artifact(&self, id: &RustArtifactId) -> Option<&dyn ServiceFactory> {
        if self.deployed_artifacts.contains(&id) {
            self.available_artifacts.get(&id).map(AsRef::as_ref)
        } else {
            None
        }
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
        Self {
            runtime_id: RustRuntime::ID as u32,
            name: inner.to_string(),
        }
    }
}

impl fmt::Display for RustArtifactId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.name, self.version)
    }
}

impl FromStr for RustArtifactId {
    type Err = failure::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let split = s.split(':').take(2).collect::<Vec<_>>();
        match &split[..] {
            [name, version] => {
                let version = Version::parse(version)?;
                Ok(Self {
                    name: name.to_string(),
                    version,
                })
            },
            _ => Err(failure::format_err!("Wrong rust artifact name format, it should be in form \"artifact_name:artifact_version\""))
        }
    }
}

impl Runtime for RustRuntime {
    fn deploy_artifact(
        &mut self,
        artifact: ArtifactId,
        spec: Any,
    ) -> Box<dyn Future<Item = (), Error = ExecutionError>> {
        if !spec.is_empty() {
            // Keep the spec for Rust artifacts empty.
            return Box::new(future::err(Error::IncorrectArtifactId.into()));
        }
        Box::new(self.deploy(&artifact).into_future())
    }

    fn is_artifact_deployed(&self, id: &ArtifactId) -> bool {
        if let Ok(artifact) = self.parse_artifact(id) {
            self.deployed_artifacts.contains(&artifact)
        } else {
            false
        }
    }

    fn artifact_protobuf_spec(&self, id: &ArtifactId) -> Option<ArtifactProtobufSpec> {
        let id = self.parse_artifact(id).ok()?;
        self.deployed_artifact(&id)
            .map(ServiceFactory::artifact_protobuf_spec)
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

        let service = self.available_artifacts[&artifact].create_instance();
        self.add_started_service(Instance::new(spec.id, spec.name.clone(), service));
        Ok(())
    }

    fn stop_service(&mut self, descriptor: InstanceDescriptor) -> Result<(), ExecutionError> {
        trace!("Stop service instance {}", descriptor);

        self.started_services
            .remove(&descriptor.id)
            .ok_or(dispatcher::Error::ServiceNotStarted)
            .map(drop)
            .map_err(From::from)
    }

    fn execute(
        &self,
        context: &ExecutionContext,
        call_info: &CallInfo,
        payload: &[u8],
    ) -> Result<(), ExecutionError> {
        let instance = self
            .started_services
            .get(&call_info.instance_id)
            .expect("BUG: an attempt to execute transaction of unknown service.");

        instance.as_ref().call(
            context.interface_name,
            call_info.method_id,
            TransactionContext::new(context, instance.descriptor()),
            payload,
        )
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

    fn before_commit(&self, dispatcher: &Dispatcher, fork: &mut Fork) {
        for instance in self.started_services.values() {
            let result = catch_panic(|| {
                instance.as_ref().before_commit(BeforeCommitContext::new(
                    instance.descriptor(),
                    fork,
                    dispatcher,
                ));
                Ok(())
            });

            match result {
                Ok(..) => fork.flush(),
                Err(e) => {
                    fork.rollback();
                    error!(
                        "Service \"{}\" `before_commit` failed with error: {:?}",
                        instance.name, e
                    );
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
                service.descriptor(),
                snapshot,
                dispatcher,
                service_keypair,
                tx_sender,
            ));
        }
    }

    fn api_endpoints(&self, context: &ApiContext) -> Vec<(String, ServiceApiBuilder)> {
        self.started_services
            .values()
            .map(|instance| {
                let mut builder = ServiceApiBuilder::new(
                    context.clone(),
                    InstanceDescriptor {
                        id: instance.id,
                        name: instance.name.as_ref(),
                    },
                );
                instance.as_ref().wire_api(&mut builder);
                (["services/", &instance.name].concat(), builder)
            })
            .collect()
    }
}

#[test]
fn parse_rust_artifact_id_correct() {
    RustArtifactId::from_str("my-service:1.0.0").unwrap();
}
