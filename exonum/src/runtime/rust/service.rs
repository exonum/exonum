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
    proto::Any,
    runtime::{
        api::ServiceApiBuilder,
        dispatcher::{self, Dispatcher, DispatcherSender},
        error::ExecutionError,
        AnyTx, ArtifactProtobufSpec, CallContext, CallInfo, Caller, ExecutionContext,
        InstanceDescriptor, InstanceId, MethodId,
    },
};

use super::RustArtifactId;

pub trait ServiceDispatcher: Send {
    fn call(
        &self,
        interface_name: &str,
        method: MethodId,
        ctx: TransactionContext,
        payload: &[u8],
    ) -> Result<(), ExecutionError>;
}

pub trait Service: ServiceDispatcher + Debug + 'static {
    fn configure(
        &self,
        _instance: InstanceDescriptor,
        _fork: &Fork,
        _params: Any,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn state_hash(&self, _descriptor: InstanceDescriptor, _snapshot: &dyn Snapshot) -> Vec<Hash>;

    fn before_commit(&self, _context: BeforeCommitContext) {}

    fn after_commit(&self, _context: AfterCommitContext) {}

    fn wire_api(&self, _builder: &mut ServiceApiBuilder) {}
    // TODO: add other hooks such as "on node startup", etc.
}

pub trait ServiceFactory: Send + Debug + 'static {
    fn artifact_id(&self) -> RustArtifactId;

    fn artifact_protobuf_spec(&self) -> ArtifactProtobufSpec;

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

    /// Create unsigned service transaction from the value.
    fn into_any_tx(self, instance_id: InstanceId) -> AnyTx {
        AnyTx {
            call_info: CallInfo {
                instance_id,
                method_id: Self::METHOD_ID,
            },
            arguments: self.into_bytes(),
        }
    }

    /// Sign the value as a transaction with the specified instance identifier.
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
    /// Create a new transaction context for the specified execution context and the instance descriptor.
    pub(crate) fn new(context: &'a ExecutionContext<'b>, instance: InstanceDescriptor<'a>) -> Self {
        Self {
            inner: context,
            instance,
        }
    }

    /// Return the writable snapshot of the current blockchain state.
    pub fn fork(&self) -> &Fork {
        self.inner.fork
    }

    /// Return the initiator of the actual transaction execution.
    pub fn caller(&self) -> &Caller {
        &self.inner.caller
    }

    /// Return validator ID if the transaction author is validator.
    pub fn validator_id(&self) -> Option<ValidatorId> {
        // TODO Perhaps we should optimize this method [ECR-3222]
        self.caller().author().and_then(|author| {
            CoreSchema::new(self.fork())
                .consensus_config()
                .find_validator(|validator_keys| author == validator_keys.service_key)
        })
    }

    /// Enqueue dispatcher action.
    pub(crate) fn dispatch_action(&self, action: dispatcher::Action) {
        self.inner.dispatch_action(action)
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
    /// Create a context to call interfaces of the specified service instance.
    pub fn call_context(&self, called: InstanceId) -> CallContext<'a> {
        CallContext::new(self.inner, self.instance.id, called)
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
    dispatcher: &'a Dispatcher,
}

impl<'a> BeforeCommitContext<'a> {
    /// Create a new `BeforeCommit` context.
    pub(crate) fn new(
        instance: InstanceDescriptor<'a>,
        fork: &'a Fork,
        dispatcher: &'a Dispatcher,
    ) -> Self {
        Self {
            instance,
            fork,
            dispatcher,
        }
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
    /// Create a new `AfterCommit` context.
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

    /// Return a validator ID if the current node is validator.
    pub fn validator_id(&self) -> Option<ValidatorId> {
        // TODO Perhaps we should optimize this method [ECR-3222]
        CoreSchema::new(self.snapshot)
            .consensus_config()
            .find_validator(|validator_keys| self.service_keypair.0 == validator_keys.service_key)
    }

    /// Return a current blockchain height. This height is "height of the last committed block".
    pub fn height(&self) -> Height {
        // TODO Perhaps we should optimize this method [ECR-3222]
        CoreSchema::new(self.snapshot).height()
    }

    /// Sign and broadcast transaction to other nodes in the network.
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

    /// Broadcast transaction to the other nodes in the network.
    /// This transaction should be signed externally.
    pub fn broadcast_signed_transaction(&self, msg: Verified<AnyTx>) {
        if let Err(e) = self.tx_sender.broadcast_transaction(msg) {
            error!("Couldn't broadcast transaction {}.", e);
        }
    }

    /// Return a communication channel with the dispatcher.
    pub(crate) fn dispatcher_channel(&self) -> &DispatcherSender {
        self.dispatcher
    }

    /// Return a transaction broadcaster.
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
    /// Fully qualified name of this interface for the [call info].
    ///
    /// [call info]: ../struct.CallInfo.html
    const NAME: &'static str;
    /// Invoke the specified method handler of the service instance.
    fn dispatch(
        &self,
        ctx: TransactionContext,
        method: MethodId,
        payload: &[u8],
    ) -> Result<(), ExecutionError>;
}
