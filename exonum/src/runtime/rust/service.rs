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

use exonum_merkledb::Snapshot;
use failure::Error;
use protobuf::well_known_types::Any;

use std::fmt::{self, Debug};

use crate::{
    api::ServiceApiBuilder,
    blockchain::Schema as CoreSchema,
    crypto::{Hash, PublicKey, SecretKey},
    helpers::Height,
    messages::{AnyTx, Message, MethodId, ServiceInstanceId, ServiceTransaction, Signed},
    node::ApiSender,
    runtime::{error::ExecutionError, rust::TransactionContext},
};

use super::RustArtifactSpec;

pub trait ServiceDispatcher: Send {
    fn call(
        &self,
        method: MethodId,
        ctx: TransactionContext,
        payload: &[u8],
    ) -> Result<Result<(), ExecutionError>, Error>;
}

pub trait Service: ServiceDispatcher + Debug + 'static {
    fn configure(&self, _context: TransactionContext, _params: &Any) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn state_hash(
        &self,
        _service_id: ServiceInstanceId,
        _service_name: &str,
        _snapshot: &dyn Snapshot,
    ) -> Vec<Hash> {
        vec![]
    }

    fn before_commit(&self, _context: TransactionContext) {}

    fn after_commit(&self, _context: AfterCommitContext) {}

    fn wire_api(
        &self,
        _service_id: ServiceInstanceId,
        _service_name: &str,
        _builder: &mut ServiceApiBuilder,
    ) {
    }
    // TODO: add other hooks such as "on node startup", etc.
}

pub trait ServiceFactory: Send + Debug + 'static {
    fn artifact(&self) -> RustArtifactSpec;
    fn new_instance(&self) -> Box<dyn Service>;
}

impl<T> From<T> for Box<dyn ServiceFactory>
where
    T: ServiceFactory,
{
    fn from(factory: T) -> Self {
        Box::new(factory) as Self
    }
}

pub struct AfterCommitContext<'a> {
    service_id: ServiceInstanceId,
    service_name: &'a str,
    snapshot: &'a dyn Snapshot,
    service_keypair: &'a (PublicKey, SecretKey),
    tx_sender: &'a ApiSender,
}

impl<'a> AfterCommitContext<'a> {
    /// Creates context for `after_commit` method.
    pub(crate) fn new(
        service_id: ServiceInstanceId,
        service_name: &'a str,
        snapshot: &'a dyn Snapshot,
        service_keypair: &'a (PublicKey, SecretKey),
        tx_sender: &'a ApiSender,
    ) -> Self {
        Self {
            service_id,
            service_name,
            snapshot,
            service_keypair,
            tx_sender,
        }
    }

    /// Returns the current service instance identifier.
    pub fn service_id(&self) -> ServiceInstanceId {
        self.service_id
    }

    /// Returns the current service instance name.
    pub fn service_name(&self) -> &str {
        self.service_name
    }

    /// Returns the current blockchain height. This height is "height of the last committed block".
    pub fn height(&self) -> Height {
        CoreSchema::new(self.snapshot).height()
    }

    /// Signs and broadcasts transaction to other nodes in the network.
    pub fn broadcast_transaction(&self, tx: impl Into<ServiceTransaction>) {
        let msg = Message::sign_transaction(
            tx,
            self.service_id,
            self.service_keypair.0,
            &self.service_keypair.1,
        );

        if let Err(e) = self.tx_sender.broadcast_transaction(msg) {
            error!("Couldn't broadcast transaction {}.", e);
        }
    }

    /// Broadcast transaction to other nodes in the network.
    /// This transaction should be signed externally.
    pub fn broadcast_signed_transaction(&self, msg: Signed<AnyTx>) {
        if let Err(e) = self.tx_sender.broadcast_transaction(msg) {
            error!("Couldn't broadcast transaction {}.", e);
        }
    }
}

impl<'a> Debug for AfterCommitContext<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("AfterCommitContext")
            .field("service_id", &self.service_id)
            .field("service_name", &self.service_name)
            .finish()
    }
}

#[macro_export]
macro_rules! impl_service_dispatcher {
    ($struct_name:ident, $interface:ident) => {
        impl $crate::runtime::rust::service::ServiceDispatcher for $struct_name {
            fn call(
                &self,
                method: $crate::messages::MethodId,
                ctx: $crate::runtime::rust::TransactionContext,
                payload: &[u8],
            ) -> Result<Result<(), $crate::runtime::error::ExecutionError>, failure::Error> {
                <$struct_name as $interface>::_dispatch(self, ctx, method, payload)
            }
        }
    };
}
