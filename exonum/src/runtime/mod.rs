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

//! Transactions Runtime.
//!
//! This module contains common building blocks for creating runtimes for the Exonum blockchain.
//!
//! Each runtime contains specific services to execute transactions, process events,
//! provide user APIs, etc. There is a unified dispatcher that redirects all the calls
//! and requests to the appropriate runtime environment. Thus, blockchain interacts with the
//! dispatcher, and not with a specific runtime instance.
//!
//! # Service Life Cycle
//!
//! 1. Each runtime has its own [artifacts] registry from which users can deploy them. The artifact
//! identifier is required by the runtime for constructing service instances. In other words,
//! an artifact identifier is similar to a class name, and a specific
//! service instance - to a class instance.
//!
//! 2. Each validator administrator requests the dispatcher to deploy an artifact
//! and then the validator node should send the confirmation if this request is successful. Then, if the
//! number of confirmations is equal to the total number of validators, each validator calls the
//! dispatcher to register the artifact as deployed. After that the validators can send requests to
//! start new service instances from this artifact.
//!
//! 3. To start a new service instance, each validator administrator should send a request
//! to the dispatcher. Each request contains exactly the same artifact identifier, instance name, and
//! instance configuration parameters. Then, as in the previous case, if the number of
//! confirmations is equal to the total number of validators, each validator calls the dispatcher
//! to start a new service instance.
//!
//! 4. Modify instance configuration procedure. TODO [ECR-3306]
//!
//! 5. Stop instance procedure. TODO
//!
//! Each Exonum transaction is an [`AnyTx`] message with a correct signature.
//!
//! # Transaction Life Cycle
//!
//! 1. An Exonum client creates a transaction message which includes [CallInfo] information
//! about the corresponding method to call and serialized method parameters as a payload.
//! The client then signs the message with the author's key pair.
//!
//! 2. The client transmits the message to one of the Exonum nodes in the network.
//! The transaction is identified by the hash of the corresponding message.
//!
//! 3. The node verifies the transaction for correctness of the signature and retransmits it to
//! the other network nodes if it is correct.
//!
//! 4. When the validator decides to include the transaction into the next block it takes the message
//! from the transaction pool and passes it to the [`Dispatcher`] for execution.
//!
//! 5. The dispatcher uses a lookup table to find the corresponding [`Runtime`] for the transaction
//! by the service [instance_id] recorded in the message. If the corresponding runtime is
//! successfully found, the dispatcher passes the transaction into this runtime for
//! immediate [execution].
//!
//! 6. After execution the transaction [execution status] is written into the blockchain.
//!
//! # Service interfaces
//!
//! In addition to its own methods, a service can implement methods of additional interfaces.
//! In your own runtime implementation, you must provide support of the following interfaces:
//!
//! TODO: Think about runtime agnostic interfaces description. [ECR-3531]
//!
//! ## Configure
//!
//! Describes a procedure for updating the configuration of a service instance.
//!
//! See explanation in the Rust runtime definition of the [`Configure`] interface.
//!
//! [`AnyTx`]: struct.AnyTx.html
//! [`CallInfo`]: struct.CallInfo.html
//! [`Dispatcher`]: dispatcher/struct.Dispatcher.html
//! [`instance_id`]: struct.CallInfo.html#structfield.instance_id
//! [`Runtime`]: trait.Runtime.html
//! [execution]: trait.Runtime.html#execute
//! [execution status]: error/struct.ExecutionStatus.html
//! [artifacts]: struct.ArtifactId.html
//! [`Configure`]: rust/interfaces/trait.Configure.html

pub(crate) use self::dispatcher::Dispatcher;
pub use self::{
    dispatcher::{Error as DispatcherError, Mailbox, Schema as DispatcherSchema},
    error::{ErrorKind, ExecutionError},
    types::{
        AnyTx, ArtifactId, ArtifactSpec, CallInfo, DeployStatus, InstanceId, InstanceQuery,
        InstanceSpec, MethodId,
    },
};

#[macro_use]
pub mod rust;
pub mod api;
pub mod error;

use futures::Future;

use std::fmt::Debug;

use exonum_merkledb::{Fork, Snapshot};

use crate::{
    api::ApiContext,
    crypto::{Hash, PublicKey, SecretKey},
    node::ApiSender,
};

use self::api::ServiceApiBuilder;

mod dispatcher;
mod types;

/// Persistent identifier of supervisor service instance.
///
/// Only a service with this ID can perform actions with the dispatcher.
pub const SUPERVISOR_INSTANCE_ID: InstanceId = 0;
/// Persistent name of supervisor service instance.
pub const SUPERVISOR_INSTANCE_NAME: &str = "supervisor";

