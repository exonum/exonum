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

//! The runtime for the native Rust services.
//!
//! A set of artifacts available to deploy in the Rust runtime is static and defined at the compile
//! time. Once created, it can be changed only by the node binary recompilation.
//!
//! Beware of the removing the artifacts from the Rust runtime, since attempt to remove an artifact
//! with already running instances can cause blockchain to break. The only safe change that can be
//! performed here is adding new artifacts.
//!
//! Rust runtime does not provide any level of service isolation from the operation system,
//! therefore security audit of the artifacts to be deployed is up to the node administrators.
//!
//! The artifact interface in the Rust runtime is represented by the
//! [`ServiceFactory`][ServiceFactory] trait, which creates service instances and provides
//! information about the artifact.
//!
//! [ServiceFactory]: trait.ServiceFactory.html

pub use self::{
    call_context::CallContext,
    error::Error,
    service::{
        AfterCommitContext, Interface, Service, ServiceDispatcher, ServiceFactory, Transaction,
    },
};

pub mod error;

use exonum_merkledb::{is_valid_index_name, Snapshot};
use futures::{future, sync::mpsc, Future, IntoFuture, Sink};
use semver::Version;

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt,
    str::FromStr,
};

use crate::{
    api::{manager::UpdateEndpoints, ApiBuilder},
    blockchain::{Blockchain, Schema as CoreSchema},
    crypto::Hash,
    helpers::Height,
};

use super::{
    api::{ApiContext, ServiceApiBuilder},
    dispatcher::{self, Mailbox},
    error::{catch_panic, ExecutionError},
    ArtifactId, ArtifactProtobufSpec, CallInfo, ExecutionContext, InstanceDescriptor, InstanceId,
    InstanceSpec, Runtime, RuntimeIdentifier, StateHashAggregator,
};

mod call_context;
mod service;
#[cfg(test)]
mod tests;

/// Rust runtime entity.
///
/// [Detailed description of the Rust runtime](index.html).
#[derive(Debug)]
pub struct RustRuntime {
    api_context: Option<ApiContext>,
    api_notifier: mpsc::Sender<UpdateEndpoints>,
    available_artifacts: HashMap<RustArtifactId, Box<dyn ServiceFactory>>,
    deployed_artifacts: HashSet<RustArtifactId>,
    started_services: BTreeMap<InstanceId, Instance>,
    started_services_by_name: HashMap<String, InstanceId>,
    new_services_since_last_block: bool,
}

#[derive(Debug)]
struct Instance {
    id: InstanceId,
    name: String,
    service: Box<dyn Service>,
}

impl Instance {
    fn new(id: InstanceId, name: String, service: Box<dyn Service>) -> Self {
        Self { id, name, service }
    }

