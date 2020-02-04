// Copyright 2020 The Exonum Team
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
//! In the Rust runtime a set of service artifacts that you may want to deploy is static. The set
//! is defined at the time of compilation. Once the set is created, you can change it only by
//! the node binary recompilation.
//!
//! Beware of removing artifacts from the Rust runtime. An attempt to remove an artifact
//! from an instance that is already running can cause the blockchain to break. It is only safe
//! to add new artifacts.
//!
//! The Rust runtime does not provide any level of service isolation from the operation system.
//! Therefore, the security audit of the artifacts that should be deployed is
//! up to the node administrators.
//!
//! The artifact interface in the Rust runtime is represented by the
//! [`ServiceFactory`] trait. The trait creates service instances and provides
//! information about the artifact.
//!
//! [`ServiceFactory`]: trait.ServiceFactory.html
//!
//! # Examples
//!
//! ## Minimal complete example
//!
//! ```
//! use exonum::runtime::{BlockchainData, ExecutionError};
//! use exonum_rust_runtime::{ExecutionContext, Service};
//! use exonum_derive::*;
//! use serde_derive::*;
//!
//! // Determine the types of data that will be used in service transactions.
//!
//! #[derive(Debug, PartialEq, Serialize, Deserialize, BinaryValue)]
//! #[binary_value(codec = "bincode")]
//! pub struct CreateWallet {
//!     pub name: String,
//! }
//!
//! // You may create service-specific error types.
//!
//! #[derive(Debug, ExecutionFail)]
//! pub enum Error {
//!     /// Wallet with the specified owner key already exists.
//!     WalletAlreadyExists = 1,
//! }
//!
//! // Define a transaction interface for your service by creating a `Transactions` trait with
//! // the following attribute and method signatures.
//! #[exonum_interface]
//! pub trait Transactions<Ctx> {
//!     type Output;
//!     // Each method in service should have an `interface_method` attribute specifying its ID.
//!     // Alternative is to use `#[exonum_interface(auto_ids)]` to assign IDs automatically, but
//!     // this is not a good idea for production code, since the method IDs assigned automatically
//!     // can change (e.g. because of reordering methods in trait).
//!     #[interface_method(id = 0)]
//!     // Each method of the trait should have a signature of the following format.
//!     // The argument should implement the `BinaryValue` trait.
//!     fn create_wallet(&self, context: Ctx, arg: CreateWallet) -> Self::Output;
//! }
//!
//! // In order a service could process transactions, you have to implement the
//! // `ServiceDispatcher` trait, which can be derived using the corresponding macro.
//! // To explain to the runtime how to create instances of this service, you have
//! // to implement the `ServiceFactory` trait by using the `ServiceFactory` derive macro.
//! #[derive(Debug, ServiceDispatcher, ServiceFactory)]
//! // Declare that the service implements the `Transactions` interface
//! // that was presented above.
//! #[service_dispatcher(implements("Transactions"))]
//! // By default the macro uses the crate name and version to provide an artifact ID
//! // for this service factory. You should only provide a path to the generated
//! // Protobuf schema.
//! #[service_factory(proto_sources = "exonum::proto::schema")]
//! pub struct WalletService;
//!
//! // Do not forget to implement the `Transactions` and `Service` traits
//! // for the service.
//! impl Transactions<ExecutionContext<'_>> for WalletService {
//!     type Output = Result<(), ExecutionError>;
//!
//!     fn create_wallet(
//!         &self,
//!         context: ExecutionContext<'_>,
//!         arg: CreateWallet,
//!     ) -> Result<(), ExecutionError> {
//!         // Some business logic...
//!         Ok(())
//!     }
//! }
//!
//! impl Service for WalletService {}
//! ```
//!
//! ## Stateful Service Definition
//!
//! Beware of stateful services in production, use this functionality only for debugging and
//! prototyping.
//!
//! ```
//! # use exonum::runtime::{BlockchainData, ExecutionError};
//! # use exonum_rust_runtime::{ExecutionContext, Service};
//! # use exonum_derive::{exonum_interface, ServiceDispatcher, ServiceFactory};
//! #[exonum_interface]
//! pub trait Transactions<Ctx> {
//! #   type Output;
//!     // service methods...
//! }
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
//! # impl Transactions<ExecutionContext<'_>> for StatefulService {
//! #     type Output = Result<(), ExecutionError>;
//! # }
//! #
//! # impl Service for StatefulService {}
//! ```
//!
//! ## Removing Transactions
//!
//! If transaction became obsolete, it can be removed from the service.
//! Removed transaction will remain its ID, but attempt to invoke it will result
//! in returning [`CommonError::NoSuchMethod`].
//!
//! Note to service authors: when removing transaction from interface, leave a comment
//! why this method was removed and which ID it has. It is not required, but seeing the service
//! history may be helpful, and it's easier to see than ID in the macro above the trait.
//!
//! Example:
//!
//! ```
//! # use exonum::runtime::{ExecutionError};
//! # use exonum_rust_runtime::{ExecutionContext, Service};
//! # use exonum_derive::{exonum_interface, interface_method, ServiceDispatcher, ServiceFactory};
//! #[exonum_interface(removed_method_ids(0, 2))]
//! pub trait Transactions<Ctx> {
//!     type Output;
//!
//!     // Method with ID 0 is removed because it was buggy.
//!
//!     #[interface_method(id = 1)]
//!     fn actual_method(&self, context: Ctx, arg: u64) -> Self::Output;
//!
//!     // Method with ID 2 is removed because it wasn't used by anybody.
//! }
//!
//! #[derive(Debug, ServiceDispatcher, ServiceFactory)]
//! #[service_dispatcher(implements("Transactions"))]
//! #[service_factory(proto_sources = "exonum::proto::schema")]
//! pub struct SampleService;
//!
//! impl Transactions<ExecutionContext<'_>> for SampleService {
//!     type Output = Result<(), ExecutionError>;
//!
//!     // Implement only existing methods in trait.
//!     fn actual_method(
//!         &self,
//!         context: ExecutionContext<'_>,
//!         arg: u64,
//!     ) -> Result<(), ExecutionError> {
//!         // Some business logic...
//!         Ok(())
//!     }
//! }
//!
//! impl Service for SampleService {}
//! ```
//!
//! # Interfaces
//!
//! By bringing an interface trait into scope, you can use its methods with any stub type.
//! Stub here means a type that can process calls from *any* interface. (What exactly
//! is meant by processing depends on the stub and on the provided context argument.)
//! For example, the following stubs are defined in this crate:
//!
//! | Stub | Behavior |
//! |------|----------|
//! | [`TxStub`] | Generates unsigned transactions |
//! | `KeyPair` | Generates signed transactions (preferred) |
//! | `(PublicKey, SecretKey)` | Generates signed transactions |
//! | [`Broadcaster`] | Broadcasts transactions signed by the service keys of the node |
//! | [`ExecutionContext`] | Calls methods of another service during transaction execution **(1)** |
//!
//! 1. Beware that this is experimental functionality which is subject to change in next releases.
//!
//! More stub types can be defined in other crates. To define a stub type, you need to implement
//! one of [`GenericCall`] or [`GenericCallMut`] traits.
//!
//! ## Mutable interfaces
//!
//! `#[exonum_interface]` macro produces a mutable version of the interface trait,
//! which differs from the original trait in the following ways:
//!
//! - Name is the original trait name appended with `Mut` (e.g., `TransactionsMut`)
//! - All methods consume `&mut self` instead of `&self`
//!
//! Otherwise, the mutable trait is a carbon copy of the original trait.
//!
//! The mutable trait is necessary for some stub types (e.g., `ExecutionContext`) because they need
//! to mutate their state when processing the calls. Hence, the mutable trait should be
//! exported from the crate along with the original "immutable" trait.
//!
//! [`TxStub`]: struct.TxStub.html
//! [`Broadcaster`]: struct.Broadcaster.html
//! [`ExecutionContext`]: struct.ExecutionContext.html
//! [`GenericCall`]: trait.GenericCall.html
//! [`GenericCallMut`]: trait.GenericCallMut.html
//! [`CommonError::NoSuchMethod`]: https://docs.rs/exonum/latest/exonum/runtime/enum.CommonError.html
//!
//! ## Interface usage
//!
//! ```
//! # use exonum::runtime::ExecutionError;
//! # use exonum_rust_runtime::ExecutionContext;
//! # use exonum::crypto::KeyPair;
//! # use exonum_derive::{exonum_interface, interface_method};
//! # type CreateWallet = String;
//! # type Transfer = String;
//! #[exonum_interface]
//! pub trait Transactions<Ctx> {
//!     type Output;
//!     #[interface_method(id = 0)]
//!     fn create_wallet(&self, context: Ctx, arg: CreateWallet) -> Self::Output;
//!     #[interface_method(id = 1)]
//!     fn transfer(&self, context: Ctx, arg: Transfer) -> Self::Output;
//! }
//!
//! // Create a signed transaction.
//! let keypair = KeyPair::random();
//! let create_wallet: CreateWallet = // ...
//! #    "create_wallet".to_owned();
//! // The context in this case is the numerical instance ID.
//! let instance_id = 100;
//! let tx = keypair.create_wallet(instance_id, create_wallet);
//! let transfer: Transfer = // ...
//! #    "transfer".to_owned();
//! let other_tx = keypair.transfer(instance_id, transfer);
//! // The same call with the explicit method attribution:
//! # let transfer = "transfer".to_owned();
//! let other_tx = Transactions::transfer(&keypair, instance_id, transfer);
//!
//! // Using the trait within another service implementation:
//! # struct Service;
//! # type SomeArg = String;
//! # impl Service {
//! fn batch_transfers(
//!     &self,
//!     mut ctx: ExecutionContext<'_>,
//!     wallet_count: u64,
//! ) -> Result<(), ExecutionError> {
//!     let receiver_service = "token";
//!     // ^-- `ExecutionContext` allows to use any of service IDs as the context.
//!     for _ in 0..wallet_count {
//!         let transfer: Transfer = // ...
//! #           "transfer".to_owned();
//!         ctx.transfer(receiver_service, transfer)?;
//!         // The same call with the explicit attribution:
//! #       let transfer = "transfer".to_owned();
//!         TransactionsMut::transfer(&mut ctx, receiver_service, transfer)?;
//!     }
//!     Ok(())
//! }
//! # }
//! ```

