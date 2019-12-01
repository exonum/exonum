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

//! Common building blocks for creating runtimes for the Exonum blockchain.
//!
//! Each runtime contains specific services to execute transactions, process events,
//! provide user APIs, etc. A unified dispatcher redirects all the calls
//! and requests to the appropriate runtime environment. Thus, a blockchain interacts with the
//! dispatcher, and not with specific runtime instances.
//!
//! # Artifacts
//!
//! Each runtime has its own [artifacts] registry from which users can create a service.
//! The artifact identifier is required by the runtime to construct service instances.
//! In other words, an artifact identifier is similar to a class name, and a specific
//! service instance - to a class instance. A single artifact may be used to instantiate
//! zero or more services.
//!
//! The format of an artifact ID is uniform across runtimes (it is essentially a string),
//! but the runtime may customize artifact deployment via runtime-specific deployment arguments.
//!
//! # Artifact Lifecycle
//!
//! 1. An artifact is assembled in a way specific to the runtime. For example, the artifact may
//!   be compiled from sources and packaged using an automated build system.
//!
//! 2. The artifact with the service is deployed on the blockchain. The decision to deploy the
//!   artifact and the deployment spec are usually performed by the blockchain administrators.
//!   The corresponding logic is customizable via a [supervisor service](#supervisor-service).
//!   What deployment entails depends on the runtime; e.g., the artifact may be downloaded
//!   by each Exonum node, verified for integrity and then added into the execution environment.
//!
//! 3. For each node, an artifact may be deployed either asynchronously or synchronously /
//!   in a blocking manner. The supervisor usually first commands a node to deploy the artifact
//!   asynchronously via [`Mailbox`] once the decision to start deployment is reached
//!   by the blockchain administrators. Async deployment speed and outcome may differ among nodes.
//!
//! 4. The supervisor translates local deployment outcomes into a consensus-agreed result.
//!   For example, the supervisor may collect confirmations from the validator nodes that have
//!   successfully deployed the artifact, and once all the validator nodes have sent
//!   their confirmations, the artifact is *committed*. Being part of the service logic,
//!   artifact commitment is completely deterministic, agreed via consensus,
//!   and occurs at the same blockchain height for all nodes in the network.
//!
//! 5. Once the artifact is committed, every node in the network is required to have it deployed
//!   in order to continue functioning.
//!   If a node has not deployed the artifact previously, deployment becomes blocking; the node
//!   does not participate in consensus or block processing until the deployment is completed
//!   successfully. If the deployment is unsuccessful, the node stops indefinitely.
//!   Due to deployment confirmation mechanics built into the supervisor, it is reasonable
//!   to assume that a deployment failure at this stage is local to the node and
//!   could be fixed by the node admin.
//!
//! # Service Lifecycle
//!
//! 1. Once the artifact is committed, it is possible to instantiate a corresponding service.
//!   Each instantiation request contains an ID of a previously deployed artifact,
//!   a string instance ID, and instantiation arguments in a binary encoding
//!   (by convention, Protobuf). As with the artifacts, the logic controlling instantiation
//!   is encapsulated in the supervisor service.
//!
//! 2. During instantiation, the service is assigned a numeric ID, which is used to reference
//!   the service in transactions. The runtime can execute initialization logic defined
//!   in the service artifact; e.g., the service may store some initial data in the storage,
//!   check service dependencies, etc. The service (or the enclosing runtime) may signal that
//!   the initialization failed, in which case the service is considered not instantiated.
//!
//! 3. Once the service is instantiated, it can process transactions and interact with the
//!   external users in other ways. Different services instantiated from the same artifact
//!   are independent and have separate blockchain storages. Users can distinguish services
//!   by their IDs; both numeric and string IDs are unique within a blockchain.
//!
//! [`Dispatcher`] is responsible for persisting artifacts and services across node restarts.
//!
//! # Transaction Lifecycle
//!
//! 1. An Exonum client creates a transaction message which includes [`CallInfo`] information
//!   about the corresponding method to call and serialized method parameters as a payload.
//!   The client then signs the message using the Ed25519 signature system.
//!
//! 2. The client transmits the message to one of the Exonum nodes in the network.
//!
//! 3. The node verifies correctness of the transaction signature and retransmits it to
//!   the other network nodes if it is correct.
//!
//! 4. When the consensus algorithm run by Exonum nodes finds a feasible candidate for the next block
//!   of transactions, transactions in the block are passed to the [`Dispatcher`] for execution.
//!
//! 5. The dispatcher uses a lookup table to find the corresponding [`Runtime`] for the transaction
//!   by the [`instance_id`] recorded in the message. If the corresponding runtime is
//!   found, the dispatcher passes the transaction into this runtime for
//!   immediate [execution].
//!
//! 6. After execution the transaction [execution status] is written into the blockchain.
//!
//! # Supervisor Service
//!
//! A supervisor service is a service that has additional privileges. This service
//! allows to deploy artifacts and instantiate new services after the blockchain is launched
//! and running. Other than that, it looks like an ordinary service.
//! The supervisor should be present during the blockchain start, otherwise no new artifacts / services
//! could ever be added to the blockchain.
//!
//! The supervisor service is distinguished by its numerical ID, which must be set
//! to [`SUPERVISOR_INSTANCE_ID`]. Services may assume that transactions originating from
//! the supervisor service are authorized by the blockchain administrators. This can be used
//! in services: if a certain transaction originates from a service with `SUPERVISOR_INSTANCE_ID`,
//! it is authorized by the administrators.
//!
//! [`AnyTx`]: struct.AnyTx.html
//! [`CallInfo`]: struct.CallInfo.html
//! [`Dispatcher`]: struct.Dispatcher.html
//! [`instance_id`]: struct.CallInfo.html#structfield.instance_id
//! [`Runtime`]: trait.Runtime.html
//! [execution]: trait.Runtime.html#execute
//! [execution status]: error/struct.ExecutionStatus.html
//! [artifacts]: struct.ArtifactId.html
//! [`SUPERVISOR_INSTANCE_ID`]: constant.SUPERVISOR_INSTANCE_ID.html
//! [`Mailbox`]: struct.Mailbox.html