    fn descriptor(&self) -> InstanceDescriptor<'_> {
        InstanceDescriptor {
            id: self.id,
            name: &self.name,
        }
    }

    fn state_hash(&self, snapshot: &dyn Snapshot) -> (InstanceId, Vec<Hash>) {
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
    pub fn new(api_notifier: mpsc::Sender<UpdateEndpoints>) -> Self {
        Self {
            api_context: None,
            api_notifier,
            available_artifacts: Default::default(),
            deployed_artifacts: Default::default(),
            started_services: Default::default(),
            started_services_by_name: Default::default(),
            new_services_since_last_block: false,
        }
    }

    fn api_context(&self) -> &ApiContext {
        self.api_context
            .as_ref()
            .expect("Method called before Rust runtime is initialized")
    }

    /// Adds a new service factory to the list of available artifacts.
    pub fn add_service_factory(&mut self, service_factory: Box<dyn ServiceFactory>) {
        let artifact = service_factory.artifact_id();
        trace!("Added available artifact {}", artifact);
        self.available_artifacts.insert(artifact, service_factory);
    }

    /// Adds a new service factory to the list of available artifacts and returns
    /// modified `RustRuntime` object for further chaining.
    pub fn with_available_service(
        mut self,
        service_factory: impl Into<Box<dyn ServiceFactory>>,
    ) -> Self {
        self.add_service_factory(service_factory.into());
        self
    }

    fn add_started_service(&mut self, instance: Instance) {
        self.started_services_by_name
            .insert(instance.name.clone(), instance.id);
        self.started_services.insert(instance.id, instance);
    }

    fn deploy(&mut self, artifact: &ArtifactId) -> Result<(), ExecutionError> {
        let artifact = RustArtifactId::parse(&artifact)?;
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

    fn new_service(&self, spec: &InstanceSpec) -> Result<Instance, ExecutionError> {
        let artifact = RustArtifactId::parse(&spec.artifact)?;
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
        Ok(Instance::new(spec.id, spec.name.clone(), service))
    }

    fn deployed_artifact(&self, id: &RustArtifactId) -> Option<&dyn ServiceFactory> {
        if self.deployed_artifacts.contains(&id) {
            self.available_artifacts.get(&id).map(AsRef::as_ref)
        } else {
            None
        }
    }

    fn api_endpoints(&self) -> Vec<(String, ApiBuilder)> {
        self.started_services
            .values()
            .map(|instance| {
                let mut builder = ServiceApiBuilder::new(
                    self.api_context().clone(),
                    InstanceDescriptor {
                        id: instance.id,
                        name: instance.name.as_ref(),
                    },
                );
                instance.as_ref().wire_api(&mut builder);
                (
                    ["services/", &instance.name].concat(),
                    ApiBuilder::from(builder),
                )
            })
            .collect()
    }

    fn push_api_changes(&mut self) {
        if self.new_services_since_last_block {
            let user_endpoints = self.api_endpoints();
            // FIXME: this should either be made async, or an unbounded channel should be used.
            if !self.api_notifier.is_closed() {
                self.api_notifier
                    .clone()
                    .send(UpdateEndpoints { user_endpoints })
                    .wait()
                    .ok();
            }
        }
        self.new_services_since_last_block = false;
    }
}

impl From<RustRuntime> for (u32, Box<dyn Runtime>) {
    fn from(r: RustRuntime) -> Self {
        (RustRuntime::ID as u32, Box::new(r))
    }
}

/// The unique identifier of the Rust artifact, containing the name and version of the artifact.
///
/// As a string, the artifact name is represented as follows:
///
/// `{artifact_name}:{artifact_version}`, where `artifact_name` is a unique name of the artifact,
/// and `artifact_version` is a semantic version identifier.
///
/// * Artifact name can contain only the following characters: `a-zA-Z0-9` and one of `_-.`.
/// * Artifact version identifier must conform to the semantic version scheme (major.minor.patch).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RustArtifactId {
    /// Artifact name.
    pub name: String,
    /// Artifact version identifier conforming to the semantic versioning scheme.
    pub version: Version,
}

impl RustArtifactId {
    /// Creates a new Rust artifact ID from the provided name and version.
    ///
    /// # Panics
    ///
    /// If the `name` is empty or contains illegal character.
    pub fn new(name: &str, major: u64, minor: u64, patch: u64) -> Self {
        Self::is_valid_name(name).expect("Invalid Rust artifact name.");
        Self {
            name: name.to_owned(),
            version: Version::new(major, minor, patch),
        }
    }

    /// Checks that the Rust artifact name contains only allowed characters and is not empty.
    fn is_valid_name(name: impl AsRef<str>) -> Result<(), failure::Error> {
        let name = name.as_ref();
        ensure!(!name.is_empty(), "Rust artifact name should not be empty.");
        ensure!(
            is_valid_index_name(name),
            "Rust artifact name contains illegal character, use only: a-zA-Z0-9 and one of _-."
        );
        Ok(())
    }

    fn parse(artifact: &ArtifactId) -> Result<Self, ExecutionError> {
        if artifact.runtime_id != RuntimeIdentifier::Rust as u32 {
            return Err(Error::IncorrectArtifactId.into());
        }
        artifact
            .name
            .parse()
            .map_err(|inner| (Error::IncorrectArtifactId, inner).into())
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.name, self.version)
    }
}