#![warn(missing_debug_implementations, missing_docs)]
#![deny(unsafe_code, bare_trait_objects)]

pub use exonum::runtime::ExecutionContext;

pub use self::{
    error::Error,
    runtime_api::{ArtifactProtobufSpec, ProtoSourceFile, ProtoSourcesQuery},
    service::{
        AfterCommitContext, Broadcaster, DefaultInstance, Service, ServiceDispatcher,
        ServiceFactory,
    },
    stubs::{FallthroughAuth, GenericCall, GenericCallMut, Interface, MethodDescriptor, TxStub},
};

pub mod api;

use exonum::{
    blockchain::{Blockchain, Schema as CoreSchema},
    helpers::Height,
    merkledb::Snapshot,
    runtime::{
        catch_panic,
        migrations::{InitMigrationError, MigrateData, MigrationScript},
        versioning::Version,
        ArtifactId, ExecutionError, ExecutionFail, InstanceDescriptor, InstanceId, InstanceSpec,
        InstanceStatus, Mailbox, MethodId, Runtime, RuntimeIdentifier, WellKnownRuntime,
    },
};
use exonum_api::{ApiBuilder, UpdateEndpoints};
use futures::{future, sync::mpsc, Future, IntoFuture, Sink};
use log::trace;

use std::collections::{BTreeMap, HashMap, HashSet};

