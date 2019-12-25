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

//! Building blocks for creating HTTP API of Rust services.

pub use crate::api::{Deprecated, EndpointMutability, Error, FutureResult, Result};

use futures::IntoFuture;
use serde::{de::DeserializeOwned, Serialize};

use exonum_crypto::PublicKey;
use exonum_merkledb::{access::Prefixed, Snapshot};

use super::Broadcaster;
use crate::{
    api::{error::MovedPermanentlyError, ApiBuilder, ApiScope},
    blockchain::{Blockchain, Schema as CoreSchema},
    runtime::{BlockchainData, InstanceDescriptor, InstanceId},
};

/// Provide the current blockchain state snapshot to API handlers.
///
/// This structure allows a service API handler to interact with the service instance
/// and other parts of the blockchain.
#[derive(Debug)]
pub struct ServiceApiState<'a> {
    /// Transaction broadcaster.
    broadcaster: Broadcaster<'a>,
    // TODO Think about avoiding of unnecessary snapshots creation. [ECR-3222]
    snapshot: Box<dyn Snapshot>,
    /// Endpoint path relative to the service root
    endpoint: String,
}

impl<'a> ServiceApiState<'a> {
    /// Create service API state snapshot from the given blockchain and instance descriptor.
    pub fn from_api_context<S: Into<String>>(
        blockchain: &'a Blockchain,
        instance: InstanceDescriptor<'a>,
        endpoint: S,
    ) -> Self {
        Self {
            broadcaster: Broadcaster::new(
                instance,
                blockchain.service_keypair(),
                blockchain.sender(),
            ),
            snapshot: blockchain.snapshot(),
            endpoint: endpoint.into(),
        }
    }

    /// Returns readonly access to blockchain data.
    pub fn data(&'a self) -> BlockchainData<&dyn Snapshot> {
        BlockchainData::new(&self.snapshot, self.instance())
    }

    /// Returns readonly access to the data of the executing service.
    pub fn service_data(&'a self) -> Prefixed<&dyn Snapshot> {
        self.data().for_executing_service()
    }

    /// Returns the access to the entire blockchain snapshot. Use [`data`](#method.data)
    /// or [`service_data`](#method.service_data) for more structure snapshot presentations.
    pub fn snapshot(&self) -> &dyn Snapshot {
        &self.snapshot
    }

    /// Returns the service key of this node.
    pub fn service_key(&self) -> PublicKey {
        self.broadcaster.keypair().0
    }

    /// Returns information about the executing service.
    pub fn instance(&self) -> InstanceDescriptor<'_> {
        self.broadcaster.instance()
    }

    /// Returns a transaction broadcaster if the current node is a validator. If the node
    /// is not a validator, returns `None`.
    pub fn broadcaster(&self) -> Option<Broadcaster<'a>> {
        CoreSchema::new(&self.snapshot).validator_id(self.broadcaster.keypair().0)?;
        Some(self.broadcaster.clone())
    }

    /// Returns a transaction broadcaster regardless of the node status (validator or auditor).
    pub fn generic_broadcaster(&self) -> Broadcaster<'a> {
        self.broadcaster.clone()
    }

    /// Creates a new builder for `MovedPermanently` response.
    pub fn moved_permanently(&self, new_endpoint: &str) -> MovedPermanentlyError {
        let new_url = Self::relative_to(&self.endpoint, new_endpoint);

        MovedPermanentlyError::new(new_url)
    }

    /// Takes an old endpoint and a new endpoint as direct URIs, and creates
    /// a relative path from the old to the new one.
    fn relative_to(old_endpoint: &str, new_endpoint: &str) -> String {
        let endpoint_without_end_slash = old_endpoint.trim_end_matches('/');
        let mut nesting_level = endpoint_without_end_slash
            .chars()
            .filter(|&c| c == '/')
            .count();

        // Mounting points do not contain the leading slash, e.g. `endpoint("v1/stats")`.
        nesting_level += 1;

        let path_to_service_root = "../".repeat(nesting_level);

        format!("{}{}", path_to_service_root, new_endpoint)
    }
}

/// Exonum API builder for the concrete service API scope.
#[derive(Debug, Clone)]
pub struct ServiceApiScope {
    inner: ApiScope,
    blockchain: Blockchain,
    descriptor: (InstanceId, String),
}

impl ServiceApiScope {
    /// Creates a new service API scope for the specified service instance.
    pub fn new(blockchain: Blockchain, instance: InstanceDescriptor<'_>) -> Self {
        Self {
            inner: ApiScope::new(),
            blockchain,
            descriptor: instance.into(),
        }
    }

