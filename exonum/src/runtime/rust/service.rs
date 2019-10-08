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

use exonum_merkledb::{BinaryValue, Fork, Snapshot};

use std::fmt::{self, Debug};

use crate::{
    blockchain::Schema as CoreSchema,
    crypto::{Hash, PublicKey, SecretKey},
    helpers::{Height, ValidatorId},
    messages::Verified,
    node::ApiSender,
    runtime::{
        api::ServiceApiBuilder,
        dispatcher::{self, DispatcherRef, DispatcherSender},
        error::ExecutionError,
        AnyTx, ArtifactProtobufSpec, CallContext, CallInfo, Caller, ConfigChange, ExecutionContext,
        InstanceDescriptor, InstanceId, MethodId,
    },
};

use super::RustArtifactId;

/// Describes how the service instance should dispatch concrete method calls
/// (taking into account the interface to which method belongs).
///
/// Usually, it can be derived using a `ServiceFactory` macro.
pub trait ServiceDispatcher: Send {
    /// Dispatches the interface method call within the specified context.
    fn call(
        &self,
        interface_name: &str,
        method: MethodId,
        ctx: TransactionContext,
        payload: &[u8],
    ) -> Result<(), ExecutionError>;
}

/// Describes an Exonum service instance.
///
/// That is, it determines how the service instance responds to certain requests and events
/// from the runtime.
pub trait Service: ServiceDispatcher + Debug + 'static {
    /// Initializes a new service instance with the given parameters.
    ///
    /// This method is called once after creating a new service instance.
    ///
    /// The parameters passed to the method are not saved by the framework
    /// automatically, hence the user must take care of it if needed.
    fn initialize(
        &self,
        _instance: InstanceDescriptor,
        _fork: &Fork,
        _params: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    /// Returns a list of root hashes of Merkelized tables defined by the provided instance,
    /// as of the given snapshot of the blockchain state.
    ///
    /// The core uses this list to [aggregate][1] hashes of tables defined by every service into a
    /// single Merkelized meta-map.
    /// The hash of this meta-map is considered as the hash of the entire blockchain [state][2] and
    /// is recorded as such in blocks and Precommit messages.
    ///
    /// [See also.][3]
    ///
    /// [1]: ../struct.StateHashAggregator.html
    /// [2]: ../../blockchain/struct.Block.html#structfield.state_hash
    /// [3]: ../../blockchain/struct.Schema.html#method.state_hash_aggregator
    fn state_hash(&self, instance: InstanceDescriptor, snapshot: &dyn Snapshot) -> Vec<Hash>;

    /// Performs storage operations on behalf of the service before committing the block.
    ///
    /// Any changes of storage state will affect `state_hash`, which means this method must
    /// act the same on different nodes. In other words service should only use data available
    /// in the provided `BeforeCommitContext`.
    ///
    /// The order of invoking `before_commit` method is implementation detail. Effectively,
    /// this means that services must not rely on a particular ordering of `Service::before_commit`
    /// invocations.
    fn before_commit(&self, _context: BeforeCommitContext) {}
    /// Handles block commit event.
    ///
    /// This handler is an optional callback method which is invoked by the blockchain
    /// after each block commit. For example, a service can create one or more transactions
    /// if a specific condition has occurred.
    ///
    /// *Try not to perform long operations in this handler*.
    fn after_commit(&self, _context: AfterCommitContext) {}

    /// Attaches the service API request handlers to the Exonum API schema.
    ///
    /// The request handlers are mounted on the `/api/services/{instance_name}` path at the
    /// listen address of every full node in the blockchain network.
    fn wire_api(&self, _builder: &mut ServiceApiBuilder) {}

    // TODO: add other hooks such as "on node startup", etc. [ECR-3222]
}

/// Describes service instance factory for the specific Rust artifact.
pub trait ServiceFactory: Send + Debug + 'static {
    /// Returns the unique artifact identifier corresponding to the factory.
    fn artifact_id(&self) -> RustArtifactId;
    /// Returns the protobuf specification used by instances of this service.
    fn artifact_protobuf_spec(&self) -> ArtifactProtobufSpec;
    /// Creates a new service instance.
    fn create_instance(&self) -> Box<dyn Service>;
}

impl<T> From<T> for Box<dyn ServiceFactory>
where
    T: ServiceFactory,
{
    fn from(factory: T) -> Self {
        Box::new(factory) as Self
    }
}

