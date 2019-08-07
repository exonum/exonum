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

//! Building blocks for creating services' API.

pub use crate::api::{Error, FutureResult, Result};

use exonum_merkledb::{Database, Snapshot};
use futures::IntoFuture;
use serde::{de::DeserializeOwned, Serialize};

use std::sync::Arc;

use crate::{
    api::{ApiBuilder, ApiScope},
    blockchain::Blockchain,
    crypto::{PublicKey, SecretKey},
    node::ApiSender,
    runtime::{InstanceDescriptor, InstanceId},
};

/// Provides the current blockchain state to API handlers.
///
/// This structure is a part of the node that is available to the API. For example,
/// it can return the private key of the node, which allows the service to send
/// certain transactions to the blockchain.
#[derive(Debug)]
pub struct ServiceApiState<'a> {
    service_keypair: (&'a PublicKey, &'a SecretKey),
    instance_descriptor: InstanceDescriptor<'a>,
    api_sender: &'a ApiSender,
    // TODO Think about avoiding of unnecessary snapshots creation. [ECR-3222]
    snapshot: Box<dyn Snapshot>,
}

impl<'a> ServiceApiState<'a> {
    pub fn from_api_context(
        context: &'a ApiContext,
        instance_descriptor: InstanceDescriptor<'a>,
    ) -> Self {
        Self {
            service_keypair: (&context.service_keypair.0, &context.service_keypair.1),
            instance_descriptor,
            api_sender: &context.api_sender,
            snapshot: context.snapshot(),
        }
    }

    pub fn instance(&self) -> InstanceDescriptor {
        self.instance_descriptor
    }

    /// Creates a read-only snapshot of the current blockchain state.
    pub fn snapshot(&'a self) -> &dyn Snapshot {
        self.snapshot.as_ref()
    }

    /// Returns the public key of the current node.
    pub fn public_key(&self) -> &PublicKey {
        self.service_keypair.0
    }

    /// Returns the secret key of the current node.
    pub fn secret_key(&self) -> &SecretKey {
        self.service_keypair.1
    }

    /// Returns a reference to the API sender.
    pub fn sender(&self) -> &ApiSender {
        self.api_sender
    }
}

#[derive(Debug, Clone)]
pub struct ApiContext {
    service_keypair: (PublicKey, SecretKey),
    api_sender: ApiSender,
    database: Arc<dyn Database>,
}

impl ApiContext {
    pub fn with_blockchain(blockchain: &Blockchain) -> Self {
        Self {
            service_keypair: blockchain.service_keypair.clone(),
            api_sender: blockchain.api_sender.clone(),
            database: blockchain.db.clone(),
        }
    }

    pub fn snapshot(&self) -> Box<dyn Snapshot> {
        self.database.snapshot()
    }

    pub fn sender(&self) -> &ApiSender {
        &self.api_sender
    }
}

#[derive(Debug, Clone)]
pub struct ServiceApiScope {
    inner: ApiScope,
    context: ApiContext,
    descriptor: (InstanceId, String),
}

impl ServiceApiScope {
    pub fn new(context: ApiContext, instance_descriptor: InstanceDescriptor) -> Self {
        Self {
            inner: ApiScope::new(),
            context,
            descriptor: instance_descriptor.into(),
        }
    }

