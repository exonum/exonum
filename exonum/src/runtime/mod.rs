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
//! [`AnyTx`]: struct.AnyTx.html
//! [`CallInfo`]: struct.CallInfo.html
//! [`Dispatcher`]: dispatcher/struct.Dispatcher.html
//! [`instance_id`]: struct.CallInfo.html#structfield.instance_id
//! [`Runtime`]: trait.Runtime.html
//! [execution]: trait.Runtime.html#execute
//! [execution status]: error/struct.ExecutionStatus.html
//! [artifacts]: struct.ArtifactId.html

pub use self::{
    dispatcher::{Dispatcher, Error as DispatcherError, Mailbox, Schema as DispatcherSchema},
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

use std::fmt;

use exonum_merkledb::{BinaryValue, Fork, Snapshot};

use crate::{
    blockchain::Blockchain,
    crypto::{Hash, PublicKey},
    helpers::ValidateInput,
};

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

/// Runtime environment for Exonum services.
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
#[allow(unused_variables)]
pub trait Runtime: Send + fmt::Debug + 'static {
    /// Initializes the runtime, providing a `Blockchain` instance for further use.
    ///
    /// This method is guaranteed to be called before any other `Runtime` methods. It is
    /// called *exactly once* during `Runtime` lifetime.
    ///
    /// The default implementation does nothing.
    fn initialize(&mut self, blockchain: &Blockchain) {}

    /// Notifies the runtime that the dispatcher has completed re-initialization after
    /// node restart, including restoring the deployed artifacts / started service instances
    /// for all runtimes.
    ///
    /// This method is called *no more than once* during `Runtime` lifetime. It is called iff
    /// the blockchain had genesis block when the node was started. The blockchain state
    /// is guaranteed to not change between `initialize` and `after_initialize` calls.
    ///
    /// The default implementation does nothing.
    fn on_resume(&mut self) {}

    /// Request to deploy artifact with the given identifier and additional deploy specification.
    ///
    /// This method may be called multiple times with the same params; in particular, the method
    /// is called for all deployed artifacts after node restart.
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

    /// Runs the constructor of a new service instance with the given specification
    /// and initial configuration.
    ///
    /// The service is not guaranteed to be added to the blockchain at this point.
    /// In particular, the dispatcher does not route transactions and `before_commit` events
    /// until after `commit_service()` is called with the same instance spec. A call
    /// to `commit_service()` is not guaranteed for each `start_adding_service()`; indeed,
    /// committing the service will not follow if the alternative block proposal without
    /// the service instantiation was accepted. If the call is performed, it is
    /// guaranteed to be performed in the closest committed block, i.e., before the nearest
    /// `Runtime::after_commit()`.
    ///
    /// The runtime can discard the instantiated service instance after completing this method.
    /// (Alternatively, "garbage" services may be removed from `Runtime` in `after_commit`
    /// because of the time dependence between `commit_service` and `after_commit` described above.)
    /// The runtime should commit resources for the service after a `commit_service()` call.
    /// Since discarded instances persist their state in a discarded fork, no further action
    /// is required to remove this state.
    ///
    /// # Return value
    ///
    /// The `Runtime` should catch all panics except for `FatalError`s and convert
    /// them into an `ExecutionError`. A returned error or panic implies that service instantiation
    /// has failed; as a rule of a thumb, changes made by the method will be rolled back
    /// (the exact logic is determined by the supervisor).
    fn start_adding_service(
        &self,
        context: ExecutionContext,
        spec: &InstanceSpec,
        parameters: Vec<u8>,
    ) -> Result<(), ExecutionError>;

    /// Permanently adds a service to the runtime.
    ///
    /// It is guaranteed that `start_adding_service()` was called with the same `spec` earlier
    /// and returned `Ok(())`. The results of the call (i.e., changes to the blockchain state)
    /// are guaranteed to be persisted from the call.
    ///
    /// A call to `start_adding_service()` may have happened indefinite time ago;
    /// indeed, `commit_service()` is called for all services on the node startup. Likewise,
    /// `commit_service()` may be called an indefinite number of times for the same instance.
    ///
    /// `snapshot` is the storage snapshot at the latest height when the method is called:
    ///
    /// - If the service is committed during node operation, `snapshot` is taken at the
    ///   moment after applying the fork, for which the corresponding `start_adding_service`
    ///   was performed.
    /// - If the service is resumed after node restart, `snapshot` is the storage state
    ///   at the node start.
    ///
    /// For builtin services on the first node start `snapshot` will not contain info
    /// on the genesis block. Thus, using some core APIs (like requesting the current
    /// blockchain height) will result in a panic.
    ///
    /// # Return value
    ///
    /// Any error or panic returned from this method should be considered as fatal. There are edge
    /// cases where the returned error does not stop the enclosing process (e.g.,
    /// if several alternative initial service configurations are tried), but as a rule of thumb,
    /// a `Runtime` should not return an error or panic here unless it wants the node to stop forever.
    fn commit_service(
        &mut self,
        snapshot: &dyn Snapshot,
        spec: &InstanceSpec,
    ) -> Result<(), ExecutionError>;

    /// Dispatches payload to the method of a specific service instance.
    /// Service instance name and method ID are provided in the `call_info` argument and
    /// interface name is provided as the corresponding field of the `context` argument.
    ///
    /// # Notes for Runtime Developers
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

    /// Calls `before_commit` for a service stored in the runtime.
    ///
    /// `before_commit` is called for all services active at the beginning of the block
    /// (i.e., services instantiated within the block do **not** receive a call) exactly
    /// once for each block.
    ///
    /// # Guarantees
    ///
    /// - Each `before_commit` call is isolated with a separate checkpoint. A call that returns
    ///   an error will be rolled back.
    /// - Ordering of calls among service instances is not specified, but is guaranteed
    ///   to be the same for all nodes.
    fn before_commit(
        &self,
        context: ExecutionContext,
        instance_id: InstanceId,
    ) -> Result<(), ExecutionError>;

    /// Notifies the runtime about commitment of a new block.
    ///
    /// This method is guaranteed to be called *after* all `commit_service` calls related
    /// to the same block. The method is called exactly once for each block in the blockchain,
    /// including the genesis block.
    ///
    /// A block is not yet persisted when this method is called; the up to date block information
    /// is provided in the `snapshot`. It corresponds exactly to the information
    /// eventually persisted; i.e., no modifying operations are performed on the block.
    ///
    /// # Policy on Panics
    ///
    /// Catch each kind of panics except for `FatalError` and write them into the log. A panic
    /// will bubble up, i.e., will lead to immediate node termination.
    fn after_commit(&mut self, snapshot: &dyn Snapshot, mailbox: &mut Mailbox);

    /// Notify the runtime that it has to shutdown.
    ///
    /// This callback is invoked sequentially for each runtime just before the node shutdown,
    /// so runtimes can stop themselves gracefully.
    ///
    /// Invoking of this callback is guaranteed to be the last operation for the runtime.
    /// Since this method is a part of shutdown process, runtimes can perform blocking and
    /// heavy operations here if needed.
    fn shutdown(&mut self) {}
}

