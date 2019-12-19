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

//! Common building blocks that compose runtimes for the Exonum blockchain.
//!
//! Each runtime contains specific services that execute transactions, process events,
//! provide user APIs, etc. A unified dispatcher redirects all the calls
//! and requests to an appropriate runtime environment. Thus, a blockchain interacts with the
//! dispatcher, and not with specific runtime instances.
//!
//! # Artifacts
//!
//! Each runtime has its own [artifacts] registry. Users can create services from the stored
//! artifacts. An artifact identifier is required by the runtime to construct service instances.
//! In other words, an artifact identifier is similar to a class name, and a specific
//! service instance - to a class instance. A single artifact may be used to instantiate
//! zero or more services.
//!
//! The format of the artifact ID is uniform across runtimes - it is essentially a string.
//! But the runtime may customize artifact deployment via runtime-specific deployment arguments.
//!
//! # Artifact Lifecycle
//!
//! 1. An artifact is assembled in a way specific to the runtime. For example, an artifact may
//!   be compiled from sources and packaged using an automated build system.
//!
//! 2. The artifact with the service is deployed on the blockchain. The decision to deploy the
//!   artifact and the deployment spec are usually performed by the blockchain administrators.
//!   The corresponding logic is customizable via the [supervisor service](#supervisor-service).
//!   What deployment entails depends on the runtime; e.g., the artifact may be downloaded
//!   by each Exonum node, verified for integrity and then added into the execution environment.
//!
//! 3. For each node, an artifact may be deployed either asynchronously or synchronously, that is
//!   in a blocking manner. The supervisor usually first commands a node to deploy the artifact
//!   asynchronously via [`Mailbox`], once the decision to start deployment is reached
//!   by the blockchain administrators. Asynchronous deployment speed and outcome may differ among
//!   nodes.
//!
//! 4. The supervisor translates the local deployment outcomes into a consensus-agreed result.
//!   For example, the supervisor may collect confirmations from the validator nodes that have
//!   successfully deployed the artifact. Once all the validator nodes have sent
//!   their confirmations, the artifact is *committed*. As a part of the service logic,
//!   artifact commitment is completely deterministic, agreed via consensus,
//!   and occurs at the same blockchain height for all nodes in the network.
//!
//! 5. Once the artifact is committed, every node in the network must have it deployed
//!   in order to continue functioning.
//!   If a node has not deployed the artifact previously, deployment becomes blocking. The node
//!   does not participate in consensus or block processing until the deployment is completed
//!   successfully. If the deployment is unsuccessful, the node stops indefinitely.
//!   The deployment confirmation mechanics is built into the supervisor. Thus, it is reasonable
//!   to assume that a deployment failure at this stage is local to the node and
//!   could be fixed by the node admin.
//!
//! # Service Lifecycle
//!
//! 1. Once the artifact is committed, it is possible to instantiate the corresponding service.
//!   Each instantiation request contains an ID of the previously deployed artifact,
//!   a string instance ID, and instantiation arguments in a binary encoding
//!   (by convention, Protobuf). As with the artifacts, the logic that controls instantiation
//!   is encapsulated in the supervisor service.
//!
//! 2. During instantiation the service gets a numeric ID, which is used to reference
//!   the service in transactions. The runtime can execute initialization logic defined
//!   in the service artifact; e.g., the service may store some initial data in the storage,
//!   check service dependencies, etc. If the service (or the enclosing runtime) signals that
//!   the initialization failed, the service is considered not instantiated.
//!
//! 3. Once the service is instantiated, it can process transactions and interact with the
//!   external users in other ways. Different services instantiated from the same artifact
//!   are independent and have separate blockchain storages. Users can distinguish services
//!   by their IDs; both numeric and string IDs are unique within a blockchain.
//!
//! 4. Active service instances can be stopped by a corresponding request to [`Dispatcher`].
//!   A stopped service no longer participates in business logic, i.e. it does not process
//!   transactions, events, does not interact with the users in any way.
//!   Service data becomes unavailable for the other services, but still exists. The service name
//!   and identifier remain reserved for the stopped service and can't be used again for
//!   adding new services.
//!
//! The [`Dispatcher`] is responsible for persisting artifacts and services across node restarts.
//!
//! # Transaction Lifecycle
//!
//! 1. An Exonum client creates a transaction message which includes two parts. The first part is
//!   the [`CallInfo`] - information about a method to call. The second part is the
//!   serialized method parameters as a payload.
//!   The client then signs the message using the Ed25519 signature system.
//!
//! 2. The client transmits the message to one of the Exonum nodes in the network.
//!
//! 3. The node verifies correctness of the transaction signature and retransmits it to
//!   the other network nodes if it is correct.
//!
//! 4. When the consensus algorithm finds a feasible candidate for the next block
//!   of transactions, transactions in this block are passed to the [`Dispatcher`] for execution.
//!
//! 5. The dispatcher uses a lookup table to find the corresponding [`Runtime`] for each transaction
//!   by the [`instance_id`] recorded in the transaction message. If the corresponding runtime exists,
//!   the dispatcher passes the transaction into this runtime for immediate [execution].
//!
//! 6. After execution the transaction [execution status] is written into the blockchain.
//!
//! # Supervisor Service
//!
//! A supervisor service is a service that has additional privileges. This service
//! allows deploying artifacts and instantiating new services after the blockchain is launched
//! and running. Moreover the Supervisor service allows update the configuration or stop the
//! active service instances.
//! Other than that, it looks like an ordinary service.
//!
//! To enable adding new artifacts / services to the blockchain after its start, the supervisor
//! must be one of the builtin service instances.
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
    blockchain_data::{BlockchainData, ForkExt, SnapshotExt},
    dispatcher::{Dispatcher, Error as DispatcherError, Mailbox, Schema as DispatcherSchema},
    error::{CallSite, CallType, ErrorKind, ErrorMatch, ExecutionError, ExecutionFail},
    types::{
        AnyTx, ArtifactId, ArtifactSpec, ArtifactState, ArtifactStatus, CallInfo, InstanceId,
        InstanceQuery, InstanceSpec, InstanceState, InstanceStatus, MethodId,
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

/// Persistent identifier of a supervisor service instance.
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

impl std::convert::TryFrom<u32> for RuntimeIdentifier {
    type Error = ();

    fn try_from(id: u32) -> Result<Self, Self::Error> {
        match id {
            0 => Ok(Self::Rust),
            1 => Ok(Self::Java),
            _ => Err(()),
        }
    }
}

impl std::fmt::Display for RuntimeIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rust => write!(f, "Rust"),
            Self::Java => write!(f, "Java"),
        }
    }
}

