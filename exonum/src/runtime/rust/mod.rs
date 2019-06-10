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

pub use self::service::{
    AfterCommitContext, Service, ServiceDescriptor, ServiceFactory, Transaction,
};
pub use crate::messages::ServiceInstanceId;

use exonum_merkledb::{BinaryValue, Error as StorageError, Fork, Snapshot};
use protobuf::well_known_types::Any;
use semver::Version;

use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    fmt, panic,
    str::FromStr,
};

use crate::{
    api::ServiceApiBuilder,
    crypto::{Hash, PublicKey, SecretKey},
    messages::CallInfo,
    node::ApiSender,
    proto::schema,
};

use super::{
    dispatcher,
    error::{DeployError, ExecutionError, StartError, DISPATCH_ERROR},
    ArtifactSpec, DeployStatus, Runtime, RuntimeContext, RuntimeIdentifier, ServiceConstructor,
    ServiceInstanceSpec,
};

#[macro_use]
pub mod service;
#[cfg(test)]
pub mod tests;

#[derive(Debug, Default)]
pub struct RustRuntime {
    available_artifacts: HashMap<RustArtifactSpec, Box<dyn ServiceFactory>>,
    deployed_artifacts: HashSet<RustArtifactSpec>,
    started_services: HashMap<ServiceInstanceId, ServiceInstance>,
    started_services_by_name: HashMap<String, ServiceInstanceId>,
}

#[derive(Debug)]
struct ServiceInstance {
    id: ServiceInstanceId,
    name: String,
    service: Box<dyn Service>,
}

impl ServiceInstance {
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

impl AsRef<dyn Service + 'static> for ServiceInstance {
    fn as_ref(&self) -> &(dyn Service + 'static) {
        self.service.as_ref()
    }
}

impl AsMut<dyn Service + 'static> for ServiceInstance {
    fn as_mut(&mut self) -> &mut (dyn Service + 'static) {
        self.service.as_mut()
    }
}

impl RustRuntime {
    pub const ID: RuntimeIdentifier = RuntimeIdentifier::Rust;

    pub fn new() -> Self {
        Self::default()
    }

    fn parse_artifact(&self, artifact: &ArtifactSpec) -> Option<RustArtifactSpec> {
        if artifact.runtime_id != RuntimeIdentifier::Rust as u32 {
            return None;
        }

        let rust_artifact_spec: RustArtifactSpec =
            BinaryValue::from_bytes(Cow::Borrowed(&artifact.raw_spec)).ok()?;

        Some(rust_artifact_spec)
    }

    fn add_started_service(&mut self, descriptor: ServiceInstance) {
        self.started_services_by_name
            .insert(descriptor.name.clone(), descriptor.id);
        self.started_services.insert(descriptor.id, descriptor);
    }

    pub fn add_service_factory(&mut self, service_factory: Box<dyn ServiceFactory>) {
        let artifact = service_factory.artifact();
        info!("Added available artifact {}", artifact);
        self.available_artifacts.insert(artifact, service_factory);
    }
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
        any.set_value(self.to_bytes());
        any
    }
}

impl fmt::Display for RustArtifactSpec {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}/{}", self.name, self.version)
    }
}

impl FromStr for RustArtifactSpec {
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
            _ => Err(failure::format_err!("Wrong artifact spec format, in should be in form \"artifact_name/artifact_version\""))
        }
    }
}

impl Runtime for RustRuntime {
    fn begin_deploy(&mut self, artifact: &ArtifactSpec) -> Result<(), DeployError> {
        let artifact = self
            .parse_artifact(&artifact)
            .ok_or(DeployError::WrongArtifact)?;

        trace!("Begin deploy artifact: {}", artifact);

        if !self.available_artifacts.contains_key(&artifact) {
            return Err(DeployError::FailedToDeploy);
        }
        if !self.deployed_artifacts.insert(artifact) {
            return Err(DeployError::AlreadyDeployed);
        }

        Ok(())
    }

    fn check_deploy_status(
        &self,
        artifact: &ArtifactSpec,
        _cancel_if_not_complete: bool,
    ) -> Result<DeployStatus, DeployError> {
        let artifact = self
            .parse_artifact(&artifact)
            .ok_or(DeployError::WrongArtifact)?;

        if self.deployed_artifacts.contains(&artifact) {
            Ok(DeployStatus::Deployed)
        } else {
            Err(DeployError::FailedToDeploy)
        }
    }

