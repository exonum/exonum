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

use exonum_merkledb::{access::Prefixed, BinaryValue, ObjectHash, Snapshot};
use failure::Error;
use futures::IntoFuture;

use std::{
    borrow::Cow,
    fmt::{self, Debug},
};

use crate::{
    blockchain::config::InstanceInitParams,
    crypto::{Hash, PublicKey, SecretKey},
    helpers::{Height, ValidatorId},
    messages::Verified,
    node::ApiSender,
    runtime::{
        dispatcher::{Action, Mailbox},
        AnyTx, ArtifactId, CallInfo, ExecutionError, InstanceDescriptor, InstanceId, MethodId,
    },
};

use super::{
    api::ServiceApiBuilder, ArtifactProtobufSpec, BlockchainData, CallContext, RustArtifactId,
};

/// Describes how the service instance should dispatch specific method calls
/// with consideration of the interface where the method belongs.
///
/// Usually, `ServiceDispatcher` can be derived using the
/// [`ServiceDispatcher`](index.html#examples) macro.
pub trait ServiceDispatcher: Send {
    /// Dispatches the interface method call within the specified context.
    fn call(
        &self,
        interface_name: &str,
        method: MethodId,
        ctx: CallContext<'_>,
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
    fn initialize(
        &self,
        _context: CallContext<'_>,
        _params: Vec<u8>,
    ) -> Result<(), ExecutionError> {
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
    fn state_hash(&self, _data: BlockchainData<&dyn Snapshot>) -> Vec<Hash>;

    /// Performs storage operations on behalf of the service before committing the block.
    ///
    /// Any changes of the storage state will affect `state_hash`, which means this method must
    /// act similarly on different nodes. In other words, the service should only use data available
    /// in the provided `BeforeCommitContext`.
    ///
    /// The order of invoking the `before_commit` method is an implementation detail. Effectively,
    /// this means that services must not rely on a particular ordering of `Service::before_commit`
    /// invocations.
    fn before_commit(&self, _context: CallContext<'_>) {}

    /// Handles block commit event.
    ///
    /// This handler is an optional callback method which is invoked by the blockchain
    /// after each block commit. For example, a service can create one or more transactions
    /// if a specific condition has occurred.
    ///
    /// *Try not to perform long operations in this handler*.
    fn after_commit(&self, _context: AfterCommitContext<'_>) {}

    /// Attaches the request handlers of the service API to the Exonum API schema.
    ///
    /// The request handlers are mounted on the `/api/services/{instance_name}` path at the
    /// listen address of every full node in the blockchain network.
    fn wire_api(&self, _builder: &mut ServiceApiBuilder) {}

    // TODO: add other hooks such as "on node startup", etc. [ECR-3222]
}

/// Describes a service instance factory for the specific Rust artifact.
///
/// Usually, `ServiceFactory` can be derived using the
/// [`ServiceFactory`](index.html#examples) macro.
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

/// Provides default instance configuration parameters for `ServiceFactory`.
pub trait DefaultInstance: ServiceFactory {
    /// Default id for a service.
    const INSTANCE_ID: InstanceId;
    /// Default name for a service.
    const INSTANCE_NAME: &'static str;

    /// Creates default instance configuration parameters for the service.
    fn default_instance(&self) -> InstanceInitParams {
        self.artifact_id()
            .into_instance(Self::INSTANCE_ID, Self::INSTANCE_NAME)
            .into()
    }
}

/// Transaction specification for a specific service interface method.
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
    /// Reference to the dispatcher mailbox.
    mailbox: &'a mut Mailbox,
    /// Read-only snapshot of the current blockchain state.
    snapshot: &'a dyn Snapshot,
    /// Transaction broadcaster.
    broadcaster: Broadcaster<'a>,
    /// ID of the node as a validator.
    validator_id: Option<ValidatorId>,
}

impl<'a> AfterCommitContext<'a> {
    /// Creates a new `AfterCommit` context.
    pub(crate) fn new(
        mailbox: &'a mut Mailbox,
        instance: InstanceDescriptor<'a>,
        snapshot: &'a dyn Snapshot,
        service_keypair: &'a (PublicKey, SecretKey),
        tx_sender: &'a ApiSender,
        validator_id: Option<ValidatorId>,
    ) -> Self {
        Self {
            mailbox,
            snapshot,
            validator_id,
            broadcaster: Broadcaster::new(instance, service_keypair, tx_sender),
        }
    }

    /// Returns blockchain data for the snapshot associated with this context.
    pub fn data(&self) -> BlockchainData<&'a dyn Snapshot> {
        BlockchainData::new(self.snapshot, self.broadcaster.instance())
    }

    /// Returns snapshot of the data for the executing service.
    pub fn service_data(&self) -> Prefixed<&'a dyn Snapshot> {
        self.data().for_executing_service()
    }

    /// Returns a current blockchain height. This height is "height of the latest committed block".
    pub fn height(&self) -> Height {
        // TODO Perhaps we should optimize this method [ECR-3222]
        self.data().for_core().height()
    }

    /// Returns the service key of this node.
    pub fn service_key(&self) -> PublicKey {
        self.broadcaster.service_keypair.0
    }

    /// Returns the ID of this node as a validator. If the node is not a validator, returns `None`.
    pub fn validator_id(&self) -> Option<ValidatorId> {
        self.validator_id
    }

    /// Returns a transaction broadcaster if the current node is a validator. If the node
    /// is not a validator, returns `None`.
    pub fn broadcaster(&self) -> Option<Broadcaster<'a>> {
        self.validator_id?;
        Some(self.broadcaster.clone())
    }

