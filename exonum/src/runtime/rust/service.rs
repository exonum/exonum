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

use exonum_merkledb::{BinaryValue, Fork, IndexAccess, Snapshot};
use failure::Error;

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
        AnyTx, ArtifactInfo, CallInfo, ExecutionContext, InstanceDescriptor, InstanceId, MethodId,
    },
};

use super::RustArtifactId;

pub trait ServiceDispatcher: Send {
    fn call(
        &self,
        method: MethodId,
        ctx: TransactionContext,
        payload: &[u8],
    ) -> Result<Result<(), ExecutionError>, Error>;
}

pub trait Service: ServiceDispatcher + Debug + 'static {
    fn configure(
        &self,
        _descriptor: InstanceDescriptor,
        _fork: &Fork,
        _params: Any,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn state_hash(&self, _descriptor: InstanceDescriptor, _snapshot: &dyn Snapshot) -> Vec<Hash> {
        vec![]
    }

    fn before_commit(&self, _context: TransactionContext) {}

    fn after_commit(&self, _context: AfterCommitContext) {}

    fn wire_api(&self, _builder: &mut ServiceApiBuilder) {}
    // TODO: add other hooks such as "on node startup", etc.
}

pub trait ServiceFactory: Send + Debug + 'static {
    fn artifact_id(&self) -> RustArtifactId;

    fn artifact_info(&self) -> ArtifactInfo;

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

#[derive(Debug)]
pub struct TransactionContext<'a, 'b> {
    pub(super) instance_descriptor: InstanceDescriptor<'a>,
    pub(super) runtime_context: &'a mut ExecutionContext<'b>,
    pub(super) dispatcher: &'a Dispatcher,
}

impl<'a, 'b> TransactionContext<'a, 'b> {
    // TODO replace this methods by the `instance_descriptor` [ECR-3222]

    pub fn service_id(&self) -> InstanceId {
        self.instance_descriptor.id
    }

    pub fn service_name(&self) -> &str {
        self.instance_descriptor.name
    }

    /// If the current node is a validator, returns its identifier, for other nodes return `None`.
    pub fn validator_id(&self) -> Option<ValidatorId> {
        // TODO Perhaps we should optimize this method [ECR-3222]
        CoreSchema::new(self.runtime_context.fork)
            .actual_configuration()
            .validator_keys
            .iter()
            .position(|validator| self.author() == validator.service_key)
            .map(|id| ValidatorId(id as u16))
    }

    pub fn fork(&self) -> impl IndexAccess + 'b {
        self.runtime_context.fork
    }

    pub fn tx_hash(&self) -> Hash {
        self.runtime_context.caller.transaction_hash().unwrap()
    }

    pub fn author(&self) -> PublicKey {
        self.runtime_context.caller.author().unwrap()
    }

    pub fn call(&mut self, call_info: CallInfo, payload: &[u8]) -> Result<(), ExecutionError> {
        self.dispatcher
            .call(self.runtime_context, call_info, payload)
    }

    pub(crate) fn dispatch_action(&mut self, action: dispatcher::Action) {
        self.runtime_context.dispatch_action(action)
    }
}

pub struct AfterCommitContext<'a> {
    dispatcher: &'a DispatcherSender,
    instance_descriptor: InstanceDescriptor<'a>,
    snapshot: &'a dyn Snapshot,
    service_keypair: &'a (PublicKey, SecretKey),
    tx_sender: &'a ApiSender,
}

impl<'a> AfterCommitContext<'a> {
    /// Creates context for `after_commit` method.
    pub(crate) fn new(
        dispatcher: &'a DispatcherSender,
        instance_descriptor: InstanceDescriptor<'a>,
        snapshot: &'a dyn Snapshot,
        service_keypair: &'a (PublicKey, SecretKey),
        tx_sender: &'a ApiSender,
    ) -> Self {
        Self {
            dispatcher,
            instance_descriptor,
            snapshot,
            service_keypair,
            tx_sender,
        }
    }

    /// Returns the current database snapshot. This snapshot is used to
    /// retrieve schema information from the database.
    pub fn snapshot(&self) -> &dyn Snapshot {
        self.snapshot
    }

    /// Returns the current service instance identifier.
    pub fn service_id(&self) -> InstanceId {
        self.instance_descriptor.id
    }

    /// Returns the current service instance name.
    pub fn service_name(&self) -> &str {
        self.instance_descriptor.name
    }

    /// If the current node is a validator, returns its identifier, for other nodes return `None`.
    pub fn validator_id(&self) -> Option<ValidatorId> {
        // TODO Perhaps we should optimize this method [ECR-3222]
        CoreSchema::new(self.snapshot)
            .actual_configuration()
            .validator_keys
            .iter()
            .position(|validator| self.service_keypair.0 == validator.service_key)
            .map(|id| ValidatorId(id as u16))
    }

    /// Returns the public key of the current node.
    pub fn public_key(&self) -> &PublicKey {
        &self.service_keypair.0
    }

    /// Returns the secret key of the current node.
    pub fn secret_key(&self) -> &SecretKey {
        &self.service_keypair.1
    }

    /// Returns the current blockchain height. This height is "height of the last committed block".
    pub fn height(&self) -> Height {
        // TODO Perhaps we should optimize this method [ECR-3222]
        CoreSchema::new(self.snapshot).height()
    }

    /// Signs and broadcasts transaction to other nodes in the network.
    pub fn broadcast_transaction(&self, tx: impl Transaction) {
        let msg = tx.sign(
            self.service_id(),
            self.service_keypair.0,
            &self.service_keypair.1,
        );
        if let Err(e) = self.tx_sender.broadcast_transaction(msg) {
            error!("Couldn't broadcast transaction {}.", e);
        }
    }

    /// Broadcast transaction to other nodes in the network.
    /// This transaction should be signed externally.
    pub fn broadcast_signed_transaction(&self, msg: Verified<AnyTx>) {
        if let Err(e) = self.tx_sender.broadcast_transaction(msg) {
            error!("Couldn't broadcast transaction {}.", e);
        }
    }

    /// Returns reference to communication channel with dispatcher.
    pub(crate) fn dispatcher_channel(&self) -> &DispatcherSender {
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
            .field("instance_descriptor", &self.instance_descriptor)
            .finish()
    }
}

/// Transaction specification for the concrete service interface.
pub trait Transaction: BinaryValue {
    /// Service interface associated for the given transaction.
    type Service;
    /// Identifier of service method which executes the given transaction.
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

    /// Signs value as transaction with the specified instance identifier.
    fn sign(
        self,
        service_id: InstanceId,
        public_key: PublicKey,
        secret_key: &SecretKey,
    ) -> Verified<AnyTx> {
        Verified::from_value(self.into_any_tx(service_id), public_key, secret_key)
    }
}