impl<T: Runtime> From<T> for Box<dyn Runtime> {
    fn from(value: T) -> Self {
        Box::new(value)
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

    /// Call is invoked by one of the blockchain lifecycle events.
    ///
    /// This kind of authorization is used for `before_commit` calls to the service instances,
    /// and for initialization of builtin services.
    Blockchain,
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
            let kind = DispatcherError::StackOverflow;
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
        let reborrowed = self.reborrow_with_interface(interface_name);
        runtime.execute(reborrowed, call_info, arguments)
    }

    /// Starts adding a new service instance to the blockchain. The created service is not active
    /// (i.e., does not process transactions or the `before_commit` hook)
    /// until the block built on top of the provided `fork` is committed.
    ///
    /// This method should be called for the exact context passed to the runtime.
    pub(crate) fn start_adding_service(
        &mut self,
        spec: InstanceSpec,
        constructor: impl BinaryValue,
    ) -> Result<(), ExecutionError> {
        // TODO: revise dispatcher integrity checks [ECR-3743]
        debug_assert!(spec.validate().is_ok(), "{:?}", spec.validate());
        let runtime = self
            .dispatcher
            .runtime_by_id(spec.artifact.runtime_id)
            .ok_or(DispatcherError::IncorrectRuntime)?;
        runtime.start_adding_service(self.reborrow(), &spec, constructor.into_bytes())?;

        // Add service instance to the dispatcher schema.
        DispatcherSchema::new(&*self.fork)
            .add_pending_service(spec)
            .map_err(From::from)
    }

    fn reborrow(&mut self) -> ExecutionContext<'_> {
        self.reborrow_with_interface(self.interface_name)
    }

    fn reborrow_with_interface<'s>(&'s mut self, interface_name: &'s str) -> ExecutionContext<'s> {
        ExecutionContext {
            fork: &mut *self.fork,
            caller: self.caller,
            interface_name,
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

impl fmt::Display for InstanceDescriptor<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.id, self.name)
    }
}

impl From<InstanceDescriptor<'_>> for (InstanceId, String) {
    fn from(descriptor: InstanceDescriptor) -> Self {
        (descriptor.id, descriptor.name.to_owned())
    }
}