pub use self::{
    blockchain_data::{BlockchainData, SnapshotExt},
    dispatcher::{Dispatcher, Error as DispatcherError, Mailbox, Schema as DispatcherSchema},
    error::{ErrorKind, ExecutionError, ServiceFail},
    types::{
        AnyTx, ArtifactId, ArtifactSpec, CallInfo, DeployStatus, InstanceId, InstanceQuery,
        InstanceSpec, MethodId,
    },
};

#[macro_use]
pub mod rust;
pub mod error;

use futures::Future;

use std::fmt;

use exonum_merkledb::{BinaryValue, Fork, Snapshot};

use crate::{
    blockchain::Blockchain,
    crypto::{Hash, PublicKey},
    helpers::ValidateInput,
};

mod blockchain_data;
mod dispatcher;
mod types;

/// Persistent identifier of supervisor service instance.
///
/// Only a service with this ID can perform actions with the dispatcher.
pub const SUPERVISOR_INSTANCE_ID: InstanceId = 0;

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
/// complex and long and even may fail; therefore, an additional entity is introduced - *artifacts*.
/// Each artifact has a unique identifier and, depending on the runtime, may have an additional
/// specification needed for its deployment; e.g., files to be compiled.
/// An artifact creates corresponding service instances similar to classes in object-oriented
/// programming.
///
/// # Call Ordering
///
/// Within the lifetime of a `Runtime`, calls to its methods have the following order:
///
/// ```text
/// LIFE ::= initialize (GENESIS | RESUME) BLOCK* shutdown
/// GENESIS ::= (deploy_artifact | start_adding_service commit_service)* after_commit
/// RESUME ::= (deploy_artifact | commit_service)* on_resume
/// BLOCK* ::= PROPOSAL+ COMMIT
/// PROPOSAL ::= (execute | start_adding_service)* before_commit*
/// COMMIT ::= deploy_artifact* commit_service* after_commit
/// ```
///
/// The ordering for the "readonly" methods `is_artifact_deployed` and `state_hashes` in relation
/// to the lifecycle above is not specified.
///
/// # Consensus and Local Methods
///
/// The following methods should return the same result given the same arguments for all nodes
/// in the blockchain network:
///
/// - `execute`
/// - `before_commit`
/// - `start_adding_service`
/// - `state_hashes`
///
/// All these methods except for `state_hashes` should also produce the same changes
/// to the storage via provided `ExecutionContext`. Discrepancy in node behavior within
/// these methods may lead to a consensus failure.
///
/// The other `Runtime` methods may execute logic specific to the node.
///
/// # Handling Panics
///
/// Unless specified in the method docs, a panic in the `Runtime` methods will **not** be caught
/// and will cause node termination. You may use [`catch_panic`](error/fn.catch_panic.html) method
/// to catch panics according to panic policy.
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
    /// is guaranteed to not change between `initialize` and `on_resume` calls.
    ///
    /// The default implementation does nothing.
    fn on_resume(&mut self) {}

    /// Request to deploy artifact with the given identifier and additional deploy specification.
    ///
    /// This method is called *once* for a specific artifact during `Runtime` lifetime:
    ///
    /// - For newly added artifacts, the method is called as the decision to deploy the artifact
    ///   is made by the supervisor service.
    /// - After a node restart, the method is called for all previously deployed artifacts.
    // TODO: Elaborate constraints on `Runtime::deploy_artifact` futures (ECR-3840)
    fn deploy_artifact(
        &mut self,
        artifact: ArtifactId,
        deploy_spec: Vec<u8>,
    ) -> Box<dyn Future<Item = (), Error = ExecutionError>>;

    /// Return true if the specified artifact is deployed in this runtime.
    fn is_artifact_deployed(&self, id: &ArtifactId) -> bool;

    /// Runs the constructor of a new service instance with the given specification
    /// and initial arguments. The constructor can initialize the storage of the service,
    /// check for dependencies, etc.
    ///
    /// The constructor is run *exactly once* during blockchain lifetime for each successfully
    /// initialized service instance. That is to say, it is *not* called on a node restart.
    /// The caveat here is "successfully initialized"; at the point `start_adding_service` is called,
    /// the service is not guaranteed to eventually be added to the blockchain via `commit_service`.
    /// Indeed, committing the service will not follow if the alternative block proposal without
    /// the service instantiation was accepted. If the `commit_service` call *is* performed, it is
    /// guaranteed to be performed in the closest committed block, i.e., before the nearest
    /// `Runtime::after_commit()`.
    ///
    /// The dispatcher does not route transactions and `before_commit` events to the service
    /// until after `commit_service()` is called with the same instance spec.
    ///
    /// The runtime should discard the instantiated service instance after completing this method,
    /// unless there are compelling reasons to retain it (e.g., creating an instance takes very
    /// long time).
    /// Alternatively, "garbage" services may be removed from `Runtime` in `after_commit`
    /// because of the time dependence between `commit_service` and `after_commit` described above.
    /// The runtime should commit long-term resources for the service after a `commit_service()` call.
    /// Since discarded instances persist their state in a discarded fork, no further action
    /// is required to remove this state.
    ///
    /// # Return value
    ///
    /// The `Runtime` should catch all panics except for `FatalError`s and convert
    /// them into an `ExecutionError`.
    ///
    /// Returning an error or panicking provides a way for the `Runtime` to signal that
    /// service instantiation has failed. As a rule of a thumb, changes made by the method
    /// will be rolled back after such a signal (the exact logic is determined by the supervisor).
    /// Because an error is one of expected / handled outcomes, verifying prerequisites
    /// for instantiation and reporting corresponding failures should be performed at this stage
    /// rather than in `commit_service`.
    fn start_adding_service(
        &self,
        context: ExecutionContext<'_>,
        spec: &InstanceSpec,
        parameters: Vec<u8>,
    ) -> Result<(), ExecutionError>;

    /// Permanently adds a service to the runtime.
    ///
    /// This method is called *once* for a specific service instance during `Runtime` lifetime:
    ///
    /// - For newly added instances, the method is called when the fork with the corresponding
    ///   `start_adding_service()` call is committed.
    /// - After a node restart, the method is called for all existing service instances.
    ///
    /// It is guaranteed that `start_adding_service()` was called with the same `spec` earlier
    /// and returned `Ok(())`. The results of the call (i.e., changes to the blockchain state)
    /// are guaranteed to be persisted from the call.
    ///
    /// # Arguments
    ///
    /// `snapshot` is the storage snapshot at the latest height when the method is called:
    ///
    /// - If the service is committed during node operation, `snapshot` is taken at the
    ///   moment after applying the fork for which the corresponding `start_adding_service`
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
    /// An error or panic returned from this method will not be processed and will lead
    /// to the node stopping. A runtime should only return an error / panic if the error is local
    /// to the node with reasonable certainty, rather than common to all nodes in the network.
    /// (The latter kind of errors should be produced during the preceding `start_adding_service`
    /// call.) The error should contain a description allowing the node administrator to determine
    /// the root cause of the error and (ideally) recover the node by eliminating it.
    fn commit_service(
        &mut self,
        snapshot: &dyn Snapshot,
        spec: &InstanceSpec,
    ) -> Result<(), ExecutionError>;

    /// Dispatches payload to the method of a specific service instance.
    ///
    /// # Arguments
    ///
    /// Service instance name and method ID are provided in the `call_info` argument and
    /// interface name is provided as the corresponding field of the `context` argument.
    ///
    /// Blank interface name denotes the "default" interface; it should be supported by all
    /// services. The methods of the default interface are defined by the service artifact
    /// and thus may have different signatures for different services.
    ///
    /// Non-empty interface name denotes an interface defined externally to the service instance.
    /// In this case, the name is a Protobuf flavor of a fully qualified name
    /// (e.g., `exonum.Configure`), and method signatures can be inferred from the name
    /// using an interface definition.
    ///
    /// **Note**. Support of non-default interfaces is experimental; as such, an IDL for them
    /// is not stabilized yet.
    ///
    /// # Return value
    ///
    /// - If service does not implement an interface, return `NoSuchInterface` error.
    /// - If the interface does not have a method, return `NoSuchMethod` error.
    ///
    /// An error or panic returned from this method will lead to the rollback of all changes
    /// in the fork enclosed in the `context`. Runtimes can, but are not required to convert panics
    /// into errors.
    fn execute(
        &self,
        context: ExecutionContext<'_>,
        call_info: &CallInfo,
        arguments: &[u8],
    ) -> Result<(), ExecutionError>;

    /// Gets the state hashes of the every available service in the runtime.
    fn state_hashes(&self, snapshot: &dyn Snapshot) -> StateHashAggregator;

    /// Notifies a service stored in this runtime about the end of the block, allowing it
    /// to modify the blockchain state after all transactions in the block are processed.
    ///
    /// `before_commit` is called for every service active at the beginning of the block
    /// (i.e., services instantiated within the block do **not** receive a call) exactly
    /// once for each block. The method is not called for the genesis block.
    ///
    /// # Return value
    ///
    /// An error or panic returned from this method will lead to the rollback of all changes
    /// in the fork enclosed in the `context`. Runtimes can, but are not required to convert panics
    /// into errors.
    fn before_commit(
        &self,
        context: ExecutionContext<'_>,
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
    /// `mailbox` is used to send async commands to the dispatcher. This mechanism is used, e.g.,
    /// by the supervisor service to enqueue artifact deployment. A runtime may ignore `mailbox`
    /// if its services (or the runtime itself) do not require privileged access to the dispatcher.
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
    /// At the moment this field is always empty for the primary the service interface.
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

    pub(crate) fn child_context(&mut self, caller_service_id: InstanceId) -> ExecutionContext<'_> {
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
        if self.call_stack_depth >= Self::MAX_CALL_STACK_DEPTH {
            let err = DispatcherError::stack_overflow(Self::MAX_CALL_STACK_DEPTH);
            return Err(err);
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
    fn from(descriptor: InstanceDescriptor<'_>) -> Self {
        (descriptor.id, descriptor.name.to_owned())
    }
}