/// List of predefined runtimes.
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
#[repr(u32)]
pub enum RuntimeIdentifier {
    /// Built-in Rust runtime.
    Rust = 0,
    /// Exonum Java Binding runtime.
    Java = 1,
}

impl From<RuntimeIdentifier> for u32 {
    fn from(id: RuntimeIdentifier) -> Self {
        id as Self
    }
}

/// This trait describes runtime environment for the Exonum services.
///
/// You can read more about the life cycle of services and transactions
/// [above](index.html#service-life-cycle).
///
/// Using this trait, you can extend the Exonum blockchain with the services written in
/// different languages. It assumes that the deployment procedure of a new service may be
/// complex and long and even may fail;
/// therefore, we introduce an additional entity - artifacts.
/// Each artifact has a unique identifier and, depending on the runtime, may have an additional
/// specification needed for its deployment. For example, the file to be compiled.
/// Artifact creates corresponding service instances similar to classes in the object
/// oriented programming.
///
/// # Notes
///
/// * Please pay attention to the panic handling policy during the implementation of methods.
/// If no policy is specified, then the method should not panic and each panic will abort the node.
/// * If you have to revert changes in the fork, you should revert only changes which were made by
/// the service that caused panic.
/// * Keep in mind that runtime methods can be executed in two ways: during the blocks execution
/// and during the node restart. Thus, be careful to avoid unnecessary actions in the runtime
/// methods.
///
/// # Hints
///
/// * You may use [`catch_panic`](error/fn.catch_panic.html) method to catch panics according to panic policy.
pub trait Runtime: Send + Debug + 'static {
    /// Request to deploy artifact with the given identifier and additional deploy specification.
    ///
    /// # Policy on Panics
    ///
    /// * Catch each kind of panics except for `FatalError` and convert
    /// them into `ExecutionError`.
    fn deploy_artifact(
        &mut self,
        artifact: ArtifactId,
        deploy_spec: Vec<u8>,
    ) -> Box<dyn Future<Item = (), Error = ExecutionError>>;

    /// Return true if the specified artifact is deployed in this runtime.
    fn is_artifact_deployed(&self, id: &ArtifactId) -> bool;

    /// Return Protobuf description of the deployed artifact with the specified identifier.
    /// If the artifact is not deployed, return `None`.
    ///
    /// # Notes for Runtime Developers
    ///
    /// * Ensure that the deployed artifact has the following information, even if it is empty.
    fn artifact_protobuf_spec(&self, id: &ArtifactId) -> Option<ArtifactProtobufSpec>;

    /// Runs the constructor of a new service instance with the given specification
    /// and initial configuration.
    ///
    /// The service is not guaranteed to be added to the runtime, so the runtime may freely
    /// discard the instantiated service instance after completing this method. Instead,
    /// the service is added to the runtime permanently via `add_service()`.
    ///
    /// # Policy on Panics
    ///
    /// * Catch each kind of panics except for `FatalError` and convert
    /// them into `ExecutionError`.
    fn start_adding_service(
        &self,
        fork: &Fork,
        spec: &InstanceSpec,
        parameters: Vec<u8>,
    ) -> Result<(), ExecutionError>;

    /// Permanently adds a service to the runtime.
    ///
    /// `start_adding_service()` is guaranteed to be called with the same `spec` and `parameters`
    /// earlier.
    fn add_service(&mut self, spec: &InstanceSpec) -> Result<(), ExecutionError>;

    /// Dispatch payload to the method of a specific service instance.
    /// Service instance name and method ID are provided in the `call_info` argument and
    /// interface name is provided as the corresponding field of the `context` argument.
    ///
    /// # Notes for Runtime Developers.
    ///
    /// * If service does not implement required interface, return `NoSuchInterface` error.
    /// * If interface does not have required method, return `NoSuchMethod` error.
    /// * For compatibility reasons, the interface name for user transactions is currently
    /// always blank. But it may be changed in future releases.
    ///
    /// # Policy on Panics
    ///
    /// Do not process. Panic will be processed by the method caller.
    fn execute(
        &self,
        context: ExecutionContext,
        call_info: &CallInfo,
        arguments: &[u8],
    ) -> Result<(), ExecutionError>;

    /// Gets the state hashes of the every available service.
    fn state_hashes(&self, snapshot: &dyn Snapshot) -> StateHashAggregator;

    /// Calls `before_commit` for all the services stored in the runtime.
    ///
    /// # Guarantees
    ///
    /// - Calls are ordered by increasing `instance_id`.
    /// - Each `before_commit` call is isolated with a separate checkpoint. A call that returns
    ///   an error will be rolled back.
    ///
    /// # Policy on Panics
    ///
    /// * Catch each kind of panics except for `FatalError` and write
    /// them into the log.
    /// * If panic occurs, the runtime rolls back the changes in the fork.
    fn before_commit(
        &self,
        context: ExecutionContext,
        instance_id: InstanceId,
    ) -> Result<(), ExecutionError>;

    /// Calls `after_commit` for all the services stored in the runtime.
    ///
    /// # Policy on Panics
    ///
    /// * Catch each kind of panics except for `FatalError` and write
    /// them into the log.
    fn after_commit(
        &mut self,
        mailbox: &mut Mailbox,
        snapshot: &dyn Snapshot,
        service_keypair: &(PublicKey, SecretKey),
        tx_sender: &ApiSender,
    );

    /// Collect the full list of API handlers from the runtime for the built-in Exonum API server.
    ///
    /// This method is called during the API server restart. Use this method if you do not plan to
    /// use your own API processing mechanism.
    ///
    /// Warning! It is a temporary method which retains the existing `RustRuntime` code.
    /// It will be removed in the future.
    #[doc(hidden)]
    fn api_endpoints(&self, _context: &ApiContext) -> Vec<(String, ServiceApiBuilder)> {
        Vec::new()
    }

    /// Notify the runtime about the changes in the list of service instances.
    ///
    /// The purpose of this method is to provide building blocks to create your own
    /// API processing mechanisms.
    fn notify_api_changes(&self, _context: &ApiContext, _changes: &[ApiChange]) {}

    /// Notify the runtime that it has to shutdown.
    ///
    /// This callback is invoked sequentially for each runtime just before the node shutdown,
    /// so runtimes can stop themselves gracefully.
    ///
    /// Invoking of this callback is guaranteed to be the last operation for the runtime.
    /// Since this method is a part of shutdown process, runtimes can perform blocking and
    /// heavy operations here if needed.
    fn shutdown(&self) {}
}

