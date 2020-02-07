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

//! Building blocks for creating HTTP API of Rust services.

pub use exonum_api::{Deprecated, EndpointMutability, Error, FutureResult, HttpStatusCode, Result};

use exonum::{
    blockchain::{Blockchain, Schema as CoreSchema},
    crypto::PublicKey,
    merkledb::{access::Prefixed, Snapshot},
    runtime::{BlockchainData, InstanceDescriptor, InstanceId},
};
use exonum_api::{backends::actix, ApiBuilder, ApiScope, MovedPermanentlyError};
use futures::{Future, IntoFuture};
use serde::{de::DeserializeOwned, Serialize};

use super::Broadcaster;

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
        instance: InstanceDescriptor,
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
        BlockchainData::new(&self.snapshot, &self.instance().name)
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
        self.broadcaster.keypair().public_key()
    }

    /// Returns information about the executing service.
    pub fn instance(&self) -> &InstanceDescriptor {
        &self.broadcaster.instance()
    }

    /// Returns a transaction broadcaster if the current node is a validator. If the node
    /// is not a validator, returns `None`.
    pub fn broadcaster(&self) -> Option<Broadcaster<'a>> {
        CoreSchema::new(&self.snapshot).validator_id(self.broadcaster.keypair().public_key())?;
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
    pub fn new(blockchain: Blockchain, instance: &InstanceDescriptor) -> Self {
        Self {
            inner: ApiScope::new(),
            blockchain,
            descriptor: (instance.id, instance.name.clone()),
        }
    }

    /// Adds a readonly endpoint handler to the service API scope.
    ///
    /// In HTTP backends this type of endpoint corresponds to `GET` requests.
    pub fn endpoint<Q, I, F, R>(&mut self, name: &'static str, handler: F) -> &mut Self
    where
        Q: DeserializeOwned + 'static,
        I: Serialize + 'static,
        F: Fn(&ServiceApiState<'_>, Q) -> R + 'static + Clone + Send + Sync,
        R: IntoFuture<Item = I, Error = Error> + 'static,
    {
        let blockchain = self.blockchain.clone();
        let (instance_id, instance_name) = self.descriptor.clone();
        self.inner
            .endpoint(name, move |query: Q| -> FutureResult<I> {
                let descriptor = InstanceDescriptor::new(instance_id, &instance_name);
                let state = ServiceApiState::from_api_context(&blockchain, descriptor, name);
                let result = handler(&state, query);

                let instance_name = instance_name.clone();
                let future = result
                    .into_future()
                    .map_err(move |err| err.source(format!("{}:{}", instance_id, instance_name)));
                Box::new(future)
            });
        self
    }

    /// Adds an endpoint handler to the service API scope.
    ///
    /// In HTTP backends this type of endpoint corresponds to `POST` requests.
    pub fn endpoint_mut<Q, I, F, R>(&mut self, name: &'static str, handler: F) -> &mut Self
    where
        Q: DeserializeOwned + 'static,
        I: Serialize + 'static,
        F: Fn(&ServiceApiState<'_>, Q) -> R + 'static + Clone + Send + Sync,
        R: IntoFuture<Item = I, Error = Error> + 'static,
    {
        let blockchain = self.blockchain.clone();
        let (instance_id, instance_name) = self.descriptor.clone();
        self.inner
            .endpoint_mut(name, move |query: Q| -> FutureResult<I> {
                let descriptor = InstanceDescriptor::new(instance_id, &instance_name);
                let state = ServiceApiState::from_api_context(&blockchain, descriptor, name);
                let result = handler(&state, query);

                let instance_name = instance_name.clone();
                let future = result
                    .into_future()
                    .map_err(move |err| err.source(format!("{}:{}", instance_id, instance_name)));
                Box::new(future)
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
        R: IntoFuture<Item = I, Error = Error> + 'static,
    {
        let blockchain = self.blockchain.clone();
        let (instance_id, instance_name) = self.descriptor.clone();
        let inner = deprecated.handler.clone();
        let handler = move |query: Q| -> FutureResult<I> {
            let descriptor = InstanceDescriptor::new(instance_id, &instance_name);
            let state = ServiceApiState::from_api_context(&blockchain, descriptor, name);
            let result = inner(&state, query);

            let instance_name = instance_name.clone();
            let future = result
                .into_future()
                .map_err(move |err| err.source(format!("{}:{}", instance_id, instance_name)));
            Box::new(future)
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
        R: IntoFuture<Item = I, Error = Error> + 'static,
    {
        let blockchain = self.blockchain.clone();
        let (instance_id, instance_name) = self.descriptor.clone();
        let inner = deprecated.handler.clone();
        let handler = move |query: Q| -> FutureResult<I> {
            let descriptor = InstanceDescriptor::new(instance_id, &instance_name);
            let state = ServiceApiState::from_api_context(&blockchain, descriptor, name);
            let result = inner(&state, query);

            let instance_name = instance_name.clone();
            let future = result
                .into_future()
                .map_err(move |err| err.source(format!("{}:{}", instance_id, instance_name)));
            Box::new(future)
        };
        // Mark endpoint as deprecated.
        let handler = deprecated.with_different_handler(handler);
        self.inner.endpoint_mut(name, handler);
        self
    }

    /// Return a mutable reference to the underlying web backend.
    pub fn web_backend(&mut self) -> &mut actix::ApiBuilder {
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
/// use exonum::{blockchain::Schema, crypto::Hash, merkledb::ObjectHash};
/// use exonum_rust_runtime::api::{self, ServiceApiBuilder, ServiceApiState};
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
/// # use exonum::{
/// #     blockchain::{ApiSender, Blockchain}, merkledb::TemporaryDB,
/// #     runtime::InstanceDescriptor,
/// # };
/// # use futures::sync::mpsc;
/// # fn main() {
/// #     let blockchain = Blockchain::new(
/// #         TemporaryDB::new(),
/// #         exonum::crypto::gen_keypair(),
/// #         ApiSender::closed(),
/// #     );
/// #     let mut builder = ServiceApiBuilder::new(
/// #         blockchain,
/// #         InstanceDescriptor::new(1100, "example"),
/// #     );
/// #     wire_api(&mut builder);
/// # }
/// ```
#[derive(Debug)]
pub struct ServiceApiBuilder {
    blockchain: Blockchain,
    public_scope: ServiceApiScope,
    private_scope: ServiceApiScope,
    root_path: Option<String>,
}

impl ServiceApiBuilder {
    /// Create a new service API builder for the specified service instance.
    #[doc(hidden)]
    pub fn new(blockchain: Blockchain, instance: InstanceDescriptor) -> Self {
        Self {
            blockchain: blockchain.clone(),
            public_scope: ServiceApiScope::new(blockchain.clone(), &instance),
            private_scope: ServiceApiScope::new(blockchain, &instance),
            root_path: None,
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

    /// Overrides the service root path as opposed to the default `services/$service_name`.
    ///
    /// # Safety
    ///
    /// The caller is responsible for the path not interfering with root paths of other services
    /// or that of the Rust runtime (`runtimes/rust`).
    #[doc(hidden)]
    pub fn with_root_path(&mut self, root_path: impl Into<String>) -> &mut Self {
        let root_path = root_path.into();
        self.root_path = Some(root_path);
        self
    }

    /// Takes the root path associated redefined by the service. If the service didn't redefine
    /// the root path, returns `None`.
    pub(super) fn take_root_path(&mut self) -> Option<String> {
        self.root_path.take()
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
