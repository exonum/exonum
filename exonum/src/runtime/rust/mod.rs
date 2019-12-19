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
//! In the Rust runtime a set of service artifacts that you may want to deploy is static. The set
//! is defined at the time of compilation. Once the set is created, you can change it only by
//! the node binary recompilation.
//!
//! Beware of removing artifacts from the Rust runtime. An attempt to remove an artifact
//! from an instance that is already running can cause the blockchain to break. It is only safe
//! to add new artifacts.
//!
//! The Rust runtime does not provide any level of service isolation from the operation system.
//! Therefore, the security audit of the artifacts that should be deployed is up to the node administrators.
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
//! use exonum::{
//!     proto::schema::doc_tests,
//!     runtime::{
//!         rust::{CallContext, Service},
//!         BlockchainData, ExecutionError,
//!     },
//! };
//! use exonum_derive::*;
//! use exonum_merkledb::Snapshot;
//! use exonum_proto::ProtobufConvert;
//! use exonum_crypto::Hash;
//!
//! // Determine the types of data that will be used in service transactions.
//!
//! #[derive(Debug, PartialEq, ProtobufConvert, BinaryValue, ObjectHash)]
//! #[protobuf_convert(source = "doc_tests::CreateWallet")]
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
//! pub struct PointService;
//!
//! // Do not forget to implement the `Transactions` and `Service` traits
//! // for the service.
//! impl Transactions<CallContext<'_>> for PointService {
//!     type Output = Result<(), ExecutionError>;
//!
//!     fn create_wallet(
//!         &self,
//!         _context: CallContext<'_>,
//!         _arg: CreateWallet,
//!     ) -> Result<(), ExecutionError> {
//!         // Some business logic...
//! #       Ok(())
//!     }
//! }
//!
//! impl Service for PointService {}
//! ```
//!
//! ## Stateful Service Definition
//!
//! Beware of stateful services in production, use this functionality only for debugging and
//! prototyping.
//!
//! ```
//! # use exonum::runtime::{rust::{CallContext, Service}, BlockchainData, ExecutionError};
//! # use exonum_crypto::Hash;
//! # use exonum_derive::{exonum_interface, ServiceDispatcher, ServiceFactory};
//! # use exonum_merkledb::Snapshot;
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
//!
//! # impl Transactions<CallContext<'_>> for StatefulService {
//! #     type Output = Result<(), ExecutionError>;
//! # }
//! #
//! # impl Service for StatefulService {}
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
//! | `(PublicKey, SecretKey)` | Generates signed transactions |
//! | [`Broadcaster`] | Broadcasts transactions signed by the service keys of the node |
//! | [`CallContext`] | Calls methods of another service during transaction execution **(1)** |
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
//! The mutable trait is necessary for some stub types (e.g., `CallContext`) because they need
//! to mutate their state when processing the calls. Hence, the mutable trait should be
//! exported from the crate along with the original "immutable" trait.
//!
//! [`TxStub`]: struct.TxStub.html
//! [`Broadcaster`]: struct.Broadcaster.html
//! [`CallContext`]: struct.CallContext.html
//! [`GenericCall`]: trait.GenericCall.html
//! [`GenericCallMut`]: trait.GenericCallMut.html
//!
//! ## Interface usage
//!
//! ```
//! # use exonum::runtime::{rust::CallContext, ExecutionError};
//! # use exonum_crypto::gen_keypair;
//! # use exonum_derive::exonum_interface;
//! # type CreateWallet = String;
//! # type Transfer = String;
//! #[exonum_interface]
//! pub trait Transactions<Ctx> {
//!     type Output;
//!     fn create_wallet(&self, context: Ctx, arg: CreateWallet) -> Self::Output;
//!     fn transfer(&self, context: Ctx, arg: Transfer) -> Self::Output;
//! }
//!
//! // Create a signed transaction.
//! let keypair = gen_keypair();
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
//!     mut ctx: CallContext<'_>,
//!     wallet_count: u64,
//! ) -> Result<(), ExecutionError> {
//!     let receiver_service = "token";
//!     // ^-- `CallContext` allows to use any of service IDs as the context.
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

pub use self::{
    call_context::CallContext,
    error::Error,
    runtime_api::{ArtifactProtobufSpec, ProtoSourceFile, ProtoSourcesQuery},
    service::{
        AfterCommitContext, Broadcaster, DefaultInstance, Service, ServiceDispatcher,
        ServiceFactory,
    },
    stubs::{GenericCall, GenericCallMut, Interface, MethodDescriptor, TxStub},
};

pub mod api;
pub mod error;

use exonum_merkledb::Snapshot;
use futures::{future, sync::mpsc, Future, IntoFuture, Sink};

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::{
    api::{manager::UpdateEndpoints, ApiBuilder},
    blockchain::{Blockchain, Schema as CoreSchema},
    helpers::Height,
    runtime::{
        dispatcher::{self, Mailbox},
        error::{catch_panic, ExecutionError},
        ArtifactId, BlockchainData, CallInfo, ExecutionContext, InstanceDescriptor, InstanceId,
        InstanceSpec, InstanceStatus, Runtime, RuntimeIdentifier, WellKnownRuntime,
    },
};