impl<T: Runtime> From<T> for Box<dyn Runtime> {
    fn from(value: T) -> Self {
        Box::new(value)
    }
}

/// Artifact Protobuf file sources.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProtoSourceFile {
    /// File name.
    pub name: String,
    /// File contents.
    pub content: String,
}

impl From<&(&str, &str)> for ProtoSourceFile {
    fn from(v: &(&str, &str)) -> Self {
        Self {
            name: v.0.to_owned(),
            content: v.1.to_owned(),
        }
    }
}

/// Artifact Protobuf specification for the Exonum clients.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct ArtifactProtobufSpec {
    /// List of Protobuf files that make up the service interface. The first element in the tuple
    /// is the file name, the second one is its content.
    ///
    /// The common interface entry point is always in the `service.proto` file.
    pub sources: Vec<ProtoSourceFile>,
}

impl From<&[(&str, &str)]> for ArtifactProtobufSpec {
    fn from(sources_strings: &[(&str, &str)]) -> Self {
        let sources = sources_strings.iter().map(From::from).collect();

        Self { sources }
    }
}

/// An accessory structure that aggregates root object hashes of the service
/// information schemas of the runtime with the root hash of the runtime information schema itself.
#[derive(Debug, PartialEq, Default)]
pub struct StateHashAggregator {
    /// List of hashes of the root objects of the runtime information schemas.
    pub runtime: Vec<Hash>,
    /// List of hashes of the root objects of the service instances schemas.
    pub instances: Vec<(InstanceId, Vec<Hash>)>,
}

/// The initiator of the method execution.
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum Caller {
    /// A usual transaction from the Exonum client, authorized by its key pair.
    Transaction {
        /// Hash of the transaction message.
        hash: Hash,
        /// Public key of the user who signed this transaction.
        author: PublicKey,
    },

    /// Method is invoked during the method execution of a different service.
    Service {
        /// Identifier of the service instance which invoked this method.
        instance_id: InstanceId,
    },

    /// Method is invoked by the `before_commit` hook.
    BeforeCommit,
}

impl Caller {
    /// Return the author's public key, if it exists.
    pub fn author(&self) -> Option<PublicKey> {
        self.as_transaction().map(|(_hash, author)| author)
    }

    /// Return the transaction hash, if it exists.
    pub fn transaction_hash(&self) -> Option<Hash> {
        self.as_transaction().map(|(hash, _)| hash)
    }

