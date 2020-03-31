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

//! Common building blocks that compose runtimes for the Exonum blockchain.
//!
//! Each runtime contains specific services that execute transactions, process events,
//! provide user APIs, etc. A unified *dispatcher* redirects all the calls
//! and requests to an appropriate runtime environment. Thus, a blockchain interacts with the
//! dispatcher, and not with specific runtime instances.
//!
//! # Artifacts
//!
//! An artifact creates service instances similar to classes in object-oriented programming.
//! Artifacts reflect the assumption that deploying business logic onto the blockchain
//! may take a long time, may fail, end up with differing results on different nodes, etc.
//! Thus, artifacts decouple the complex *deployment* of the business logic from its instantiation
//! (which we assume is simple / non-fallible).
//!
//! Depending on the runtime, an artifact may have an additional specification required
//! for its deployment; e.g., files to be compiled.
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
//! 6. If the artifact is not associated with any services, it can be *unloaded*. Unloading
//!   the artifact may free resources associated with it in the corresponding runtime.
//!   Like other lifecycle events, unloading an artifact is controlled by the supervisor service.
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
//!   by their IDs; both numeric and string IDs are unique within a blockchain. (Note that
//!   the transition to the "active" state is not immediate;
//!   see [*Service State Transitions*](#service-state-transitions) section below.)
//!
//! 4. Active service instances can be stopped or frozen by a corresponding request to the dispatcher.
//!
//! The dispatcher is responsible for persisting artifacts and services across node restarts.
//!
//! A **stopped** service no longer participates in business logic, i.e.,
//! it does not process transactions or hooks, and does not interact with the users
//! in any way. Service data becomes unavailable for the other services,
//! but still exists. The service name and identifier remain reserved
//! for the stopped service and can't be used again for adding new services.
//!
//! **Frozen** service state is similar to the stopped one, except the service
//! state can be read both by internal readers (other services) and external ones
//! (HTTP API handlers).
//!
//! ## Service Hooks
//!
//! Each active service is called before any transactions in the block are processed;
//! we call this `before_transactions` hook. The service may modify the blockchain state in this hook.
//! Likewise, each active service is called after all transactions in the block have been processed
//! (we call this `after_transactions` hook). These calls are quite similar to transactions:
//!
//! - Each call is isolated
//! - Service logic may return an error, meaning that all state changes made within the hook
//!   are rolled back
//! - The service may call other services within the hook
//!
//! ## Service State Transitions
//!
//! Transitions between service states (including service creation) occur once the block
//! with the transition is committed; the effect of a transition is not immediate. This means
//! that, for example, an instantiated service cannot process transactions or internal calls
//! in the block with instantiation, but can in the following block. Likewise, the service hooks
//! (`before_transactions` / `after_transactions`) are *not* called in the block with service
//! instantiation.
//!
//! When the service is stopped or frozen, the reverse is true:
//!
//! - The service continues processing transactions until the end of the block containing
//!   the stop command
//! - The service hooks *are* called for the service in this block
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
//!   of transactions, transactions in this block are passed to the dispatcher for execution.
//!
//! 5. The dispatcher uses a lookup table to find the corresponding [`Runtime`] for each transaction
//!   by the [`instance_id`] recorded in the transaction message. If the corresponding runtime exists,
//!   the dispatcher passes the transaction into this runtime for immediate [execution].
//!
//! 6. After execution the transaction [execution status] is written into the blockchain.
//!
//! # Data Migration Lifecycle
//!
//! Service data can be migrated to a newer version of the service artifact.
//! See [`migrations` module docs] for details.
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
//! # See Also
//!
//! - [Article on service lifecycle in general docs][docs:lifecycle]
//! - [Blog article on service lifecycle][blog:lifecycle]
//!
//! [`AnyTx`]: struct.AnyTx.html
//! [`CallInfo`]: struct.CallInfo.html
//! [`instance_id`]: struct.CallInfo.html#structfield.instance_id
//! [`Runtime`]: trait.Runtime.html
//! [execution]: trait.Runtime.html#execute
//! [execution status]: struct.ExecutionStatus.html
//! [artifacts]: struct.ArtifactId.html
//! [`migrations` module docs]: migrations/index.html
//! [`SUPERVISOR_INSTANCE_ID`]: constant.SUPERVISOR_INSTANCE_ID.html
//! [`Mailbox`]: struct.Mailbox.html
//! [`ExecutionError`]: struct.ExecutionError.html
//! [`instance_id`]: struct.CallInfo.html#structfield.method_id
//! [docs:lifecycle]: https://exonum.com/doc/version/latest/architecture/service-lifecycle/
//! [blog:lifecycle]: https://medium.com/meetbitfury/about-service-lifecycles-in-exonum-58c67678c6bb