use self::api::ServiceApiBuilder;

mod error;
mod runtime_api;
mod service;
mod stubs;

#[doc(hidden)]
pub mod _reexports {
    //! Types necessary for `ServiceDispatcher` and `ServiceFactory` derive macros to work.

    pub use exonum::runtime::{
        ArtifactId, CommonError, ExecutionContext, ExecutionError, MethodId, RuntimeIdentifier,
    };
}

trait FactoryWithMigrations: ServiceFactory + MigrateData {}

impl<T: ServiceFactory + MigrateData> FactoryWithMigrations for T {}

/// Wrapper around a service factory that does not support migrations.
#[derive(Debug)]
struct WithoutMigrations<T>(T);

impl<T: ServiceFactory> ServiceFactory for WithoutMigrations<T> {
    fn artifact_id(&self) -> ArtifactId {
        self.0.artifact_id()
    }

    fn artifact_protobuf_spec(&self) -> ArtifactProtobufSpec {
        self.0.artifact_protobuf_spec()
    }

    fn create_instance(&self) -> Box<dyn Service> {
        self.0.create_instance()
    }
}

impl<T> MigrateData for WithoutMigrations<T> {
    fn migration_scripts(
        &self,
        _start_version: &Version,
    ) -> Result<Vec<MigrationScript>, InitMigrationError> {
        Err(InitMigrationError::NotSupported)
    }
}