    pub fn endpoint<Q, I, F, R>(&mut self, name: &'static str, handler: F) -> &mut Self
    where
        Q: DeserializeOwned + 'static,
        I: Serialize + 'static,
        F: Fn(&ServiceApiState, Q) -> R + 'static + Clone + Send + Sync,
        R: IntoFuture<Item = I, Error = crate::api::Error> + 'static,
    {
        let context = self.context.clone();
        let descriptor = self.descriptor.clone();
        self.inner
            .endpoint(name, move |query: Q| -> crate::api::FutureResult<I> {
                let state = ServiceApiState::from_api_context(
                    &context,
                    InstanceDescriptor {
                        id: descriptor.0,
                        name: descriptor.1.as_ref(),
                    },
                );
                let result = handler(&state, query);
                Box::new(result.into_future())
            });
        self
    }

    pub fn endpoint_mut<Q, I, F, R>(&mut self, name: &'static str, handler: F) -> &mut Self
    where
        Q: DeserializeOwned + 'static,
        I: Serialize + 'static,
        F: Fn(&ServiceApiState, Q) -> R + 'static + Clone + Send + Sync,
        R: IntoFuture<Item = I, Error = crate::api::Error> + 'static,
    {
        let context = self.context.clone();
        let descriptor = self.descriptor.clone();
        self.inner
            .endpoint_mut(name, move |query: Q| -> crate::api::FutureResult<I> {
                let state = ServiceApiState::from_api_context(
                    &context,
                    InstanceDescriptor {
                        id: descriptor.0,
                        name: descriptor.1.as_ref(),
                    },
                );
                let result = handler(&state, query);
                Box::new(result.into_future())
            });
        self
    }

    /// Returns a mutable reference to the underlying web backend.
    pub fn web_backend(&mut self) -> &mut crate::api::backends::actix::ApiBuilder {
        self.inner.web_backend()
    }
}

/// Exonum service API builder, which is used to add endpoints to the node API.
///
/// # Examples
///
/// The example below shows a common practice of API implementation.
///
/// ```rust
/// #[macro_use] extern crate exonum;
/// #[macro_use] extern crate serde_derive;
/// extern crate futures;
///
/// use futures::Future;
///
/// use std::net::SocketAddr;
///
/// use exonum::api::{self, ServiceApiBuilder, ServiceApiState};
/// use exonum::blockchain::{Schema};
/// use exonum::crypto::{Hash, PublicKey};
///
/// // Declares a type which describes an API specification and implementation.
/// pub struct MyApi;
///
/// // Declares structures for requests and responses.
///
/// // For the web backend, `MyQuery` will be deserialized from a `block_height={number}` string.
/// #[derive(Deserialize, Clone, Copy)]
/// pub struct MyQuery {
///     pub block_height: u64
/// }
///
/// // For the web backend, `BlockInfo` will be serialized into a JSON string.
/// #[derive(Serialize, Clone, Copy)]
/// pub struct BlockInfo {
///     pub hash: Hash,
/// }
///
/// // Creates API handlers.
/// impl MyApi {
///     // Immutable handler, which returns a hash of the block at the given height.
///     pub fn block_hash(state: &ServiceApiState, query: MyQuery) -> api::Result<Option<BlockInfo>> {
///         let snapshot = state.snapshot();
///         let schema = Schema::new(&snapshot);
///         Ok(schema.block_hashes_by_height()
///             .get(query.block_height)
///             .map(|hash| BlockInfo { hash })
///         )
///     }
///
///     // Mutable handler which removes the peer with the given key from the cache.
///     pub fn remove_peer(state: &ServiceApiState, query: PublicKey) -> api::Result<()> {
///         let mut blockchain = state.blockchain().clone();
///         Ok(blockchain.remove_peer_with_pubkey(&query))
///     }
///
///     // Simple handler without any parameters.
///     pub fn ping(_state: &ServiceApiState, _query: ()) -> api::Result<()> {
///         Ok(())
///     }
///
///     // You may also create asynchronous handlers for long requests.
///     pub fn block_hash_async(state: &ServiceApiState, query: MyQuery)
///      -> api::FutureResult<Option<Hash>> {
///         let blockchain = state.blockchain().clone().snapshot();
///         Box::new(futures::lazy(move || {
///             let schema = Schema::new(&blockchain);
///             Ok(schema.block_hashes_by_height().get(query.block_height))
///         }))
///     }
/// }
///
/// # let mut builder = ServiceApiBuilder::default();
/// // Adds `MyApi` handlers to the corresponding builder.
/// builder.public_scope()
///     .endpoint("v1/ping", MyApi::ping)
///     .endpoint("v1/block_hash", MyApi::block_hash)
///     .endpoint("v1/block_hash_async", MyApi::block_hash_async);
/// // Adds a mutable endpoint for to the private API.
/// builder.private_scope()
///     .endpoint_mut("v1/remove_peer", MyApi::remove_peer);
/// ```
#[derive(Debug)]
pub struct ServiceApiBuilder {
    context: ApiContext,
    public_scope: ServiceApiScope,
    private_scope: ServiceApiScope,
}

impl ServiceApiBuilder {
    /// Creates a new service API builder.
    pub(crate) fn new(context: ApiContext, instance_descriptor: InstanceDescriptor) -> Self {
        Self {
            context: context.clone(),
            public_scope: ServiceApiScope::new(context.clone(), instance_descriptor),
            private_scope: ServiceApiScope::new(context, instance_descriptor),
        }
    }

    /// Returns a mutable reference to the public API scope builder.
    pub fn public_scope(&mut self) -> &mut ServiceApiScope {
        &mut self.public_scope
    }

    /// Returns a mutable reference to the private API scope builder.
    pub fn private_scope(&mut self) -> &mut ServiceApiScope {
        &mut self.private_scope
    }

    /// Returns a reference to the underlying API context.
    pub fn context(&self) -> &ApiContext {
        &self.context
    }
}

impl From<ServiceApiBuilder> for ApiBuilder {
    fn from(inner: ServiceApiBuilder) -> Self {
        Self {
            public_scope: inner.public_scope.inner,
            private_scope: inner.private_scope.inner,
            blockchain: None,
        }
    }
}