impl FromStr for RustArtifactId {
    type Err = failure::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let split = s.split(':').take(2).collect::<Vec<_>>();
        match &split[..] {
            [name, version] => {
                Self::is_valid_name(name)?;
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
    fn initialize(&mut self, blockchain: &Blockchain) {
        self.api_context = Some(ApiContext::with_blockchain(blockchain));
    }

    // We need to propagate changes in the services immediately after initialization.
    fn on_resume(&mut self) {
        self.push_api_changes();
    }

    fn deploy_artifact(
        &mut self,
        artifact: ArtifactId,
        spec: Vec<u8>,
    ) -> Box<dyn Future<Item = ArtifactProtobufSpec, Error = ExecutionError>> {
        if !spec.is_empty() {
            // Keep the spec for Rust artifacts empty.
            Box::new(future::err(Error::IncorrectArtifactId.into()))
        } else {
            let res = self.deploy(&artifact).and_then(|()| {
                let id = RustArtifactId::parse(&artifact)?;
                self.deployed_artifact(&id)
                    .map(ServiceFactory::artifact_protobuf_spec)
                    .ok_or_else(|| Error::UnableToDeploy.into())
            });
            Box::new(res.into_future())
        }
    }

    fn is_artifact_deployed(&self, id: &ArtifactId) -> bool {
        if let Ok(artifact) = RustArtifactId::parse(id) {
            self.deployed_artifacts.contains(&artifact)
        } else {
            false
        }
    }

    fn start_adding_service(
        &self,
        context: ExecutionContext<'_>,
        spec: &InstanceSpec,
        parameters: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        let instance = self.new_service(spec)?;
        let service = instance.as_ref();
        let descriptor = instance.descriptor();
        let context = CallContext::new(context, descriptor);
        catch_panic(|| service.initialize(context, parameters))
    }

    fn commit_service(
        &mut self,
        _snapshot: &dyn Snapshot,
        spec: &InstanceSpec,
    ) -> Result<(), ExecutionError> {
        let instance = self.new_service(spec)?;
        self.add_started_service(instance);
        self.new_services_since_last_block = true;
        Ok(())
    }

    fn execute(
        &self,
        context: ExecutionContext<'_>,
        call_info: &CallInfo,
        payload: &[u8],
    ) -> Result<(), ExecutionError> {
        let instance = self
            .started_services
            .get(&call_info.instance_id)
            .expect("BUG: an attempt to execute transaction of unknown service.");

        let descriptor = instance.descriptor();
        instance.as_ref().call(
            context.interface_name,
            call_info.method_id,
            CallContext::new(context, descriptor),
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

    fn before_commit(
        &self,
        context: ExecutionContext<'_>,
        instance_id: InstanceId,
    ) -> Result<(), ExecutionError> {
        // We avoid a potential deadlock by cloning instances (i.e., copying them out
        // of the locked memory). Thus, we don't need to hold the lock for the duration
        // of the cycle below.
        let instance = self
            .started_services
            .get(&instance_id)
            .expect("`before_commit` called with non-existing `instance_id`");

        let descriptor = instance.descriptor();
        let result = catch_panic(|| {
            let context = CallContext::new(context, descriptor);
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

    fn after_commit(&mut self, snapshot: &dyn Snapshot, mailbox: &mut Mailbox) {
        self.push_api_changes();

        // By convention, services don't handle `after_commit()` on the genesis block.
        if CoreSchema::new(snapshot).height() == Height(0) {
            return;
        }

        let api_context = self.api_context();
        for service in self.started_services.values() {
            service.as_ref().after_commit(AfterCommitContext::new(
                mailbox,
                service.descriptor(),
                snapshot,
                api_context.service_keypair(),
                api_context.sender(),
            ));
        }
    }
}

#[test]
fn parse_rust_artifact_id_correct() {
    RustArtifactId::from_str("my-service:1.0.0").unwrap();
}

#[test]
fn parse_rust_artifact_id_incorrect() {
    let cases = vec![
        ("my-service:1.1.1.1.1", "Extra junk after valid version"),
        (":1.0", "Rust artifact name should not be empty"),
        ("name:", "Error parsing major identifier"),
        ("$name:1.0", "Rust artifact name contains illegal character"),
        ("aAa", "Wrong rust artifact name format"),
    ];

    for (artifact_str, expected_err) in cases {
        let actual_err = RustArtifactId::from_str(artifact_str)
            .unwrap_err()
            .to_string();
        assert!(
            actual_err.contains(expected_err),
            "Actual error is: \"{}\", but expected \"{}\"",
            actual_err,
            expected_err
        )
    }
}
