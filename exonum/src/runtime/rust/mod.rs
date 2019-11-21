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

//! The current runtime is for running native services written in Rust.
//!
//! A set of artifacts available to deploy in the Rust runtime is static. The set is defined at the time
//! of compilation. Once created, the set can be changed only by the node binary recompilation.
//!
//! Beware of removing the artifacts from the Rust runtime. An attempt to remove an artifact
//! from an instance that is already running can cause blockchain to break. It is only safe
//! to add new artifacts.
//!
//! The Rust runtime does not provide any level of service isolation from the operation system.
//! Therefore, the security audit of the artifacts that should be deployed is up to the node administrators.
//!
//! The artifact interface in the Rust runtime is represented by the
//! [`ServiceFactory`][ServiceFactory] trait. The trait creates service instances and provides
//! information about the artifact.
//!
//! # Examples
//!
//! ## Minimal complete example of an Exonum service definition.
//!
//! ```
//! use exonum::{
//!     proto::schema::doc_tests,
//!     runtime::{
//!         rust::{CallContext, Service},
//!         BlockchainData, ExecutionError,
//!     },
//! };
//! use exonum_derive::{
//!     exonum_interface, BinaryValue, IntoExecutionError,
//!     ObjectHash, ServiceDispatcher, ServiceFactory
//! };
//! use exonum_merkledb::Snapshot;
//! use exonum_proto::ProtobufConvert;
//! use exonum_crypto::Hash;
//!
//! // Determine the types of data that will be used in service transactions.
//!
//! #[derive(Debug, PartialEq, ProtobufConvert, BinaryValue, ObjectHash)]
//! #[protobuf_convert(source = "doc_tests::Point")]
//! pub struct Point {
//!     pub x: i32,
//!     pub y: i32,
//! }
//!
//! #[derive(Debug, PartialEq, ProtobufConvert, BinaryValue, ObjectHash)]
//! #[protobuf_convert(source = "doc_tests::CreateWallet")]
//! pub struct CreateWallet {
//!     pub name: String,
//! }
//!
//! // You may create service-specific error types.
//!
//! #[derive(Debug, IntoExecutionError)]
//! pub enum Error {
//!     PointAlreadyExists = 0,
//!     WalletAlreadyExists = 1,
//! }
//!
//! // Define the transaction interface for your service by creating a trait with
//! // the following attribute and method signatures.
//! // This attribute implements `Interface` trait for this trait and `Transaction`
//! // trait for each argument.
//! #[exonum_interface]
//! pub trait Transactions {
//!     // Each method of the trait should have a signature of the following format. The argument
//!     // should implement the `BinaryValue` trait.
//!     fn create_wallet(
//!         &self,
//!         context: CallContext<'_>,
//!         arg: CreateWallet,
//!     ) -> Result<(), ExecutionError>; // You may use `ExecutionError` directly.
//!     // Also you can use any type which implements `Into<ExecutionError>` for the error.
//!     fn add_point(
//!         &self,
//!         context: CallContext<'_>,
//!         arg: Point,
//!     ) -> Result<(), Error>;
//! }
//!
//! // In order a service could process transactions, you have to implement the
//! // `ServiceDispatcher` trait, which can be derived using the corresponding macro.
//! // To explain to the runtime how to create instances of this service, you have to implement
//! // the `ServiceFactory` trait by using the `ServiceFactory` derive macro.
//! #[derive(Debug, ServiceDispatcher, ServiceFactory)]
//! // Declare that the service implements the `Transactions` interface that was presented above.
//! #[service_dispatcher(implements("Transactions"))]
//! // By default the macro uses the crate name and version to provide an artifact ID for this
//! // service factory. You should only provide a path to the generated Protobuf schema.
//! #[service_factory(proto_sources = "exonum::proto::schema")]
//! pub struct PointService;
//!
//! // Do not forget to implement the `Transactions` and `Service` traits for the service.
//!
//! impl Transactions for PointService {
//!     fn create_wallet(
//!         &self,
//!         _context: CallContext<'_>,
//!         _arg: CreateWallet,
//!     ) -> Result<(), ExecutionError> {
//!         // Some business logic...
//!         Ok(())
//!     }
//!
//!     fn add_point(
//!         &self,
//!         _context: CallContext<'_>,
//!         _arg: Point
//!     ) -> Result<(), Error> {
//!         // Some business logic...
//!         Ok(())
//!     }
//! }
//!
//! impl Service for PointService {
//!     fn state_hash(&self, _data: BlockchainData<&dyn Snapshot>) -> Vec<Hash> {
//!         Vec::new()
//!     }
//! }
//! ```
//!
//! ## Stateful Service Definition
//!
//! Beware of stateful services in production, use this functionality only for debugging and
//! prototyping.
//!
//! ```
//! use exonum::runtime::{rust::Service, BlockchainData};
//! use exonum_crypto::Hash;
//! use exonum_derive::{exonum_interface, ServiceDispatcher, ServiceFactory};
//! use exonum_merkledb::Snapshot;
//!
//! #  #[exonum_interface]
//! #  pub trait Transactions {}
//!
//! // If your service has a state, for example, for debugging purposes, then you can
//! // use a separate structure for the service.
//!
//! #[derive(Debug, Default, ServiceDispatcher)]
//! #[service_dispatcher(implements("Transactions"))]
//! pub struct StatefulService {
//!     state: u64,
//! }
//!
//! #[derive(Debug, ServiceFactory)]
//! #[service_factory(
//!     // In this case you have to specify the service constructor explicitly.
//!     service_constructor = "Self::new_instance",
//!     proto_sources = "exonum::proto::schema",
//!     // To specify the artifact name and/or version explicitly you have to use the
//!     // following attributes.
//!     artifact_name = "stateful",
//!     artifact_version = "1.0.0",
//! )]
//! pub struct StatefulServiceFactory;
//!
//! impl StatefulServiceFactory {
//!     fn new_instance(&self) -> Box<dyn Service> {
//!         Box::new(StatefulService::default())
//!     }
//! }
//!
//! # impl Transactions for StatefulService {}
//! #
//! #  impl Service for StatefulService {
//! #      fn state_hash(&self, _data: BlockchainData<&dyn Snapshot>) -> Vec<Hash> {
//! #          Vec::new()
//! #      }
//! #  }
//! ```
//!
//! [ServiceFactory]: trait.ServiceFactory.html