use self::api::ServiceApiBuilder;

mod call_context;
mod runtime_api;
mod service;
mod stubs;
#[cfg(test)]
mod tests;

/// Rust runtime entity.
///
/// [Detailed description of the Rust runtime](index.html).
#[derive(Debug)]
pub struct RustRuntime {
    blockchain: Option<Blockchain>,
    api_notifier: mpsc::Sender<UpdateEndpoints>,
    available_artifacts: HashMap<ArtifactId, Box<dyn ServiceFactory>>,
    deployed_artifacts: HashSet<ArtifactId>,
    started_services: BTreeMap<InstanceId, Instance>,
    started_services_by_name: HashMap<String, InstanceId>,
    changed_services_since_last_block: bool,
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
}

impl AsRef<dyn Service + 'static> for Instance {
    fn as_ref(&self) -> &(dyn Service + 'static) {
        self.service.as_ref()
    }
}

impl RustRuntime {
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
            changed_services_since_last_block: false,
        }
    }

    fn blockchain(&self) -> &Blockchain {
        self.blockchain
            .as_ref()
            .expect("Method called before Rust runtime is initialized")
    }

    /// Adds a new service factory to the runtime and returns
    /// a modified `RustRuntime` object for further chaining.
    pub fn with_factory(mut self, service_factory: impl Into<Box<dyn ServiceFactory>>) -> Self {
        let service_factory = service_factory.into();
        let artifact = service_factory.artifact_id();
        trace!("Added available artifact {}", artifact);
        self.available_artifacts.insert(artifact, service_factory);
        self
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
            return Err(dispatcher::Error::ArtifactAlreadyDeployed.into());
        }
        if !self.available_artifacts.contains_key(&artifact) {
            return Err(Error::UnableToDeploy.into());
        }

        trace!("Deployed artifact: {}", artifact);
        self.deployed_artifacts.insert(artifact.to_owned());
        Ok(())
    }

    fn new_service(&self, spec: &InstanceSpec) -> Result<Instance, ExecutionError> {
        if !self.deployed_artifacts.contains(&spec.artifact) {
            return Err(dispatcher::Error::ArtifactNotDeployed.into());
        }
        if self.started_services.contains_key(&spec.id) {
            return Err(dispatcher::Error::ServiceIdExists.into());
        }
        if self.started_services_by_name.contains_key(&spec.name) {
            return Err(dispatcher::Error::ServiceNameExists.into());
        }

        let service = self.available_artifacts[&spec.artifact].create_instance();
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
                    .send(UpdateEndpoints { user_endpoints })
                    .wait()
                    .ok();
            }
        }
        self.changed_services_since_last_block = false;
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
        spec: &InstanceSpec,
        parameters: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        let instance = self.new_service(spec)?;
        let service = instance.as_ref();
        let descriptor = instance.descriptor();
        let context = CallContext::new(context, descriptor);
        catch_panic(|| service.initialize(context, parameters))
    }

    fn update_service_status(
        &mut self,
        _snapshot: &dyn Snapshot,
        spec: &InstanceSpec,
        status: InstanceStatus,
    ) -> Result<(), ExecutionError> {
        match status {
            InstanceStatus::Active => {
                let instance = self.new_service(spec)?;
                self.add_started_service(instance);
            }

            InstanceStatus::Stopped => {
                self.remove_started_service(spec);
            }
        }
        self.changed_services_since_last_block = true;
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
        let id = call_info.method_id;
        catch_panic(|| {
            instance.as_ref().call(
                context.interface_name,
                id,
                CallContext::new(context, descriptor),
                payload,
            )
        })
    }

    fn before_transactions(
        &self,
        context: ExecutionContext<'_>,
        instance_id: InstanceId,
    ) -> Result<(), ExecutionError> {
        let instance = self
            .started_services
            .get(&instance_id)
            .expect("`before_transactions` called with non-existing `instance_id`");

        let descriptor = instance.descriptor();
        catch_panic(|| {
            let context = CallContext::new(context, descriptor);
            instance.as_ref().before_transactions(context)
        })
    }

    fn after_transactions(
        &self,
        context: ExecutionContext<'_>,
        instance_id: InstanceId,
    ) -> Result<(), ExecutionError> {
        let instance = self
            .started_services
            .get(&instance_id)
            .expect("`after_transactions` called with non-existing `instance_id`");

        let descriptor = instance.descriptor();
        catch_panic(|| {
            let context = CallContext::new(context, descriptor);
            instance.as_ref().after_transactions(context)
        })
    }

    fn after_commit(&mut self, snapshot: &dyn Snapshot, mailbox: &mut Mailbox) {
        self.push_api_changes();

        // By convention, services do not handle `after_commit()` on the genesis block.
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