    /// Adds a readonly endpoint handler to the service API scope.
    ///
    /// In HTTP backends this type of endpoint corresponds to `GET` requests.
    /// [Read more.](../../../api/struct.ApiScope.html#endpoint)
    pub fn endpoint<Q, I, F, R>(&mut self, name: &'static str, handler: F) -> &mut Self
    where
        Q: DeserializeOwned + 'static,
        I: Serialize + 'static,
        F: Fn(&ServiceApiState<'_>, Q) -> R + 'static + Clone + Send + Sync,
        R: IntoFuture<Item = I, Error = crate::api::Error> + 'static,
    {
        let blockchain = self.blockchain.clone();
        let descriptor = self.descriptor.clone();
        self.inner
            .endpoint(name, move |query: Q| -> crate::api::FutureResult<I> {
                let descriptor = (descriptor.0, descriptor.1.as_ref());
                let state = ServiceApiState::from_api_context(&blockchain, descriptor.into(), name);
                let result = handler(&state, query);
                Box::new(result.into_future())
            });
        self
    }

    /// Adds an endpoint handler to the service API scope.
    ///
    /// In HTTP backends this type of endpoint corresponds to `POST` requests.
    /// [Read more.](../../../api/struct.ApiScope.html#endpoint_mut)
    pub fn endpoint_mut<Q, I, F, R>(&mut self, name: &'static str, handler: F) -> &mut Self
    where
        Q: DeserializeOwned + 'static,
        I: Serialize + 'static,
        F: Fn(&ServiceApiState<'_>, Q) -> R + 'static + Clone + Send + Sync,
        R: IntoFuture<Item = I, Error = crate::api::Error> + 'static,
    {
        let blockchain = self.blockchain.clone();
        let descriptor = self.descriptor.clone();
        self.inner
            .endpoint_mut(name, move |query: Q| -> crate::api::FutureResult<I> {
                let descriptor = (descriptor.0, descriptor.1.as_ref());
                let state = ServiceApiState::from_api_context(&blockchain, descriptor.into(), name);
                let result = handler(&state, query);
                Box::new(result.into_future())
            });
        self
    }

    /// Same as `endpoint`, but the response will contain a warning about endpoint being deprecated.
    /// Optional endpoint expiration date and deprecation-related information (e.g., link to a documentation for
    /// a new API) can be included in the warning.
    pub fn deprecated_endpoint<Q, I, F, R>(
        &mut self,
        name: &'static str,
        deprecated: Deprecated<Q, I, R, F>,
    ) -> &mut Self
    where
        Q: DeserializeOwned + 'static,
        I: Serialize + 'static,
        F: Fn(&ServiceApiState<'_>, Q) -> R + 'static + Clone + Send + Sync,
        R: IntoFuture<Item = I, Error = crate::api::Error> + 'static,
    {
        let blockchain = self.blockchain.clone();
        let descriptor = self.descriptor.clone();
        let inner = deprecated.handler.clone();
        let handler = move |query: Q| -> crate::api::FutureResult<I> {
            let descriptor = (descriptor.0, descriptor.1.as_ref());
            let state = ServiceApiState::from_api_context(&blockchain, descriptor.into(), name);
            let result = inner(&state, query);
            Box::new(result.into_future())
        };
        // Mark endpoint as deprecated.
        let handler = deprecated.with_different_handler(handler);
        self.inner.endpoint(name, handler);
        self
    }

    /// Same as `endpoint_mut`, but the response will contain a warning about endpoint being deprecated.
    /// Optional endpoint expiration date and deprecation-related information (e.g., link to a documentation for
    /// a new API) can be included in the warning.
    pub fn deprecated_endpoint_mut<Q, I, F, R>(
        &mut self,
        name: &'static str,
        deprecated: Deprecated<Q, I, R, F>,
    ) -> &mut Self
    where
        Q: DeserializeOwned + 'static,
        I: Serialize + 'static,
        F: Fn(&ServiceApiState<'_>, Q) -> R + 'static + Clone + Send + Sync,
        R: IntoFuture<Item = I, Error = crate::api::Error> + 'static,
    {
        let blockchain = self.blockchain.clone();
        let descriptor = self.descriptor.clone();
        let inner = deprecated.handler.clone();
        let handler = move |query: Q| -> crate::api::FutureResult<I> {
            let descriptor = (descriptor.0, descriptor.1.as_ref());
            let state = ServiceApiState::from_api_context(&blockchain, descriptor.into(), name);
            let result = inner(&state, query);
            Box::new(result.into_future())
        };
        // Mark endpoint as deprecated.
        let handler = deprecated.with_different_handler(handler);
        self.inner.endpoint_mut(name, handler);
        self
    }

    /// Return a mutable reference to the underlying web backend.
    pub fn web_backend(&mut self) -> &mut crate::api::backends::actix::ApiBuilder {
        self.inner.web_backend()
    }
}

/// Exonum service API builder which is used to add endpoints to the node API.
///
/// # Examples
///
/// The example below shows a common practice of the API implementation.
///
/// ```
/// use serde_derive::{Deserialize, Serialize};
///
/// use exonum::{
///     blockchain::Schema,
///     crypto::{self, Hash},
///     node::ExternalMessage,
///     runtime::rust::api::{self, ServiceApiBuilder, ServiceApiState},
/// };
/// use exonum_merkledb::ObjectHash;
///
/// // Declare a type which describes an API specification and implementation.
/// pub struct MyApi;
///
/// // Declare structures for requests and responses.
///
/// // For the web backend `MyQuery` will be deserialized from the `block_height={number}` string.
/// #[derive(Deserialize, Clone, Copy)]
/// pub struct MyQuery {
///     pub block_height: u64,
/// }
///
/// // For the web backend `BlockInfo` will be serialized into a JSON string.
/// #[derive(Serialize, Clone, Copy)]
/// pub struct BlockInfo {
///     pub hash: Hash,
/// }
///
/// // Create API handlers.
/// impl MyApi {
///     /// Immutable handler which returns a hash of the block at the given height.
///     pub fn block_hash(
///         state: &ServiceApiState,
///         query: MyQuery,
///     ) -> api::Result<Option<BlockInfo>> {
///         let schema = state.data().for_core();
///         Ok(schema
///             .block_hashes_by_height()
///             .get(query.block_height)
///             .map(|hash| BlockInfo { hash }))
///     }
///
///     /// Simple handler without any parameters.
///     pub fn ping(_state: &ServiceApiState, _query: ()) -> api::Result<()> {
///         Ok(())
///     }
///
///     /// You may also create asynchronous handlers for long requests.
///     pub fn async_operation(
///         _state: &ServiceApiState,
///         query: MyQuery,
///     ) -> api::FutureResult<Option<Hash>> {
///         Box::new(futures::lazy(move || {
///             Ok(Some(query.block_height.object_hash()))
///         }))
///     }
/// }
///
/// fn wire_api(builder: &mut ServiceApiBuilder) -> &mut ServiceApiBuilder {
///     // Add `MyApi` handlers to the corresponding builder.
///     builder
///         .public_scope()
///         .endpoint("v1/block_hash", MyApi::block_hash)
///         .endpoint("v1/async_operation", MyApi::async_operation);
///     // Add a mutable endpoint to the private API.
///     builder
///         .private_scope()
///         .endpoint("v1/ping", MyApi::ping);
///     builder
/// }
/// # fn main() {
/// #     use exonum::{blockchain::Blockchain, node::ApiSender, runtime::InstanceDescriptor};
/// #     use exonum_merkledb::TemporaryDB;
/// #     use futures::sync::mpsc;
/// #
/// #     let blockchain = Blockchain::new(
/// #         TemporaryDB::new(),
/// #         crypto::gen_keypair(),
/// #         ApiSender::new(mpsc::channel(0).0),
/// #     );
/// #     let mut builder = ServiceApiBuilder::new(
/// #         blockchain,
/// #         InstanceDescriptor {
/// #             id: 1100,
/// #             name: "example",
/// #         },
/// #     );
/// #     wire_api(&mut builder);
/// # }
/// ```
#[derive(Debug)]
pub struct ServiceApiBuilder {
    blockchain: Blockchain,
    public_scope: ServiceApiScope,
    private_scope: ServiceApiScope,
}

impl ServiceApiBuilder {
    /// Create a new service API builder for the specified service instance.
    #[doc(hidden)]
    pub fn new(blockchain: Blockchain, instance: InstanceDescriptor<'_>) -> Self {
        Self {
            blockchain: blockchain.clone(),
            public_scope: ServiceApiScope::new(blockchain.clone(), instance),
            private_scope: ServiceApiScope::new(blockchain, instance),
        }
    }

    /// Return a mutable reference to the public API scope builder.
    pub fn public_scope(&mut self) -> &mut ServiceApiScope {
        &mut self.public_scope
    }

    /// Return a mutable reference to the private API scope builder.
    pub fn private_scope(&mut self) -> &mut ServiceApiScope {
        &mut self.private_scope
    }

    /// Return a reference to the blockchain.
    pub fn blockchain(&self) -> &Blockchain {
        &self.blockchain
    }
}

impl From<ServiceApiBuilder> for ApiBuilder {
    fn from(inner: ServiceApiBuilder) -> Self {
        Self {
            public_scope: inner.public_scope.inner,
            private_scope: inner.private_scope.inner,
        }
    }
}
