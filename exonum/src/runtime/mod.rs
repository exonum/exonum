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
//! 4. // TODO modify instance configuration procedure.
//!
//! 5. // TODO stop instance procedure.
//!
//! Each Exonum transaction is an [`AnyTx`] message with a correct signature.
//!
//! # Transaction Life Cycle
//!
//! 1. An Exonum client creates a transaction message which includes [CallInfo] information
//! about the corresponding handler and serialized transaction parameters as a payload.
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
//!
//! [`AnyTx`]: struct.AnyTx.html
//! [`CallInfo`]: struct.CallInfo.html
//! [`Dispatcher`]: dispatcher/struct.Dispatcher.html
//! [`instance_id`]: struct.CallInfo.html#structfield.instance_id
//! [`Runtime`]: trait.Runtime.html
//! [execution]: trait.Runtime.html#execute
//! [execution status]: error/struct.ExecutionStatus.html
//! [artifacts]: struct.ArtifactId.html

pub use self::{
    error::{ErrorKind, ExecutionError},
    types::{AnyTx, ArtifactId, CallInfo, InstanceId, InstanceSpec, MethodId},
};

#[macro_use]
pub mod rust;
pub mod api;
pub mod dispatcher;
pub mod error;
pub mod supervisor;

use exonum_merkledb::{Fork, Snapshot};
use futures::Future;

use std::fmt::Debug;

use crate::{
    api::ApiContext,
    crypto::{Hash, PublicKey, SecretKey},
    node::ApiSender,
    proto::Any,
};

use self::{
    api::ServiceApiBuilder,
    dispatcher::{Dispatcher, DispatcherSender},
};

mod types;

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
        deploy_spec: Any,
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

    /// Start a new service instance with the given specification.
    ///
    /// # Policy on Panics
    ///
    /// * Catch each kind of panics except for `FatalError` and convert
    /// them into `ExecutionError`.
    /// * If panic occurs, the runtime must ensure that it is in a consistent state.
    fn start_service(&mut self, spec: &InstanceSpec) -> Result<(), ExecutionError>;

    /// Configure a service instance with the given parameters.
    ///
    /// The configuration parameters passed to the method are discarded immediately.
    /// So the service instance should save them by itself if it is important for
    /// the service business logic.
    ///
    /// This method is called in two cases:
    ///
    /// * After creating a new service instance by the [`start_service`] invocation. In this case,
    /// if an error during this action occurs, the dispatcher will invoke [`stop_service`].
    /// Make sure that this invocation will not fail.
    /// * During the configuration change procedure. [ECR-3306]
    ///
    /// # Policy on Panics
    ///
    /// * Catch each kind of panics except for `FatalError` and convert
    /// them into `ExecutionError`.
    ///
    /// ['start_service`]: #start_service
    /// ['stop_service`]: #stop_service
    fn configure_service(
        &self,
        fork: &Fork,
        descriptor: InstanceDescriptor,
        parameters: Any,
    ) -> Result<(), ExecutionError>;

    /// Stop existing service instance with the given specification.
    ///
    /// # Policy on Panics
    ///
    /// * Catch each kind of panics except for `FatalError` and convert
    /// them into `ExecutionError`.
    /// * If panic occurs, the runtime must ensure that it is in a consistent state.
    fn stop_service(&mut self, descriptor: InstanceDescriptor) -> Result<(), ExecutionError>;

    /// Execute service transaction.
    ///
    /// # Policy on Panics
    ///
    /// Do not process. Panic will be processed by the method caller.
    fn execute(
        &self,
        context: &mut ExecutionContext,
        call_info: CallInfo,
        arguments: &[u8],
    ) -> Result<(), ExecutionError>;

    /// Gets the state hashes of the every available service.
    fn state_hashes(&self, snapshot: &dyn Snapshot) -> StateHashAggregator;

    /// Calls `before_commit` for all the services stored in the runtime.
    ///
    /// # Notes for Runtime Developers.
    ///
    /// * The order of services during invocation of this method must be the same for each node.
    /// In other words, the order of the runtime services must be the same for each node.
    ///
    /// # Policy on Panics
    ///
    /// * Catch each kind of panics except for `FatalError` and write
    /// them into the log.
    /// * If panic occurs, the runtime rolls back the changes in the fork.
    fn before_commit(&self, context: &mut ExecutionContext);

    /// Calls `after_commit` for all the services stored in the runtime.
    ///
    /// # Policy on Panics
    ///
    /// * Catch each kind of panics except for `FatalError` and write
    /// them into the log.
    fn after_commit(
        &self,
        dispatcher: &DispatcherSender,
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
}