/// Runtime environment for the Exonum services.
///
/// You can read more about the life cycle of services and transactions
/// [above](index.html#service-life-cycle).
///
/// Using this trait, you can extend the Exonum blockchain with the services written in
/// different languages. It assumes that the deployment procedure of a new service may be
/// complex and long and even may fail. Therefore, an additional entity is introduced - *artifacts*.
/// Each artifact has a unique identifier. Depending on the runtime, an artifact may have an
/// additional specification required for its deployment; e.g., files to be compiled.
/// An artifact creates corresponding service instances similar to classes in object-oriented
/// programming.
///
/// # Call Ordering
///
/// Within the lifetime of a `Runtime`, calls to its methods have the following order:
///
/// ```text
/// LIFE ::= initialize (GENESIS | RESUME) BLOCK* shutdown
/// GENESIS ::= (deploy_artifact | initiate_adding_service update_service_status)* after_commit
/// RESUME ::= (deploy_artifact | update_service_status)* on_resume
/// BLOCK* ::= PROPOSAL+ COMMIT
/// PROPOSAL ::= before_transactions* (execute | initiate_adding_service)* after_transactions*
/// COMMIT ::= deploy_artifact* update_service_status* after_commit
/// ```
///
/// The ordering for the "read-only" method `is_artifact_deployed` in relation
/// to the lifecycle above is not specified.
///
/// # Consensus and Local Methods
///
/// The following methods should return the same result if provided arguments are the same for all
/// the nodes in the blockchain network:
///
/// - `before_transactions`
/// - `execute`
/// - `after_transactions`
/// - `initiate_adding_service`
///
/// All these methods should also produce the same changes to the storage via
/// the provided `ExecutionContext`. Discrepancy in node behavior within these methods may lead
/// to a consensus failure.
///
/// Other `Runtime` methods may execute logic specific to the node.
///
/// # Handling Panics
///
/// Panics in the `Runtime` methods are **not** caught. A panic in the runtime method will cause
/// the node termination. To catch panics in the Rust code and convert them to unchecked execution
/// errors, use the [`catch_panic`](error/fn.catch_panic.html) method.
#[allow(unused_variables)]
pub trait Runtime: Send + fmt::Debug + 'static {
    /// Initializes the runtime, providing a `Blockchain` instance for further use.
    ///
    /// Calling this method always takes place before calling any other `Runtime` methods.
    /// The `initialize` method is called *exactly once* during the `Runtime` lifetime.
    ///
    /// The default implementation does nothing.
    fn initialize(&mut self, blockchain: &Blockchain) {}

    /// Notifies the runtime that the dispatcher has completed re-initialization after the
    /// node restart. Re-initialization includes restoring the deployed artifacts / started service
    /// instances for all the runtimes.
    ///
    /// This method is called *maximum once* during the `Runtime` lifetime. It is called iff
    /// the genesis block was already created before the node start (e.g. after node relaunch).
    /// The blockchain state will remain the same between the `initialize` and `on_resume` calls.
    ///
    /// The default implementation does nothing.
    fn on_resume(&mut self) {}

    /// A request to deploy an artifact with the given identifier and an additional deploy
    /// specification.
    ///
    /// This method is called *once* for a specific artifact during the `Runtime` lifetime:
    ///
    /// - For newly added artifacts, the method is called as the supervisor service decides to deploy
    ///   the artifact.
    /// - After the node restart, the method is called for all the previously deployed artifacts.
    // TODO: Elaborate constraints on `Runtime::deploy_artifact` futures (ECR-3840)
    fn deploy_artifact(
        &mut self,
        artifact: ArtifactId,
        deploy_spec: Vec<u8>,
    ) -> Box<dyn Future<Item = (), Error = ExecutionError>>;

    /// Returns `true` if the specified artifact is deployed in this runtime.
    fn is_artifact_deployed(&self, id: &ArtifactId) -> bool;

    /// Runs the constructor of a new service instance with the given specification
    /// and initial arguments. The constructor can initialize the storage of the service,
    /// check for dependencies, etc.
    ///
    /// The constructor runs *exactly once* during the blockchain lifetime for each successfully
    /// initialized service instance. That is to say, the constructor is *not* called on a node
    /// restart.
    ///
    /// At the same time, when `initiate_adding_service` is called,
    /// there is no guarantee that the service will eventually get to the blockchain via
    /// `update_service_status`. The consensus may accept an alternative block proposal, in which
    /// the service is not instantiated or instantiated with different parameters.
    ///
    /// The `update_service_status` call always takes place
    /// in the closest committed block, i.e., before the nearest `Runtime::after_commit()`.
    /// The dispatcher routes transactions and `before_transactions` / `after_transactions`
    /// events to the service only after `update_service_status()` is called with the same instance
    /// specification.
    ///
    /// The runtime should discard the instantiated service instance after completing this method.
    /// Otherwise, if the service is successfully committed in the block, it will duplicate the one
    /// instantiated in the runtime. There may be compelling reasons for the runtime to retain
    /// the instantiated service. For example, if creating an instance takes very long time.
    /// In this case, the "garbage" services may be removed from the runtime in `after_commit`
    /// because of the time dependence between `update_service_status` and `after_commit` described above.
    ///
    /// The runtime should commit long-term resources for the service only after the
    /// `update_service_status()` call. In other words, the runtime must be sure that the service
    /// has been committed to the blockchain.
    ///
    /// # Return Value
    ///
    /// Returning an error is a signal of `Runtime` that the
    /// service instantiation has failed. As a rule of a thumb, changes made by the
    /// `initiate_adding_service` method will be rolled back after such a signal. The exact logic of
    /// the rollback is determined by the supervisor.
    ///
    /// An error is one of the expected / handled outcomes of the service instantiation procedure.
    /// Thus, verifying prerequisites
    /// for instantiation and reporting corresponding failures should be performed at this stage
    /// rather than in `update_service_status`.
    fn initiate_adding_service(
        &self,
        context: ExecutionContext<'_>,
        spec: &InstanceSpec,
        parameters: Vec<u8>,
    ) -> Result<(), ExecutionError>;

    /// Notifies runtime about changes of the service instance state.
    ///
    /// This method notifies runtime about a specific service instance state changes in the
    /// dispatcher. Runtime should perform corresponding actions in according to changes in
    /// the service instance state.
    ///
    /// Method is called for a specific service instance during the `Runtime` lifetime in the
    /// following cases:
    ///
    /// - For newly added instances, or modified existing this method is called when the fork
    ///   with the corresponding changes is committed.
    /// - After a node restart, the method is called for all existing service instances regardless
    ///   of their statuses.
    ///
    /// For newly added instances invocation of this method guarantees that
    /// `initiate_adding_service()` has been called with the same `spec` already and returned
    /// `Ok(())`. The results of the call (i.e., changes to the blockchain state) will be
    /// persisted from the call.
    ///
    /// # Arguments
    ///
    /// `snapshot` is a storage snapshot at the latest height when the method is called:
    ///
    /// - Suppose the service is committed during the node operation. Then `snapshot` is taken at the
    ///   moment the fork applies for which the corresponding `initiate_adding_service`
    ///   has been performed.
    /// - Suppose the service is stopped during the node operation. `Then `snapshot` is taken at
    ///   the moment the fork applies for which the corresponding request has been performed.
    /// - Suppose the service resumes after the node restart. Then `snapshot` is the storage state
    ///   at the node start.
    ///
    /// For the built-in services, on the first node start `snapshot` will not contain information
    /// on the genesis block. Thus, using some core APIs, like requesting the current
    /// blockchain height, will result in a panic.
    ///
    /// `status` is the resulting status of the service instance.
    ///
    /// # Return value
    ///
    /// An error or panic returned from this method will not be processed and will lead
    /// to the node stopping. Thus, a runtime should only return an error / panic if the error is local
    /// to the node or affects less than 1/3 of nodes in the network. The error should contain a
    /// description allowing the node administrator to determine
    /// the root cause of the error and recover the node(s) by eliminating the cause.
    ///
    /// This error / panic must not be common to all nodes in the network.
    /// The errors common for the whole network should be produced during the preceding
    /// `initiate_adding_service` call.
    fn update_service_status(
        &mut self,
        snapshot: &dyn Snapshot,
        spec: &InstanceSpec,
        status: InstanceStatus,
    ) -> Result<(), ExecutionError>;

    /// Dispatches payload to the method of a specific service instance.
    ///
    /// # Arguments
    ///
    /// The service instance name and method ID are provided in the `call_info` argument and
    /// the interface name is provided as the corresponding field of the `context` argument.
    ///
    /// A blank interface name denotes the "default" interface; it should be supported by all
    /// services. The methods of the default interface are defined by the service artifact
    /// and thus may have different signatures for different services.
    ///
    /// A non-empty interface name denotes an interface defined externally to the service instance.
    /// In this case, the name is a Protobuf flavor of a fully qualified name
    /// (e.g., `exonum.Configure`). And the method signatures can be inferred from the name
    /// using an interface definition.
    ///
    /// **Note**. Support of non-default interfaces is experimental; as such, an IDL for them
    /// is not stabilized yet.
    ///
    /// # Return Value
    ///
    /// - If the service does not implement an interface, returns a `NoSuchInterface` error.
    /// - If the interface does not have a method, returns a `NoSuchMethod` error.
    ///
    /// An error returned from this method will lead to the rollback of all changes
    /// in the fork enclosed in the `context`.
    fn execute(
        &self,
        context: ExecutionContext<'_>,
        call_info: &CallInfo,
        arguments: &[u8],
    ) -> Result<(), ExecutionError>;

    /// Notifies a service stored in the present runtime about the beginning of the block. Allows
    /// the service to modify the blockchain state before any transaction in the block is processed.
    ///
    /// `before_transactions` is called for every service active at the beginning of the block
    /// exactly once for each block. Services that will be instantiated within the block do **not**
    /// receive a call. The method is not called for the genesis block.
    ///
    /// # Return Value
    ///
    /// An error returned from this method will lead to the rollback of all changes
    /// in the fork enclosed in the `context`.
    fn before_transactions(
        &self,
        context: ExecutionContext<'_>,
        instance_id: InstanceId,
    ) -> Result<(), ExecutionError>;

    /// Notifies a service stored in this runtime about the end of the block. Allows the method
    /// to modify the blockchain state after all transactions in the block are processed.
    ///
    /// `after_transactions` is called for every service active at the beginning of the block
    /// exactly once for each block. Services instantiated within the block do **not** receive a call.
    /// Services instantiated within genesis block are activated **immediately** and
    /// thus `after_transactions` is invoked for them in the genesis block.
    ///
    /// # Return value
    ///
    /// An error returned from this method will lead to the rollback of all changes
    /// in the fork enclosed in the `context`.
    fn after_transactions(
        &self,
        context: ExecutionContext<'_>,
        instance_id: InstanceId,
    ) -> Result<(), ExecutionError>;

    /// Notifies the runtime about commit of a new block.
    ///
    /// This method is called *after* all `update_service_status` calls related
    /// to the same block. The method is called exactly once for each block in the blockchain,
    /// including the genesis block.
    ///
    /// A block is not yet persisted when this method is called. The `snapshot` provides an up-to-date
    /// block information. It corresponds exactly to the information
    /// eventually persisted.
    ///
    /// `mailbox` is used to send async commands to the dispatcher. This mechanism is used, e.g.,
    /// by the supervisor service to enqueue artifact deployment. A runtime may ignore `mailbox`
    /// if its services (or the runtime itself) do not require privileged access to the dispatcher.
    fn after_commit(&mut self, snapshot: &dyn Snapshot, mailbox: &mut Mailbox);

    /// Notifies the runtime that it has to shutdown.
    ///
    /// This callback is invoked sequentially for each runtime just before the node shutdown.
    /// Thus, the runtimes can stop themselves gracefully.
    ///
    /// Invoking this callback is the last operation for the runtime.
    /// This method is a part of shutdown process. Thus, the runtimes can block and perform
    /// heavy operations here if needed.
    fn shutdown(&mut self) {}
}

