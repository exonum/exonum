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

use exonum_merkledb::{BinaryValue, Snapshot};
use futures::IntoFuture;

use std::fmt::{self, Debug};

use crate::{
    blockchain::Schema as CoreSchema,
    crypto::{Hash, PublicKey, SecretKey},
    helpers::{Height, ValidatorId},
    messages::Verified,
    node::ApiSender,
    runtime::{
        api::ServiceApiBuilder,
        dispatcher::{Action, Mailbox},
        AnyTx, ArtifactId, ArtifactProtobufSpec, CallInfo, ExecutionError, InstanceDescriptor,
        InstanceId, MethodId,
    },
};

use super::{CallContext, RustArtifactId};

/// Describes how the service instance should dispatch specific method calls
/// with consideration of the interface where the method belongs.
///
/// Usually, `ServiceDispatcher` can be derived using the `ServiceFactory` macro.
pub trait ServiceDispatcher: Send {
    /// Dispatches the interface method call within the specified context.
    fn call(
        &self,
        interface_name: &str,
        method: MethodId,
        ctx: CallContext,
        payload: &[u8],
    ) -> Result<(), ExecutionError>;
}

/// Describes an Exonum service instance.
///
/// That is, `Service` determines how a service instance responds to certain requests and events
/// from the runtime.
pub trait Service: ServiceDispatcher + Debug + 'static {
    /// Initializes a new service instance with the given parameters.
    ///
    /// This method is called once after creating a new service instance.
    ///
    /// The parameters passed to the method are not saved by the framework
    /// automatically, hence the user must do it manually, if needed.
    fn initialize(&self, _context: CallContext, _params: Vec<u8>) -> Result<(), ExecutionError> {
        Ok(())
    }

    /// Returns a list of root hashes of the Merkelized tables defined by the provided instance,
    /// based on the given snapshot of the blockchain state.
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
    /// Any changes of the storage state will affect `state_hash`, which means this method must
    /// act similarly on different nodes. In other words, the service should only use data available
    /// in the provided `BeforeCommitContext`.
    ///
    /// The order of invoking the `before_commit` method is an implementation detail. Effectively,
    /// this means that services must not rely on a particular ordering of `Service::before_commit`
    /// invocations.
    fn before_commit(&self, _context: CallContext) {}

    /// Handles block commit event.
    ///
    /// This handler is an optional callback method which is invoked by the blockchain
    /// after each block commit. For example, a service can create one or more transactions
    /// if a specific condition has occurred.
    ///
    /// *Try not to perform long operations in this handler*.
    fn after_commit(&self, _context: AfterCommitContext) {}

    /// Attaches the request handlers of the service API to the Exonum API schema.
    ///
    /// The request handlers are mounted on the `/api/services/{instance_name}` path at the
    /// listen address of every full node in the blockchain network.
    fn wire_api(&self, _builder: &mut ServiceApiBuilder) {}

    // TODO: add other hooks such as "on node startup", etc. [ECR-3222]
}

/// Describes a service instance factory for the specific Rust artifact.
pub trait ServiceFactory: Send + Debug + 'static {
    /// Returns the unique artifact identifier corresponding to the factory.
    fn artifact_id(&self) -> RustArtifactId;
    /// Returns the Protobuf specification used by the instances of this service.
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
pub trait Transaction<Svc: ?Sized>: BinaryValue {
    /// Identifier of the service interface required for the call.
    #[doc(hidden)]
    const INTERFACE_NAME: &'static str;
    /// Identifier of the service method which executes the given transaction.
    const METHOD_ID: MethodId;

    /// Creates an unsigned service transaction from the value.
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

/// Provide context for the `after_commit` handler.
pub struct AfterCommitContext<'a> {
    /// Service instance associated with the current context.
    pub instance: InstanceDescriptor<'a>,
    /// Reference to the dispatcher mailbox.
    mailbox: &'a mut Mailbox,
    /// Read-only snapshot of the current blockchain state.
    pub snapshot: &'a dyn Snapshot,
    /// Service key pair of the current node.
    pub service_keypair: &'a (PublicKey, SecretKey),
    /// Channel to send signed transactions to the transactions pool.
    tx_sender: &'a ApiSender,
}

impl<'a> AfterCommitContext<'a> {
    /// Creates a new `AfterCommit` context.
    pub(crate) fn new(
        mailbox: &'a mut Mailbox,
        instance: InstanceDescriptor<'a>,
        snapshot: &'a dyn Snapshot,
        service_keypair: &'a (PublicKey, SecretKey),
        tx_sender: &'a ApiSender,
    ) -> Self {
        Self {
            mailbox,
            instance,
            snapshot,
            service_keypair,
            tx_sender,
        }
    }

    /// Returns the validator ID if the current node is a validator.
    pub fn validator_id(&self) -> Option<ValidatorId> {
        // TODO Perhaps we should optimize this method [ECR-3222]
        CoreSchema::new(self.snapshot)
            .consensus_config()
            .find_validator(|validator_keys| self.service_keypair.0 == validator_keys.service_key)
    }

    /// Returns a current blockchain height. This height is "height of the latest committed block".
    pub fn height(&self) -> Height {
        // TODO Perhaps we should optimize this method [ECR-3222]
        CoreSchema::new(self.snapshot).height()
    }

    /// Signs and broadcasts a transaction to the other nodes in the network.
    pub fn broadcast_transaction<Svc: ?Sized>(&self, tx: impl Transaction<Svc>) {
        let msg = tx.sign(
            self.instance.id,
            self.service_keypair.0,
            &self.service_keypair.1,
        );
        if let Err(e) = self.tx_sender.broadcast_transaction(msg) {
            error!("Couldn't broadcast transaction {}.", e);
        }
    }

    /// Broadcasts a transaction to the other nodes in the network.
    /// This transaction should be signed externally.
    pub fn broadcast_signed_transaction(&self, msg: Verified<AnyTx>) {
        if let Err(e) = self.tx_sender.broadcast_transaction(msg) {
            error!("Couldn't broadcast transaction {}.", e);
        }
    }

    /// Returns a transaction broadcaster.
    pub fn transaction_broadcaster(&self) -> ApiSender {
        self.tx_sender.clone()
    }

    /// Provides a supervisor interface to an authorized instance.
    #[doc(hidden)]
    pub fn supervisor_extensions(&mut self) -> Option<SupervisorExtensions> {
        if !is_supervisor(self.instance.id) {
            return None;
        }
        Some(SupervisorExtensions {
            mailbox: &mut *self.mailbox,
        })
    }
}

#[derive(Debug)]
pub struct SupervisorExtensions<'a> {
    mailbox: &'a mut Mailbox,
}

impl SupervisorExtensions<'_> {
    /// Starts the deployment of an artifact. The provided callback is executed after
    /// the deployment is completed.
    pub fn start_deploy<F>(
        &mut self,
        artifact: ArtifactId,
        spec: impl BinaryValue,
        and_then: impl FnOnce() -> F + 'static,
    ) where
        F: IntoFuture<Item = (), Error = ExecutionError> + 'static,
    {
        let action = Action::StartDeploy {
            artifact,
            spec: spec.into_bytes(),
            and_then: Box::new(|| Box::new(and_then().into_future())),
        };
        self.mailbox.push(action);
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
        context: CallContext,
        method: MethodId,
        payload: &[u8],
    ) -> Result<(), ExecutionError>;
}

fn is_supervisor(instance_id: InstanceId) -> bool {
    instance_id == crate::runtime::SUPERVISOR_INSTANCE_ID
}
