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

//! Built-in Rust runtime module.

pub use self::{
    call_context::{CallContext, SupervisorExtensions},
    error::Error,
    service::{
        AfterCommitContext, Interface, Service, ServiceDispatcher, ServiceFactory, Transaction,
    },
};

pub mod error;

use exonum_merkledb::{Fork, Snapshot};
use futures::{future, Future, IntoFuture};
use semver::Version;

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt,
    str::FromStr,
    sync::{Arc, RwLock},
};

use crate::{
    crypto::{Hash, PublicKey, SecretKey},
    node::ApiSender,
};

use super::{
    api::{ApiContext, ServiceApiBuilder},
    dispatcher::{self, Dispatcher},
    error::{catch_panic, ExecutionError},
    ArtifactId, ArtifactProtobufSpec, CallInfo, ExecutionContext, InstanceDescriptor, InstanceId,
    InstanceSpec, Runtime, RuntimeIdentifier, StateHashAggregator,
};

mod call_context;
mod service;
#[cfg(test)]
mod tests;

#[derive(Debug, Default)]
pub struct RustRuntime {
    inner: RwLock<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    available_artifacts: HashMap<RustArtifactId, Box<dyn ServiceFactory>>,
    deployed_artifacts: HashSet<RustArtifactId>,
    started_services: BTreeMap<InstanceId, Instance>,
    started_services_by_name: HashMap<String, InstanceId>,
}

#[derive(Debug, Clone)]
struct Instance {
    id: InstanceId,
    name: String,
    service: Arc<dyn Service>,
}