impl<T: Runtime> From<T> for Box<dyn Runtime> {
    fn from(value: T) -> Self {
        Box::new(value)
    }
}

/// Specifies a system identifier for [`Runtime`].
pub trait WellKnownRuntime: Runtime {
    /// Identifier of the present runtime.
    const ID: u32;
}

// TODO: Rethink visibility [ECR-3913]
#[derive(Debug)]
/// Instance of [`Runtime`] with the corresponding ID.
pub struct RuntimeInstance {
    /// Identifier of the present runtime.
    pub id: u32,
    /// Instance of [`Runtime`].
    pub instance: Box<dyn Runtime>,
}

impl<T: WellKnownRuntime> From<T> for RuntimeInstance {
    fn from(runtime: T) -> Self {
        RuntimeInstance {
            id: T::ID,
            instance: runtime.into(),
        }
    }
}

/// The initiator of the method execution.
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum Caller {
    /// A usual transaction from the Exonum client authorized by its key pair.
    Transaction {
        /// Hash of the transaction message.
        hash: Hash,
        /// Public key of the user who signed this transaction.
        author: PublicKey,
    },

    /// Method is invoked during the method execution of a different service.
    Service {
        /// Identifier of the service instance which invoked the present method.
        instance_id: InstanceId,
    },

    /// Call is invoked by one of the blockchain lifecycle events.
    ///
    /// This kind of authorization is used for `before_transactions` / `after_transactions`
    /// calls to the service instances, and for initialization of built-in services.
    Blockchain,
}