/// Rust runtime entity.
///
/// [Detailed description of the Rust runtime](index.html).
#[derive(Debug)]
pub struct RustRuntime {
    blockchain: Option<Blockchain>,
    api_notifier: mpsc::Sender<UpdateEndpoints>,
    available_artifacts: HashMap<ArtifactId, Box<dyn FactoryWithMigrations>>,
    deployed_artifacts: HashSet<ArtifactId>,
    started_services: BTreeMap<InstanceId, Instance>,
    started_services_by_name: HashMap<String, InstanceId>,
    changed_services_since_last_block: bool,
}

/// Builder of the `RustRuntime`.
#[derive(Debug, Default)]
pub struct RustRuntimeBuilder {
    available_artifacts: HashMap<ArtifactId, Box<dyn FactoryWithMigrations>>,
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

    fn descriptor(&self) -> InstanceDescriptor {
        InstanceDescriptor::new(self.id, &self.name)
    }
}

impl AsRef<dyn Service> for Instance {
    fn as_ref(&self) -> &dyn Service {
        self.service.as_ref()
    }
}

impl RustRuntimeBuilder {
    /// Creates a new builder instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a new service factory to the runtime. The service factory does not support data
    /// migrations. Use [`with_migrating_factory`](#method.with_migrating_factory) to add
    /// a service factory with migration support.
    ///
    /// # Return value
    ///
    /// Returns a modified `RustRuntime` object for further chaining.
    pub fn with_factory<S: ServiceFactory>(mut self, service_factory: S) -> Self {
        let artifact = service_factory.artifact_id();
        trace!(
            "Added available artifact {} without migration support",
            artifact
        );
        let service_factory = WithoutMigrations(service_factory);
        self.available_artifacts
            .insert(artifact, Box::new(service_factory));
        self
    }

    /// Adds a new service factory with migration support to the runtime.
    ///
    /// # Return value
    ///
    /// Returns a modified `RustRuntime` object for further chaining.
    pub fn with_migrating_factory<S>(mut self, service_factory: S) -> Self
    where
        S: ServiceFactory + MigrateData,
    {
        let artifact = service_factory.artifact_id();
        trace!(
            "Added available artifact {} with migration support",
            artifact
        );
        self.available_artifacts
            .insert(artifact, Box::new(service_factory));
        self
    }

    /// Completes the build process, converting the builder into a `RustRuntime`.
    pub fn build(self, api_notifier: mpsc::Sender<UpdateEndpoints>) -> RustRuntime {
        RustRuntime {
            blockchain: None,
            api_notifier,
            available_artifacts: self.available_artifacts,
            deployed_artifacts: Default::default(),
            started_services: Default::default(),
            started_services_by_name: Default::default(),
            changed_services_since_last_block: false,
        }
    }