impl Instance {
    pub fn new(id: InstanceId, name: String, service: Box<dyn Service>) -> Self {
        Self {
            id,
            name,
            service: Arc::from(service),
        }
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

impl RustRuntime {
    /// Rust runtime identifier.
    pub const ID: RuntimeIdentifier = RuntimeIdentifier::Rust;

    /// Creates a new Rust runtime instance.
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_service_factory(&mut self, service_factory: Box<dyn ServiceFactory>) {
        let artifact = service_factory.artifact_id();
        trace!("Added available artifact {}", artifact);
        self.inner
            .write()
            .unwrap()
            .available_artifacts
            .insert(artifact, service_factory);
    }

    pub fn with_available_service(
        mut self,
        service_factory: impl Into<Box<dyn ServiceFactory>>,
    ) -> Self {
        self.add_service_factory(service_factory.into());
        self
    }
}

impl Inner {
    fn parse_artifact(artifact: &ArtifactId) -> Result<RustArtifactId, ExecutionError> {
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

    fn remove_started_instance(&mut self, instance_id: InstanceId) -> Option<Instance> {
        self.started_services
            .remove(&instance_id)
            .and_then(|instance| {
                self.started_services_by_name.remove(&instance.name);
                Some(instance)
            })
    }

    fn deploy(&mut self, artifact: &ArtifactId) -> Result<(), ExecutionError> {
        let artifact = Self::parse_artifact(&artifact)?;
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

    fn start_service(&mut self, spec: &InstanceSpec) -> Result<Instance, ExecutionError> {
        let artifact = Self::parse_artifact(&spec.artifact)?;
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
        Ok(self.started_services[&spec.id].clone())
    }

    fn deployed_artifact(&self, id: &RustArtifactId) -> Option<&dyn ServiceFactory> {
        if self.deployed_artifacts.contains(&id) {
            self.available_artifacts.get(&id).map(AsRef::as_ref)
        } else {
            None
        }
    }
}

impl From<RustRuntime> for (u32, Arc<dyn Runtime>) {
    fn from(r: RustRuntime) -> Self {
        (RustRuntime::ID as u32, Arc::new(r))
    }
}

impl From<RustRuntime> for Arc<dyn Runtime> {
    fn from(value: RustRuntime) -> Self {
        Arc::new(value)
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
        &self,
        artifact: ArtifactId,
        spec: Vec<u8>,
    ) -> Box<dyn Future<Item = (), Error = ExecutionError>> {
        if !spec.is_empty() {
            // Keep the spec for Rust artifacts empty.
            return Box::new(future::err(Error::IncorrectArtifactId.into()));
        }
        let mut inner = self.inner.write().unwrap();
        Box::new(inner.deploy(&artifact).into_future())
    }

    fn is_artifact_deployed(&self, id: &ArtifactId) -> bool {
        if let Ok(artifact) = Inner::parse_artifact(id) {
            let inner = self.inner.read().unwrap();
            inner.deployed_artifacts.contains(&artifact)
        } else {
            false
        }
    }

    fn artifact_protobuf_spec(&self, id: &ArtifactId) -> Option<ArtifactProtobufSpec> {
        let id = Inner::parse_artifact(id).ok()?;
        let inner = self.inner.read().unwrap();
        inner
            .deployed_artifact(&id)
            .map(ServiceFactory::artifact_protobuf_spec)
    }

    fn restart_service(&self, spec: &InstanceSpec) -> Result<(), ExecutionError> {
        let mut inner = self.inner.write().unwrap();
        inner.start_service(spec)?;
        trace!("Started service {}", spec);
        Ok(())
    }

    fn add_service(
        &self,
        fork: &Fork,
        spec: &InstanceSpec,
        parameters: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        let instance = self.inner.write().unwrap().start_service(spec)?;
        let service = instance.as_ref();
        let descriptor = instance.descriptor();
        let result = catch_panic(|| service.initialize(descriptor, fork, parameters));

        match result {
            Ok(()) => {
                trace!("Added service instance {}", spec);
            }
            Err(_) => {
                let mut inner = self.inner.write().unwrap();
                inner.remove_started_instance(spec.id);
            }
        }
        result
    }

    fn execute(
        &self,
        context: ExecutionContext,
        call_info: &CallInfo,
        payload: &[u8],
    ) -> Result<(), ExecutionError> {
        let instance = self
            .inner
            .read()
            .unwrap()
            .started_services
            .get(&call_info.instance_id)
            .expect("BUG: an attempt to execute transaction of unknown service.")
            .clone();

        instance.as_ref().call(
            context.interface_name,
            call_info.method_id,
            CallContext::new(context, instance.id),
            payload,
        )
    }

    fn state_hashes(&self, snapshot: &dyn Snapshot) -> StateHashAggregator {
        StateHashAggregator {
            runtime: Vec::new(),
            instances: self
                .inner
                .read()
                .unwrap()
                .started_services
                .values()
                .map(|service| service.state_hash(snapshot))
                .collect(),
        }
    }

    fn before_commit(
        &self,
        context: ExecutionContext,
        instance_id: InstanceId,
    ) -> Result<(), ExecutionError> {
        // We avoid a potential deadlock by cloning instances (i.e., copying them out
        // of the locked memory). Thus, we don't need to hold the lock for the duration
        // of the cycle below.
        let instance = self
            .inner
            .read()
            .unwrap()
            .started_services
            .get(&instance_id)
            .cloned()
            .expect("`before_commit` called with non-existing `instance_id`");

        let result = catch_panic(|| {
            let context = CallContext::new(context, instance.id);
            instance.as_ref().before_commit(context);
            Ok(())
        });
        if let Err(ref e) = result {
            error!(
                "Service \"{}\" `before_commit` failed with error: {:?}",
                instance.name, e
            );
        }
        result
    }

    fn after_commit(
        &self,
        dispatcher: &mut Dispatcher,
        snapshot: &dyn Snapshot,
        service_keypair: &(PublicKey, SecretKey),
        tx_sender: &ApiSender,
    ) {
        let instances = self.inner.read().unwrap().started_services.clone();
        for service in instances.values() {
            service.as_ref().after_commit(AfterCommitContext::new(
                dispatcher,
                service.descriptor(),
                snapshot,
                service_keypair,
                tx_sender,
            ));
        }
    }

    fn api_endpoints(&self, context: &ApiContext) -> Vec<(String, ServiceApiBuilder)> {
        self.inner
            .read()
            .unwrap()
            .started_services
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