pub use self::{
    blockchain_data::{BlockchainData, SnapshotExt},
    dispatcher::{
        remove_local_migration_result, Action as DispatcherAction, Mailbox,
        Schema as DispatcherSchema,
    },
    error::{
        catch_panic, CallSite, CallType, CommonError, CoreError, ErrorKind, ErrorMatch,
        ExecutionError, ExecutionFail, ExecutionStatus,
    },
    execution_context::{ExecutionContext, ExecutionContextUnstable, SupervisorExtensions},
    types::{
        AnyTx, ArtifactId, ArtifactSpec, ArtifactState, ArtifactStatus, CallInfo, Caller,
        CallerAddress, InstanceId, InstanceQuery, InstanceSpec, InstanceState, InstanceStatus,
        MethodId, MigrationStatus,
    },
};
pub(crate) use self::{dispatcher::Dispatcher, error::ExecutionErrorAux};

pub mod migrations;
pub mod oneshot;
pub mod versioning;

use exonum_merkledb::Snapshot;
use semver::Version;

use std::fmt;

use self::migrations::{InitMigrationError, MigrationScript};
use crate::blockchain::Blockchain;

mod blockchain_data;
mod dispatcher;
pub(crate) mod error;
mod execution_context;
mod types;

/// Persistent identifier of a supervisor service instance.
///
/// Only a service with this ID can perform actions with the dispatcher.
pub const SUPERVISOR_INSTANCE_ID: InstanceId = 0;

/// List of predefined runtimes.
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
#[repr(u32)]
#[non_exhaustive]
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

impl RuntimeIdentifier {
    fn transform(id: u32) -> Result<Self, ()> {
        match id {
            0 => Ok(Self::Rust),
            1 => Ok(Self::Java),
            _ => Err(()),
        }
    }
}

impl fmt::Display for RuntimeIdentifier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rust => formatter.write_str("Rust runtime"),
            Self::Java => formatter.write_str("Java runtime"),
        }
    }
}

/// Optional features that may or may not be supported by a particular `Runtime`.
#[derive(Debug)]
#[non_exhaustive]
pub enum RuntimeFeature {
    /// Freezing services: disabling APIs mutating service state (e.g., transactions)
    /// while leaving read-only APIs switched on.
    FreezingServices,
}

impl fmt::Display for RuntimeFeature {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FreezingServices => formatter.write_str("freezing services"),
        }
    }
}