    /// Builds the Rust runtime without connection to the HTTP API. As the name implies,
    /// this method should only be used for testing.
    pub fn build_for_tests(self) -> RustRuntime {
        self.build(mpsc::channel(1).0)
    }
}

impl RustRuntime {
    /// Rust runtime name.
    pub const NAME: &'static str = "rust";

    /// Returns a new builder for the runtime.
    pub fn builder() -> RustRuntimeBuilder {
        RustRuntimeBuilder::new()
    }

    fn blockchain(&self) -> &Blockchain {
        self.blockchain
            .as_ref()
            .expect("Method called before Rust runtime is initialized")
    }

    fn add_started_service(&mut self, instance: Instance) {
        self.started_services_by_name
            .insert(instance.name.clone(), instance.id);
        self.started_services.insert(instance.id, instance);
    }

    fn remove_started_service(&mut self, instance: &InstanceSpec) {
        self.started_services_by_name.remove(&instance.name);
        self.started_services.remove(&instance.id);
    }

    fn deploy(&mut self, artifact: &ArtifactId) -> Result<(), ExecutionError> {
        if self.deployed_artifacts.contains(&artifact) {
            panic!(
                "BUG: Core requested deploy of already deployed artifact {:?}",
                artifact
            );
        }
        if !self.available_artifacts.contains_key(&artifact) {
            let description = format!(
                "Runtime failed to deploy artifact with id {}, \
                 it is not listed among available artifacts. Available artifacts: {}",
                artifact,
                self.artifacts_to_pretty_string()
            );
            return Err(Error::UnableToDeploy.with_description(description));
        }

        trace!("Deployed artifact: {}", artifact);
        self.deployed_artifacts.insert(artifact.to_owned());
        Ok(())
    }

    fn new_service(
        &self,
        artifact: &ArtifactId,
        instance: &InstanceDescriptor,
    ) -> Result<Instance, ExecutionError> {
        if !self.deployed_artifacts.contains(artifact) {
            panic!(
                "BUG: Core requested service instance start ({}) of not deployed artifact {}",
                instance.name, artifact
            );
        }
        if self.started_services.contains_key(&instance.id) {
            panic!(
                "BUG: Core requested service service instance start ({}) with already taken ID",
                instance
            );
        }
        if self.started_services_by_name.contains_key(&instance.name) {
            panic!(
                "BUG: Core requested service service instance start ({}) with already taken name",
                instance
            );
        }

        let service = self.available_artifacts[artifact].create_instance();
        Ok(Instance::new(
            instance.id,
            instance.name.to_owned(),
            service,
        ))
    }

    fn api_endpoints(&self) -> Vec<(String, ApiBuilder)> {
        self.started_services
            .values()
            .map(|instance| {
                let mut builder = ServiceApiBuilder::new(
                    self.blockchain().clone(),
                    InstanceDescriptor::new(instance.id, &instance.name),
                );
                instance.as_ref().wire_api(&mut builder);
                let root_path = builder
                    .take_root_path()
                    .unwrap_or_else(|| ["services/", &instance.name].concat());
                (root_path, ApiBuilder::from(builder))
            })
            .chain(self::runtime_api::endpoints(self))
            .collect()
    }

    fn push_api_changes(&mut self) {
        if self.changed_services_since_last_block {
            let user_endpoints = self.api_endpoints();
            // FIXME: this should either be made async, or an unbounded channel should be used.
            if !self.api_notifier.is_closed() {
                self.api_notifier
                    .clone()
                    .send(UpdateEndpoints {
                        endpoints: user_endpoints,
                    })
                    .wait()
                    .ok();
            }
        }
        self.changed_services_since_last_block = false;
    }

    fn artifacts_to_pretty_string(&self) -> String {
        if self.available_artifacts.is_empty() {
            return "None".to_string();
        }

        self.available_artifacts
            .keys()
            .map(ToString::to_string)
            .collect::<Vec<String>>()
            .join(", ")
    }
}