    /// Try to reinterpret caller as an authorized transaction.
    pub fn as_transaction(&self) -> Option<(Hash, PublicKey)> {
        if let Caller::Transaction { hash, author } = self {
            Some((*hash, *author))
        } else {
            None
        }
    }

    /// Try to reinterpret caller as service.
    pub fn as_service(&self) -> Option<InstanceId> {
        if let Caller::Service { instance_id } = self {
            Some(*instance_id)
        } else {
            None
        }
    }

    /// Verify that the caller of this method is supervisor service.
    pub fn as_supervisor(&self) -> Option<()> {
        self.as_service().and_then(|instance_id| {
            if instance_id == SUPERVISOR_INSTANCE_ID {
                Some(())
            } else {
                None
            }
        })
    }
}

/// Provide the current state of the blockchain and the caller information in respect of the transaction
/// which is being executed.
#[derive(Debug)]
pub struct ExecutionContext<'a> {
    /// The current state of the blockchain. It includes the new, not-yet-committed, changes to
    /// the database made by the previous transactions already executed in this block.
    pub fork: &'a mut Fork,
    /// The initiator of the transaction execution.
    pub caller: Caller,
    /// Identifier of the service interface required for the call. Keep in mind that this field in
    /// fact is a part of unfinished "interfaces feature" and will be replaced in future releases.
    /// At the moment this field can only contains a core interfaces like `Configure` and
    /// always empty for the common the service interfaces.
    pub interface_name: &'a str,
    /// Reference to the dispatcher.
    dispatcher: &'a Dispatcher,
    /// Depth of call stack.
    call_stack_depth: usize,
}

impl<'a> ExecutionContext<'a> {
    /// Maximum depth of the call stack.
    const MAX_CALL_STACK_DEPTH: usize = 256;

    pub(crate) fn new(dispatcher: &'a Dispatcher, fork: &'a mut Fork, caller: Caller) -> Self {
        Self {
            dispatcher,
            fork,
            caller,
            interface_name: "",
            call_stack_depth: 0,
        }
    }

    pub(crate) fn child_context(&mut self, caller_service_id: InstanceId) -> ExecutionContext {
        ExecutionContext {
            dispatcher: self.dispatcher,
            fork: self.fork,
            caller: Caller::Service {
                instance_id: caller_service_id,
            },
            interface_name: "",
            call_stack_depth: self.call_stack_depth + 1,
        }
    }

    pub(crate) fn call(
        &mut self,
        interface_name: &str,
        call_info: &CallInfo,
        arguments: &[u8],
    ) -> Result<(), ExecutionError> {
        if self.call_stack_depth >= ExecutionContext::MAX_CALL_STACK_DEPTH {
            let kind = crate::runtime::dispatcher::Error::StackOverflow;
            let msg = format!(
                "Maximum depth of call stack has been reached. `MAX_CALL_STACK_DEPTH` is {}.",
                ExecutionContext::MAX_CALL_STACK_DEPTH
            );
            return Err((kind, msg).into());
        }

        let runtime = self
            .dispatcher
            .runtime_for_service(call_info.instance_id)
            .ok_or(DispatcherError::IncorrectRuntime)?;
        let reborrowed = ExecutionContext {
            fork: &mut *self.fork,
            caller: self.caller,
            interface_name,
            dispatcher: self.dispatcher,
            call_stack_depth: self.call_stack_depth,
        };
        runtime.execute(reborrowed, call_info, arguments)
    }

    fn reborrow(&mut self) -> ExecutionContext {
        ExecutionContext {
            fork: &mut *self.fork,
            caller: self.caller,
            interface_name: self.interface_name,
            dispatcher: self.dispatcher,
            call_stack_depth: self.call_stack_depth,
        }
    }
}

/// Instance descriptor contains information to access running service instance.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct InstanceDescriptor<'a> {
    /// The unique numeric ID of the service instance.
    /// [Read more.](struct.InstanceSpec.html#structfield.id)
    pub id: InstanceId,
    /// The unique name of the service instance.
    /// [Read more.](struct.InstanceSpec.html#structfield.name)
    pub name: &'a str,
}

impl std::fmt::Display for InstanceDescriptor<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.id, self.name)
    }
}

impl From<InstanceDescriptor<'_>> for (InstanceId, String) {
    fn from(descriptor: InstanceDescriptor) -> Self {
        (descriptor.id, descriptor.name.to_owned())
    }
}

/// Change in the list of service instances.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum ApiChange {
    /// New instance has been added.
    InstanceAdded(InstanceId),
    /// Instance has been removed.
    InstanceRemoved(InstanceId),
}