/// Runtime environment for Exonum services.
///
/// You can read more about the life cycle of services and transactions
/// [in the module docs](index.html#service-life-cycle).
///
/// Using this trait, you can extend the Exonum blockchain with the services written in
/// different languages.
///
/// # Stability
///
/// This trait is considered unstable; breaking changes may be introduced to it within
/// semantically non-breaking releases. However, it is guaranteed that such changes
/// will require reasonable amount of updates from the `Runtime` implementations.
///
/// # Call Ordering
///
/// Within the lifetime of a `Runtime`, calls to its methods have the following order:
///
/// ```text
/// LIFE ::= initialize (GENESIS | RESUME) BLOCK* shutdown
/// GENESIS ::=
///     deploy_artifact*
///     (initiate_adding_service update_service_status)*
///     after_commit
/// RESUME ::= (deploy_artifact | update_service_status | migrate)* on_resume
/// BLOCK* ::= PROPOSAL+ COMMIT
/// PROPOSAL ::=
///     (before_transactions CALL*)*
///     (execute CALL*)*
///     (after_transactions CALL*)*
/// CALL ::= execute | initiate_adding_service | initiate_resuming_service | migrate
/// COMMIT ::=
///     (deploy_artifact | unload_artifact)*
///     (update_service_status | migrate)*
///     after_commit
/// ```
///
/// `before_transactions`, `execute` and `after_transactions` handlers may spawn
/// child calls among services; this is denoted as `CALL*` in the excerpt above. The child calls
/// are executed synchronously. See the [*Service Interaction*] article for more details.
///
/// The ordering for the "read-only" methods `is_artifact_deployed` and `is_supported` in relation
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
/// - `initiate_resuming_service`
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
/// errors, use the [`catch_panic`](fn.catch_panic.html) method.
///
/// [*Service Interaction*]: https://exonum.com/doc/version/latest/advanced/service-interaction/
#[allow(unused_variables)]
pub trait Runtime: Send + fmt::Debug + 'static {
    /// Initializes the runtime, providing a `Blockchain` instance for further use.
    ///
    /// Calling this method always takes place before calling any other `Runtime` methods.
    /// The `initialize` method is called *exactly once* during the `Runtime` lifetime.
    ///
    /// The default implementation does nothing.
    fn initialize(&mut self, blockchain: &Blockchain) {}

    /// Checks if the runtime supports an optional feature.
    ///
    /// This method can be called by the core before performing operations that might not
    /// be implemented in a runtime, or by the supervisor service in order to check that a potential
    /// service / artifact state transition can be handled by the runtime.
    ///
    /// An implementation should return `false` for all features the runtime does not recognize.
    /// The default implementation always returns `false`, i.e., signals that the runtime supports
    /// no optional features.
    fn is_supported(&self, feature: &RuntimeFeature) -> bool {
        false
    }

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

    /// Requests to deploy an artifact with the given identifier and an additional deploy
    /// specification.
    ///
    /// This method is called *once* for a specific artifact during the `Runtime` lifetime:
    ///
    /// - For newly added artifacts, the method is called as the supervisor service decides to deploy
    ///   the artifact.
    /// - After the node restart, the method is called for all the previously deployed artifacts.
    ///
    /// Core guarantees that there will be no request to deploy an artifact which is already deployed,
    /// thus runtime should not report an attempt to do so as `ExecutionError`, but should consider it
    /// a bug in core.
    fn deploy_artifact(&mut self, artifact: ArtifactId, deploy_spec: Vec<u8>) -> oneshot::Receiver;

    /// Returns `true` if the specified artifact is deployed in this runtime.
    fn is_artifact_deployed(&self, artifact: &ArtifactId) -> bool;

    /// Requests to unload an artifact with the given identifier. Unloading may free resources
    /// (e.g., RAM) associated with the artifact.
    ///
    /// The following invariants are guaranteed to hold when this call is performed:
    ///
    /// - The artifact is deployed
    /// - There are no services with any status associated with the artifact, either as
    ///   an artifact [responsible for service logic][assoc-artifact] or as a [migration target]
    ///   of the data migration in a service.
    ///
    /// The default implementation does nothing. While this may be inefficient, this implementation
    /// is logically sound. Indeed, the runtime retains resources associated with the artifact
    /// (until the node is restarted), but on the blockchain level, the artifact is considered
    /// unloaded.
    ///
    /// [assoc-artifact]: struct.InstanceState.html#method.associated_artifact
    /// [migration target]: migrations/struct.InstanceMigration.html#structfield.target
    fn unload_artifact(&mut self, artifact: &ArtifactId) {
        // The default implementation does nothing.
    }

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
    ///
    /// Core guarantees that there will be no request to start a service instance which is already running,
    /// thus runtime should not report an attempt to do so as `ExecutionError`, but should consider it
    /// a bug in core.
    fn initiate_adding_service(
        &self,
        context: ExecutionContext<'_>,
        artifact: &ArtifactId,
        parameters: Vec<u8>,
    ) -> Result<(), ExecutionError>;

    /// Resumes previously stopped service instance with the given specification and arguments.
    /// As an example, arguments can be used to update the service configuration.
    ///
    /// The dispatcher ensures that a service instance with the given specification has been
    /// previously stopped and has the proper artifact version and name.
    ///
    /// This method has the same workflow as [`initiate_adding_service`] method.
    /// The main difference is that `initiate_adding_service` should call the service
    /// `initialize` method and `initiate_resuming_service` should call the service `resume` method.
    ///
    /// [`initiate_adding_service`]: #tymethod.initiate_adding_service
    fn initiate_resuming_service(
        &self,
        context: ExecutionContext<'_>,
        artifact: &ArtifactId,
        parameters: Vec<u8>,
    ) -> Result<(), ExecutionError>;

    /// Notifies runtime about changes of the service instance state.
    ///
    /// This method notifies runtime about a specific service instance state changes in the
    /// dispatcher. Runtime should perform corresponding actions in according to changes in
    /// the service instance state.
    ///
    /// This method is called for a specific service instance during the `Runtime` lifetime in the
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
    /// This method does not return a value, meaning that any error occurred during this method execution
    /// is considered critical and should lead to the node stopping.
    ///
    /// It is assumed that if `initiate_adding_service` didn't return an error previously,
    /// the runtime is able to update service status and within normal conditions no error is
    /// expected to happen.
    fn update_service_status(&mut self, snapshot: &dyn Snapshot, state: &InstanceState);

    /// Gets the migration script to migrate the data of the service to the state usable
    /// by a newer version of the artifact.
    ///
    /// An implementation of this method should be idempotent, i.e., return the same script or error
    /// for the same input.
    ///
    /// # Invariants Ensured by the Caller
    ///
    /// - `new_artifact` is deployed in the runtime
    /// - `data_version < new_artifact.version`
    ///
    /// # Return Value
    ///
    /// - An error signals that the runtime does not know how to migrate the service
    ///   to a newer version.
    /// - `Ok(Some(_))` provides a script to execute against service data. After the script
    ///   is executed, [`data_version`] of the service will be updated to `end_version`
    ///   from the script. `end_version` does not need to correspond to the version of `new_artifact`,
    ///   or to a version of an artifact deployed on the blockchain in general.
    /// - `Ok(None)` means that the service does not require data migration. `data_version`
    ///   of the service will be updated to the version of `new_artifact` once the block
    ///   with the migration command is committed; see [*Service State Transitions*] for details.
    ///
    /// [`data_version`]: struct.InstanceState.html#field.data_version
    /// [*Service State Transitions*]: index.html#service-state-transitions
    fn migrate(
        &self,
        new_artifact: &ArtifactId,
        data_version: &Version,
    ) -> Result<Option<MigrationScript>, InitMigrationError>;

    /// Dispatches payload to the method of a specific service instance.
    ///
    /// The call is dispatched iff the service is considered active at the moment.
    /// See [*Service State Transitions*] for more details.
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
    ///
    /// [*Service State Transitions*]: index.html#service-state-transitions
    fn execute(
        &self,
        context: ExecutionContext<'_>,
        method_id: MethodId,
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
    fn before_transactions(&self, context: ExecutionContext<'_>) -> Result<(), ExecutionError>;

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
    fn after_transactions(&self, context: ExecutionContext<'_>) -> Result<(), ExecutionError>;

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
}

#[allow(clippy::use_self)] // false positive
impl<T: Runtime> From<T> for Box<dyn Runtime> {
    fn from(value: T) -> Self {
        Box::new(value)
    }
}

/// A subset of [`Runtime`]s with a well-known runtime identifier.
///
/// [`Runtime`]: trait.Runtime.html
pub trait WellKnownRuntime: Runtime {
    /// Identifier of the present runtime.
    const ID: u32;
}

// TODO: Rethink visibility [ECR-3913]
/// Instance of [`Runtime`] with the corresponding ID.
///
/// [`Runtime`]: trait.Runtime.html
#[derive(Debug)]
#[non_exhaustive]
pub struct RuntimeInstance {
    /// Identifier of the enclosed runtime.
    pub id: u32,
    /// Enclosed `Runtime` object.
    pub instance: Box<dyn Runtime>,
}

impl RuntimeInstance {
    /// Constructs a new `RuntimeInstance` object.
    pub fn new(id: u32, instance: Box<dyn Runtime>) -> Self {
        Self { id, instance }
    }
}

impl<T: WellKnownRuntime> From<T> for RuntimeInstance {
    fn from(runtime: T) -> Self {
        Self::new(T::ID, runtime.into())
    }
}

/// Instance descriptor contains information to access the running service instance.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct InstanceDescriptor {
    /// A unique numeric ID of the service instance.
    /// [Read more.](struct.InstanceSpec.html#structfield.id)
    pub id: InstanceId,
    /// A unique name of the service instance.
    /// [Read more.](struct.InstanceSpec.html#structfield.name)
    pub name: String,
}

impl InstanceDescriptor {
    /// Creates a new `InstanceDescriptor` object.
    pub fn new(id: InstanceId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
        }
    }
}

impl fmt::Display for InstanceDescriptor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}", self.id, self.name)
    }
}