impl<T> From<T> for Box<dyn Runtime>
where
    T: Runtime,
{
    fn from(runtime: T) -> Self {
        Box::new(runtime) as Self
    }
}

/// Artifact Protobuf specification for the Exonum clients.
#[derive(Debug, PartialEq)]
pub struct ArtifactProtobufSpec<'a> {
    /// List of Protobuf files that make up the service interface. The first element in the tuple
    /// is the file name, the second one is its content.
    ///
    /// The common interface entry point is always in the `service.proto` file.
    pub sources: &'a [(&'a str, &'a str)],
}

impl<'a> Default for ArtifactProtobufSpec<'a> {
    /// Create blank artifact information without any proto sources.
    fn default() -> Self {
        const EMPTY_SOURCES: [(&str, &str); 0] = [];

        Self {
            sources: EMPTY_SOURCES.as_ref(),
        }
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

/// The initiator of the transaction execution.
#[derive(Debug, PartialEq)]
pub enum Caller {
    /// A usual transaction from the Exonum client, authorized by its key pair.
    Transaction {
        /// Hash of the transaction message.
        hash: Hash,
        /// Public key of the user who signed this transaction.
        author: PublicKey,
    },
    // This transaction is invoked on behalf of the blockchain itself,
    // for example [`before_commit`](trait.Runtime#before_commit) event.
    Blockchain,
    // This transaction is invoked during the transaction execution of the other service.
    Service {
        /// Identifier of the service instance which invoked this transaction.
        instance_id: InstanceId,
    }
}

impl Caller {
    /// Return the author's public key, if it exists.
    pub fn author(&self) -> Option<PublicKey> {
        self.as_transaction().map(|(_hash, author)| *author)
    }

    /// Return the transaction hash, if it exists.
    pub fn transaction_hash(&self) -> Option<Hash> {
        self.as_transaction().map(|(hash, _)| *hash)
    }

    /// Try to reinterpret caller as an authorized transaction.
    pub fn as_transaction(&self) -> Option<(&Hash, &PublicKey)> {
        if let Caller::Transaction { hash, author } = self {
            Some((hash, author))
        } else {
            None
        }
    }

    /// Try to reinterpret caller as blockchain.
    pub fn as_blockchain(&self) -> Option<()> {
        if let Caller::Blockchain = self {
            Some(())
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
    /// List of dispatcher actions that will be performed after the finish of execution.
    actions: Vec<dispatcher::Action>,
    /// Reference to the underlying runtime dispatcher.
    dispatcher: &'a Dispatcher,
}

impl<'a> ExecutionContext<'a> {
    pub(crate) fn new(dispatcher: &'a Dispatcher, fork: &'a mut Fork, caller: Caller) -> Self {
        Self {
            fork,
            caller,
            actions: Vec::new(),
            dispatcher,
        }
    }

    pub(crate) fn call(
        &mut self,
        call_info: CallInfo,
        arguments: &[u8],
    ) -> Result<(), ExecutionError> {
        // TODO Modify caller from Transaction.
        self.dispatcher.call(self, call_info, arguments)
    }

    pub(crate) fn dispatch_action(&mut self, action: dispatcher::Action) {
        self.actions.push(action);
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