impl Caller {
    /// Returns the author's public key, if it exists.
    pub fn author(&self) -> Option<PublicKey> {
        self.as_transaction().map(|(_hash, author)| author)
    }

    /// Return the transaction hash, if it exists.
    pub fn transaction_hash(&self) -> Option<Hash> {
        self.as_transaction().map(|(hash, _)| hash)
    }

    /// Tries to reinterpret the caller as an authorized transaction.
    pub fn as_transaction(&self) -> Option<(Hash, PublicKey)> {
        if let Caller::Transaction { hash, author } = self {
            Some((*hash, *author))
        } else {
            None
        }
    }

    /// Tries to reinterpret the caller as a service.
    pub fn as_service(&self) -> Option<InstanceId> {
        if let Caller::Service { instance_id } = self {
            Some(*instance_id)
        } else {
            None
        }
    }

    /// Verifies that the caller of this method is a supervisor service.
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

/// Provides the current state of the blockchain and the caller information for the transaction
/// which is being executed.
#[derive(Debug)]
pub struct ExecutionContext<'a> {
    /// The current state of the blockchain. It includes the new, not-yet-committed, changes to
    /// the database made by the previous transactions already executed in this block.
    pub fork: &'a mut Fork,
    /// The initiator of the transaction execution.
    pub caller: Caller,
    /// Identifier of the service interface required for the call. Keep in mind that this field, in
    /// fact, is a part of an unfinished "interfaces" feature. It will be replaced in future releases.
    /// At the moment this field is always empty for the primary service interface.
    pub interface_name: &'a str,
    /// Reference to the dispatcher.
    dispatcher: &'a Dispatcher,
    /// Depth of the call stack.
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