/// Transaction specification for a specific service interface.
pub trait Transaction: BinaryValue {
    /// Service interface associated with the given transaction.
    type Service;
    /// Identifier of the service interface required for the call.
    #[doc(hidden)]
    const INTERFACE_NAME: &'static str;
    /// Identifier of the service method which executes the given transaction.
    const METHOD_ID: MethodId;

    /// Creates unsigned service transaction from the value.
    fn into_any_tx(self, instance_id: InstanceId) -> AnyTx {
        AnyTx {
            call_info: CallInfo {
                instance_id,
                method_id: Self::METHOD_ID,
            },
            arguments: self.into_bytes(),
        }
    }

    /// Signs the value as a transaction with the specified instance identifier.
    fn sign(
        self,
        service_id: InstanceId,
        public_key: PublicKey,
        secret_key: &SecretKey,
    ) -> Verified<AnyTx> {
        Verified::from_value(self.into_any_tx(service_id), public_key, secret_key)
    }
}

/// Provide the context for the transaction under execution.
#[derive(Debug)]
pub struct TransactionContext<'a, 'b> {
    /// Service instance associated with the current context.
    pub instance: InstanceDescriptor<'a>,
    /// Underlying execution context.
    inner: &'a ExecutionContext<'b>,
}

impl<'a, 'b> TransactionContext<'a, 'b> {
    /// Creates a new transaction context for the specified execution context and the instance
    /// descriptor.
    pub(crate) fn new(context: &'a ExecutionContext<'b>, instance: InstanceDescriptor<'a>) -> Self {
        Self {
            inner: context,
            instance,
        }
    }

    /// Returns the writable snapshot of the current blockchain state.
    pub fn fork(&self) -> &Fork {
        self.inner.fork
    }

    /// Returns the initiator of the actual transaction execution.
    pub fn caller(&self) -> &Caller {
        &self.inner.caller
    }

    /// Returns validator ID if the transaction author is validator.
    pub fn validator_id(&self) -> Option<ValidatorId> {
        // TODO Perhaps we should optimize this method [ECR-3222]
        self.caller().author().and_then(|author| {
            CoreSchema::new(self.fork())
                .consensus_config()
                .find_validator(|validator_keys| author == validator_keys.service_key)
        })
    }

    /// Enqueue dispatcher action.
    pub fn dispatch_action(&self, action: dispatcher::Action) {
        self.inner
            .dispatcher
            .dispatch_action(self.instance.id, action)
    }

    // TODO This method is hidden until it is fully tested in next releases. [ECR-3493]
    #[doc(hidden)]
    /// Creates a client to call interface methods of the specified service instance.
    pub fn interface<T>(&self, called: InstanceId) -> T
    where
        T: From<CallContext<'a>>,
    {
        self.call_context(called).into()
    }

    // TODO This method is hidden until it is fully tested in next releases. [ECR-3493]
    #[doc(hidden)]
    /// Creates a context to call interfaces of the specified service instance.
    pub fn call_context(&self, called: InstanceId) -> CallContext<'a> {
        CallContext::from_execution_context(self.inner, self.instance.id, called)
    }

    /// Checks the caller of this method with the specified closure.
    ///
    /// If the closure returns `Some(value)`, then the method returns `Some((value, fork))` thus you
    /// get a write access to the blockchain state. Otherwise this method returns
    /// an occurred error.
    pub fn verify_caller<F, T>(&self, predicate: F) -> Option<(T, &Fork)>
    where
        F: Fn(&Caller) -> Option<T>,
    {
        // TODO Think about returning structure with the named fields instead of unnamed tuple
        // to make code more clear. [ECR-3222]
        predicate(&self.inner.caller).map(|result| (result, self.inner.fork))
    }
}

/// Provide context for the `before_commit` handler.
#[derive(Debug)]
pub struct BeforeCommitContext<'a> {
    /// Service instance associated with the current context.
    pub instance: InstanceDescriptor<'a>,
    /// The current state of the blockchain. It includes the new, not-yet-committed, changes to
    /// the database made by the previous transactions already executed in this block.
    pub fork: &'a Fork,
    /// Reference to the underlying runtime dispatcher.
    dispatcher: &'a DispatcherRef<'a>,
}

impl<'a> BeforeCommitContext<'a> {
    /// Creates a new `BeforeCommit` context.
    pub(crate) fn new(
        instance: InstanceDescriptor<'a>,
        fork: &'a Fork,
        dispatcher: &'a DispatcherRef<'a>,
    ) -> Self {
        Self {
            instance,
            fork,
            dispatcher,
        }
    }