impl WellKnownRuntime for RustRuntime {
    const ID: u32 = RuntimeIdentifier::Rust as u32;
}

impl Runtime for RustRuntime {
    fn initialize(&mut self, blockchain: &Blockchain) {
        self.blockchain = Some(blockchain.clone());
    }

    // Propagates changes in the services immediately after initialization.
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
        self.deployed_artifacts.contains(id)
    }

    fn initiate_adding_service(
        &self,
        context: ExecutionContext<'_>,
        artifact: &ArtifactId,
        parameters: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        let instance = self.new_service(artifact, context.instance())?;
        let service = instance.as_ref();
        catch_panic(|| service.initialize(context, parameters))
    }

    fn initiate_resuming_service(
        &self,
        context: ExecutionContext<'_>,
        artifact: &ArtifactId,
        parameters: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        let instance = self.new_service(artifact, context.instance())?;
        let service = instance.as_ref();
        catch_panic(|| service.resume(context, parameters))
    }

    fn update_service_status(
        &mut self,
        _snapshot: &dyn Snapshot,
        spec: &InstanceSpec,
        status: &InstanceStatus,
    ) {
        match status {
            InstanceStatus::Active => {
                let instance = self
                    .new_service(&spec.artifact, &spec.as_descriptor())
                    .expect(
                    "BUG: Attempt to create a new service instance failed; \
                     within `instantiate_adding_service` we were able to create a new instance, \
                     but now we are not.",
                );
                self.add_started_service(instance);
            }
            InstanceStatus::Stopped => {
                self.remove_started_service(spec);
            }
            InstanceStatus::Migrating(_) => { /* Do nothing. */ }
            other => {
                panic!(
                    "Received non-expected service status: {}; \
                     Rust runtime isn't prepared to process this action, \
                     probably Rust runtime is outdated relative to the core library",
                    other
                );
            }
        }
        self.changed_services_since_last_block = true;
    }

    fn migrate(
        &self,
        new_artifact: &ArtifactId,
        data_version: &Version,
    ) -> Result<Option<MigrationScript>, InitMigrationError> {
        let artifact = self
            .available_artifacts
            .get(&new_artifact)
            .unwrap_or_else(|| {
                panic!(
                    "BUG: `migrate` call to a non-existing artifact {:?}",
                    new_artifact
                );
            });

        let mut scripts = artifact.migration_scripts(data_version)?;
        Ok(if scripts.is_empty() {
            None
        } else {
            Some(scripts.swap_remove(0))
        })
    }

    fn execute(
        &self,
        context: ExecutionContext<'_>,
        method_id: MethodId,
        payload: &[u8],
    ) -> Result<(), ExecutionError> {
        let instance = self
            .started_services
            .get(&context.instance().id)
            .expect("BUG: an attempt to execute transaction of unknown service.");

        catch_panic(|| instance.as_ref().call(context, method_id, payload))
    }

    fn before_transactions(&self, context: ExecutionContext<'_>) -> Result<(), ExecutionError> {
        let instance = self
            .started_services
            .get(&context.instance().id)
            .expect("`before_transactions` called with non-existing `instance_id`");

        catch_panic(|| instance.as_ref().before_transactions(context))
    }

    fn after_transactions(&self, context: ExecutionContext<'_>) -> Result<(), ExecutionError> {
        let instance = self
            .started_services
            .get(&context.instance().id)
            .expect("`after_transactions` called with non-existing `instance_id`");

        catch_panic(|| instance.as_ref().after_transactions(context))
    }

    fn after_commit(&mut self, snapshot: &dyn Snapshot, mailbox: &mut Mailbox) {
        self.push_api_changes();

        // By convention, services do not handle `after_commit()` on the genesis block.
        let core_schema = CoreSchema::new(snapshot);
        if core_schema.height() == Height(0) {
            return;
        }

        let blockchain = self.blockchain();
        let validator_id = core_schema.validator_id(blockchain.service_keypair().public_key());
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