    /// Returns a transaction broadcaster regardless of the node status (validator or auditor).
    pub fn generic_broadcaster(&self) -> Broadcaster<'a> {
        self.broadcaster.clone()
    }

    /// Provides a privileged interface to the supervisor service.
    ///
    /// `None` will be returned if the caller is not a supervisor.
    #[doc(hidden)]
    pub fn supervisor_extensions(&mut self) -> Option<SupervisorExtensions<'_>> {
        if !is_supervisor(self.broadcaster.instance().id) {
            return None;
        }
        Some(SupervisorExtensions {
            mailbox: &mut *self.mailbox,
        })
    }
}

// It is impossible to use `Cow` with `InstanceDescriptor` since it has a lifetime of its own.
#[derive(Debug, Clone)]
enum CowInstanceDescriptor<'a> {
    Borrowed(InstanceDescriptor<'a>),
    Owned { id: InstanceId, name: String },
}

impl CowInstanceDescriptor<'_> {
    fn as_ref(&self) -> InstanceDescriptor<'_> {
        match self {
            CowInstanceDescriptor::Borrowed(descriptor) => *descriptor,
            CowInstanceDescriptor::Owned { id, ref name } => InstanceDescriptor { id: *id, name },
        }
    }

    fn into_owned(self) -> CowInstanceDescriptor<'static> {
        match self {
            CowInstanceDescriptor::Borrowed(InstanceDescriptor { id, name }) => {
                CowInstanceDescriptor::Owned {
                    id,
                    name: name.to_owned(),
                }
            }
            CowInstanceDescriptor::Owned { id, name } => CowInstanceDescriptor::Owned { id, name },
        }
    }
}

/// Transaction broadcaster.
///
/// Transaction broadcast allows a service to create transactions in the `after_commit`
/// handler or the HTTP API handlers and broadcast them to the connected Exonum nodes.
/// The transactions are addressed to the executing service instance and are signed
/// by the [service keypair] of the node.
///
/// Broadcasting functionality is primarily useful for services that receive information
/// from outside the blockchain and need to translate it to transactions. As an example,
/// a time oracle service may broadcast local node time and build the blockchain-wide time
/// by processing corresponding transactions.
///
/// [service keypair]: ../../blockchain/config/struct.ValidatorKeys.html#structfield.service_key
#[derive(Debug, Clone)]
pub struct Broadcaster<'a> {
    instance: CowInstanceDescriptor<'a>,
    service_keypair: Cow<'a, (PublicKey, SecretKey)>,
    tx_sender: Cow<'a, ApiSender>,
}

impl<'a> Broadcaster<'a> {
    /// Creates a new broadcaster.
    pub(super) fn new(
        instance: InstanceDescriptor<'a>,
        service_keypair: &'a (PublicKey, SecretKey),
        tx_sender: &'a ApiSender,
    ) -> Self {
        Self {
            instance: CowInstanceDescriptor::Borrowed(instance),
            service_keypair: Cow::Borrowed(service_keypair),
            tx_sender: Cow::Borrowed(tx_sender),
        }
    }

    pub(super) fn keypair(&self) -> &(PublicKey, SecretKey) {
        self.service_keypair.as_ref()
    }

    pub(super) fn instance(&self) -> InstanceDescriptor<'_> {
        self.instance.as_ref()
    }

    /// Signs and broadcasts a transaction to the other nodes in the network.
    ///
    /// The transaction is signed by the service keypair of the node. The same input transaction
    /// will lead to the identical transaction being broadcast. If this is undesired, add a nonce
    /// field to the input transaction (e.g., a `u64`) and change it between the calls.
    ///
    /// # Return value
    ///
    /// Returns the hash of the created transaction, or an error if the transaction cannot be
    /// broadcast. An error means that the node is being shut down.
    pub fn send<Svc: ?Sized, T>(self, tx: T) -> Result<Hash, Error>
    where
        T: Transaction<Svc>,
    {
        let (public_key, secret_key) = self.service_keypair.as_ref();
        let msg = tx.sign(self.instance().id, *public_key, secret_key);
        let tx_hash = msg.object_hash();
        self.tx_sender.broadcast_transaction(msg).map(|()| tx_hash)
    }

    /// Converts the broadcaster into the owned representation, which can be used to broadcast
    /// transactions asynchronously.
    pub fn into_owned(self) -> Broadcaster<'static> {
        Broadcaster {
            instance: self.instance.into_owned(),
            service_keypair: Cow::Owned(self.service_keypair.into_owned()),
            tx_sender: Cow::Owned(self.tx_sender.into_owned()),
        }
    }
}

/// Extended blockchain interface for the service instance authorized as a supervisor.
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
        and_then: impl FnOnce() -> F + 'static + Send,
    ) where
        F: IntoFuture<Item = (), Error = ExecutionError>,
        F::Future: 'static + Send,
    {
        let action = Action::StartDeploy {
            artifact,
            spec: spec.into_bytes(),
            and_then: Box::new(|| Box::new(and_then().into_future())),
        };
        self.mailbox.push(action);
    }
}

impl Debug for AfterCommitContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AfterCommitContext")
            .field("instance", &self.broadcaster.instance)
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
        context: CallContext<'_>,
        method: MethodId,
        payload: &[u8],
    ) -> Result<(), ExecutionError>;
}

fn is_supervisor(instance_id: InstanceId) -> bool {
    instance_id == crate::runtime::SUPERVISOR_INSTANCE_ID
}