    // TODO This method is hidden until it is fully tested in next releases. [ECR-3493]
    #[doc(hidden)]
    /// Create a client to call interface methods of the specified service instance.
    pub fn interface<T>(&self, called: InstanceId) -> T
    where
        T: From<CallContext<'a>>,
    {
        self.call_context(called).into()
    }

    // TODO This method is hidden until it is fully tested in next releases. [ECR-3493]
    #[doc(hidden)]
    /// Creates a context to call interfaces of the specified service instance.
    pub fn call_context(&self, called: InstanceId) -> CallContext<'a> {
        CallContext::new(self.fork, self.dispatcher, self.instance.id, called)
    }

    /// Adds a configuration update to pending actions. These changes will be applied immediately
    /// before the block commit.
    ///
    /// Only the supervisor service is allowed to perform this action.
    #[doc(hidden)]
    pub fn update_config(&self, changes: Vec<ConfigChange>) {
        self.dispatcher.dispatch_action(
            self.instance.id,
            dispatcher::Action::UpdateConfig {
                caller_instance_id: self.instance.id,
                changes,
            },
        )
    }
}

/// Provide context for the `after_commit` handler.
pub struct AfterCommitContext<'a> {
    /// Service instance associated with the current context.
    pub instance: InstanceDescriptor<'a>,
    /// Read-only snapshot of the current blockchain state.
    pub snapshot: &'a dyn Snapshot,
    /// Service key pair of the current node.
    pub service_keypair: &'a (PublicKey, SecretKey),
    /// Channel to communicate with the dispatcher.
    dispatcher: &'a DispatcherSender,
    /// Channel to send signed transactions to the transactions pool.
    tx_sender: &'a ApiSender,
}

impl<'a> AfterCommitContext<'a> {
    /// Creates a new `AfterCommit` context.
    pub(crate) fn new(
        instance: InstanceDescriptor<'a>,
        snapshot: &'a dyn Snapshot,
        dispatcher: &'a DispatcherSender,
        service_keypair: &'a (PublicKey, SecretKey),
        tx_sender: &'a ApiSender,
    ) -> Self {
        Self {
            dispatcher,
            instance,
            snapshot,
            service_keypair,
            tx_sender,
        }
    }

    /// Returns a validator ID if the current node is validator.
    pub fn validator_id(&self) -> Option<ValidatorId> {
        // TODO Perhaps we should optimize this method [ECR-3222]
        CoreSchema::new(self.snapshot)
            .consensus_config()
            .find_validator(|validator_keys| self.service_keypair.0 == validator_keys.service_key)
    }

    /// Returns a current blockchain height. This height is "height of the last committed block".
    pub fn height(&self) -> Height {
        // TODO Perhaps we should optimize this method [ECR-3222]
        CoreSchema::new(self.snapshot).height()
    }

    /// Signs and broadcasts transaction to other nodes in the network.
    pub fn broadcast_transaction(&self, tx: impl Transaction) {
        let msg = tx.sign(
            self.instance.id,
            self.service_keypair.0,
            &self.service_keypair.1,
        );
        if let Err(e) = self.tx_sender.broadcast_transaction(msg) {
            error!("Couldn't broadcast transaction {}.", e);
        }
    }

    /// Broadcasts transaction to the other nodes in the network.
    /// This transaction should be signed externally.
    pub fn broadcast_signed_transaction(&self, msg: Verified<AnyTx>) {
        if let Err(e) = self.tx_sender.broadcast_transaction(msg) {
            error!("Couldn't broadcast transaction {}.", e);
        }
    }

    /// Returns a communication channel with the dispatcher.
    pub fn dispatcher_channel(&self) -> &DispatcherSender {
        self.dispatcher
    }

    /// Returns a transaction broadcaster.
    pub fn transaction_broadcaster(&self) -> ApiSender {
        self.tx_sender.clone()
    }
}

impl<'a> Debug for AfterCommitContext<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("AfterCommitContext")
            .field("instance", &self.instance)
            .finish()
    }
}

/// A service interface specification.
pub trait Interface {
    /// Fully qualified name of this interface.
    const INTERFACE_NAME: &'static str;
    /// Invokes the specified method handler of the service instance.
    fn dispatch(
        &self,
        context: TransactionContext,
        method: MethodId,
        payload: &[u8],
    ) -> Result<(), ExecutionError>;
}