    pub(crate) fn child_context(
        &mut self,
        caller_service_id: Option<InstanceId>,
    ) -> ExecutionContext<'_> {
        ExecutionContext {
            caller: caller_service_id
                .map(|instance_id| Caller::Service { instance_id })
                .unwrap_or(self.caller),
            dispatcher: self.dispatcher,
            fork: self.fork,
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

        let (runtime_id, runtime) = self
            .dispatcher
            .runtime_for_service(call_info.instance_id)
            .ok_or(DispatcherError::IncorrectRuntime)?;
        let reborrowed = self.reborrow_with_interface(interface_name);
        runtime
            .execute(reborrowed, call_info, arguments)
            .map_err(|mut err| {
                err.set_runtime_id(runtime_id).set_call_site(|| CallSite {
                    instance_id: call_info.instance_id,
                    call_type: CallType::Method {
                        interface: interface_name.to_owned(),
                        id: call_info.method_id,
                    },
                });
                err
            })
    }

    /// Initiates adding a new service instance to the blockchain. The created service is not active
    /// (i.e., does not process transactions or the `after_transactions` hook)
    /// until the block built on top of the provided `fork` is committed.
    ///
    /// This method should be called for the exact context passed to the runtime.
    pub(crate) fn initiate_adding_service(
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
        runtime
            .initiate_adding_service(self.reborrow(), &spec, constructor.into_bytes())
            .map_err(|mut err| {
                err.set_runtime_id(spec.artifact.runtime_id)
                    .set_call_site(|| CallSite {
                        instance_id: spec.id,
                        call_type: CallType::Constructor,
                    });
                err
            })?;

        // Add a service instance to the dispatcher schema.
        DispatcherSchema::new(&*self.fork)
            .initiate_adding_service(spec)
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

/// Instance descriptor contains information to access the running service instance.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct InstanceDescriptor<'a> {
    /// A unique numeric ID of the service instance.
    /// [Read more.](struct.InstanceSpec.html#structfield.id)
    pub id: InstanceId,
    /// A unique name of the service instance.
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

impl<'a> From<(InstanceId, &'a str)> for InstanceDescriptor<'a> {
    fn from((id, name): (InstanceId, &'a str)) -> Self {
        InstanceDescriptor { id, name }
    }
}