    fn start_service(&mut self, spec: &ServiceInstanceSpec) -> Result<(), StartError> {
        let artifact = self
            .parse_artifact(&spec.artifact)
            .ok_or(StartError::WrongArtifact)?;

        trace!("New service {} instance with id: {}", artifact, spec.id);

        // Implement ensure like macro to reduce amount of boiler-plate code. [ECR-3222]

        if !self.deployed_artifacts.contains(&artifact) {
            return Err(StartError::NotDeployed);
        }

        if self.started_services.contains_key(&spec.id) {
            return Err(StartError::ServiceIdExists);
        }

        if self.started_services_by_name.contains_key(&spec.name) {
            return Err(StartError::ServiceNameExists);
        }

        let service = self
            .available_artifacts
            .get(&artifact)
            .unwrap()
            .new_instance();
        self.add_started_service(ServiceInstance::new(spec.id, spec.name.clone(), service));
        Ok(())
    }

    fn configure_service(
        &self,
        runtime_context: &mut RuntimeContext,
        spec: &ServiceInstanceSpec,
        parameters: &ServiceConstructor,
    ) -> Result<(), StartError> {
        let artifact = self
            .parse_artifact(&spec.artifact)
            .ok_or(StartError::WrongArtifact)?;

        trace!(
            "Configure service {} instance with id: {}",
            artifact,
            spec.id
        );

        let service_instance = self
            .started_services
            .get(&spec.id)
            .ok_or(StartError::NotStarted)?;
        service_instance
            .as_ref()
            .configure(
                TransactionContext {
                    service_descriptor: service_instance.descriptor(),
                    runtime_context,
                    runtime: self,
                },
                &parameters.data,
            )
            .map_err(|e| StartError::ExecutionError(e))
    }

    fn stop_service(&mut self, spec: &ServiceInstanceSpec) -> Result<(), StartError> {
        let artifact = self
            .parse_artifact(&spec.artifact)
            .ok_or(StartError::WrongArtifact)?;

        trace!("Stop service {} instance with id: {}", artifact, spec.id);

        self.started_services
            .remove(&spec.id)
            .ok_or(StartError::NotStarted)
            .map(drop)
    }

    fn execute(
        &self,
        runtime_context: &mut RuntimeContext,
        call_info: CallInfo,
        payload: &[u8],
    ) -> Result<(), ExecutionError> {
        debug!("Execute: {:?}", call_info);
        let service_instance = self.started_services.get(&call_info.instance_id).unwrap();

        let context = TransactionContext {
            service_descriptor: service_instance.descriptor(),
            runtime_context,
            runtime: self,
        };

        service_instance
            .as_ref()
            .call(call_info.method_id, context, payload)
            .map_err(|e| {
                ExecutionError::with_description(DISPATCH_ERROR, format!("Dispatch error: {}", e))
            })?
    }

    fn state_hashes(&self, snapshot: &dyn Snapshot) -> Vec<(ServiceInstanceId, Vec<Hash>)> {
        self.started_services
            .iter()
            .map(|(_, service)| service.state_hash(snapshot))
            .collect()
    }

    fn before_commit(&self, fork: &mut Fork) {
        for service in self.started_services.values() {
            match panic::catch_unwind(panic::AssertUnwindSafe(|| {
                service.as_ref().before_commit(TransactionContext {
                    runtime: self,
                    runtime_context: &mut RuntimeContext::without_author(fork),
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
        snapshot: &dyn Snapshot,
        service_keypair: &(PublicKey, SecretKey),
        tx_sender: &ApiSender,
    ) {
        for service in self.started_services.values() {
            let context =
                AfterCommitContext::new(service.descriptor(), snapshot, service_keypair, tx_sender);
            service.as_ref().after_commit(context);
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

// TODO move to service module [ECR-3222]

#[derive(Debug)]
pub struct TransactionContext<'a, 'b> {
    service_descriptor: ServiceDescriptor<'a>,
    runtime_context: &'a mut RuntimeContext<'b>,
    runtime: &'a RustRuntime,
}

impl<'a, 'b> TransactionContext<'a, 'b> {
    pub fn service_id(&self) -> ServiceInstanceId {
        self.service_descriptor.service_id()
    }

    pub fn service_name(&self) -> &str {
        self.service_descriptor.service_name()
    }

    pub fn fork(&self) -> &Fork {
        self.runtime_context.fork
    }

    pub fn tx_hash(&self) -> Hash {
        self.runtime_context.tx_hash
    }

    pub fn author(&self) -> PublicKey {
        self.runtime_context.author
    }

    // TODO Should we support the ability to call other service from the rust runtime during
    // the transaction execution?
    pub fn dispatch_call(
        &mut self,
        call_info: CallInfo,
        payload: &[u8],
    ) -> Result<(), ExecutionError> {
        self.runtime
            .execute(self.runtime_context, call_info, payload)
    }

    pub(crate) fn dispatch_action(&mut self, action: dispatcher::Action) {
        self.runtime_context.dispatch_action(action)
    }
}

#[test]
fn parse_artifact_spec_correct() {
    RustArtifactSpec::from_str("my-service/1.0.0").unwrap();
}