pub use self::{
    api::ArtifactProtobufSpec,
    call_context::CallContext,
    error::Error,
    service::{
        AfterCommitContext, Interface, Service, ServiceDispatcher, ServiceFactory, Transaction,
    },
};

pub mod error;

use exonum_merkledb::{validation::is_valid_index_name, Snapshot};
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
    api::ServiceApiBuilder,
    dispatcher::{self, Mailbox},
    error::{catch_panic, ExecutionError},
    ArtifactId, BlockchainData, CallInfo, ExecutionContext, InstanceDescriptor, InstanceId,
    InstanceSpec, Runtime, RuntimeIdentifier, StateHashAggregator,
};

mod api;
mod call_context;
mod service;
#[cfg(test)]
mod tests;

/// Rust runtime entity.
///
/// [Detailed description of the Rust runtime](index.html).
#[derive(Debug)]
pub struct RustRuntime {
    blockchain: Option<Blockchain>,
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
        let blockchain_data = BlockchainData::new(snapshot, self.descriptor());
        (self.id, self.service.state_hash(blockchain_data))
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
    /// Rust runtime name.
    pub const NAME: &'static str = "rust";

    /// Creates a new Rust runtime instance.
    pub fn new(api_notifier: mpsc::Sender<UpdateEndpoints>) -> Self {
        Self {
            blockchain: None,
            api_notifier,
            available_artifacts: Default::default(),
            deployed_artifacts: Default::default(),
            started_services: Default::default(),
            started_services_by_name: Default::default(),
            new_services_since_last_block: false,
        }
    }

    fn blockchain(&self) -> &Blockchain {
        self.blockchain
            .as_ref()
            .expect("Method called before Rust runtime is initialized")
    }

    /// Adds a new service factory to the runtime.
    pub fn add_service_factory(&mut self, service_factory: Box<dyn ServiceFactory>) {
        let artifact = service_factory.artifact_id();
        trace!("Added available artifact {}", artifact);
        self.available_artifacts.insert(artifact, service_factory);
    }

    /// Adds a new service factory to the runtime and returns
    /// a modified `RustRuntime` object for further chaining.
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

    fn api_endpoints(&self) -> Vec<(String, ApiBuilder)> {
        self.started_services
            .values()
            .map(|instance| {
                let mut builder = ServiceApiBuilder::new(
                    self.blockchain().clone(),
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
            .chain(self::api::endpoints(self))
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
            }
            _ => Err(failure::format_err!(
                "Wrong Rust artifact name format. The name should be arranged \
                 as follows \"artifact_name:artifact_version\""
            )),
        }
    }
}

impl Runtime for RustRuntime {
    fn initialize(&mut self, blockchain: &Blockchain) {
        self.blockchain = Some(blockchain.clone());
    }

    // We need to propagate changes in the services immediately after initialization.
    fn on_resume(&mut self) {
        self.push_api_changes();
    }

    fn deploy_artifact(
        &mut self,
        artifact: ArtifactId,
        spec: Vec<u8>,
    ) -> Box<dyn Future<Item = (), Error = ExecutionError>> {
        if !spec.is_empty() {
            // Keep the spec for Rust artifacts empty.
            Box::new(future::err(Error::IncorrectArtifactId.into()))
        } else {
            Box::new(self.deploy(&artifact).into_future())
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
        let core_schema = CoreSchema::new(snapshot);
        if core_schema.height() == Height(0) {
            return;
        }

        let blockchain = self.blockchain();
        let validator_id = core_schema.validator_id(blockchain.service_keypair().0);
        for service in self.started_services.values() {
            service.as_ref().after_commit(AfterCommitContext::new(
                mailbox,
                service.descriptor(),
                snapshot,
                blockchain.service_keypair(),
                blockchain.sender(),
                validator_id,
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
        (
            "aAa",
            "Wrong Rust artifact name format. The name should be arranged as follows",
        ),
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
