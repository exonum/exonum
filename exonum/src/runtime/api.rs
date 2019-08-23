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

//! Building blocks for creating API of services.

pub use crate::api::{ApiContext, Error, FutureResult, Result};

use exonum_merkledb::Snapshot;
use futures::IntoFuture;
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    api::{ApiBuilder, ApiScope},
    crypto::{PublicKey, SecretKey},
    node::ApiSender,
    runtime::{InstanceDescriptor, InstanceId},
};

/// Provide the current blockchain state snapshot to API handlers.
///
/// This structure allows a service API handler to interact with the service instance
/// and other parts of the blockchain.
#[derive(Debug)]
pub struct ServiceApiState<'a> {
    /// Service instance descriptor of the current API handler.
    pub instance: InstanceDescriptor<'a>,
    /// Service key pair of the current node.
    pub service_keypair: &'a (PublicKey, SecretKey),
    api_sender: &'a ApiSender,
    // TODO Think about avoiding of unnecessary snapshots creation. [ECR-3222]
    snapshot: Box<dyn Snapshot>,
}

impl<'a> ServiceApiState<'a> {
    /// Create service API state snapshot from the given context and instance descriptor.
    pub fn from_api_context(context: &'a ApiContext, instance: InstanceDescriptor<'a>) -> Self {
        Self {
            service_keypair: context.service_keypair(),
            instance,
            api_sender: context.sender(),
            snapshot: context.snapshot(),
        }
    }

    /// Return a service instance descriptor of the current API handler.
    pub fn instance(&self) -> InstanceDescriptor {
        self.instance
    }

    /// Return a read-only snapshot of the current blockchain state.
    pub fn snapshot(&'a self) -> &dyn Snapshot {
        self.snapshot.as_ref()
    }

    /// Return reference to the service key pair of the current node.
    pub fn service_keypair(&self) -> &(PublicKey, SecretKey) {
        self.service_keypair
    }

    /// Return a reference to the transactions sender.
    pub fn sender(&self) -> &ApiSender {
        self.api_sender
    }
}

/// Exonum API builder for the concrete service API [scope].
///
/// [scope]: ../../api/struct.ApiScope.html
#[derive(Debug, Clone)]
pub struct ServiceApiScope {
    inner: ApiScope,
    context: ApiContext,
    descriptor: (InstanceId, String),
}

impl ServiceApiScope {
    /// Create a new service API scope for the specified service instance.
    pub fn new(context: ApiContext, instance_descriptor: InstanceDescriptor) -> Self {
        Self {
            inner: ApiScope::new(),
            context,
            descriptor: instance_descriptor.into(),
        }
    }

    /// Add a readonly endpoint handler to the service API scope.
    ///
    /// In HTTP backends this type of endpoint corresponds to `GET` requests.
    /// [Read more.](../../api/struct.ApiScope.html#endpoint)
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

    /// Add an endpoint handler to the service API scope.
    ///
    /// In HTTP backends this type of endpoint corresponds to `POST` requests.
    /// [Read more.](../../api/struct.ApiScope.html#endpoint_mut)
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
/// ```rust
/// use serde_derive::{Deserialize, Serialize};
///
/// use exonum::{
///     blockchain::Schema,
///     crypto::{self, Hash},
///     node::ExternalMessage,
///     runtime::api::{self, ServiceApiBuilder, ServiceApiState},
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
///     // Immutable handler which returns a hash of the block at the given height.
///     pub fn block_hash(state: &ServiceApiState, query: MyQuery) -> api::Result<Option<BlockInfo>> {
///         let schema = Schema::new(state.snapshot());
///         Ok(schema
///             .block_hashes_by_height()
///             .get(query.block_height)
///             .map(|hash| BlockInfo { hash }))
///     }
///
///     // Mutable handler which sends `Rebroadcast` request to the node.
///     pub fn rebroadcast(state: &ServiceApiState, _query: ()) -> api::Result<()> {
///         state
///             .sender()
///             .send_external_message(ExternalMessage::Rebroadcast)
///             .map_err(From::from)
///     }
///
///     // Simple handler without any parameters.
///     pub fn ping(_state: &ServiceApiState, _query: ()) -> api::Result<()> {
///         Ok(())
///     }
///
///     // You may also create asynchronous handlers for long requests.
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
///         .endpoint("v1/ping", MyApi::ping)
///         .endpoint("v1/block_hash", MyApi::block_hash)
///         .endpoint("v1/async_operation", MyApi::async_operation);
///     // Add a mutable endpoint for to the private API.
///     builder
///         .private_scope()
///         .endpoint_mut("v1/rebroadcast", MyApi::rebroadcast);
///     builder
/// }
///
/// # fn main() {
/// #     use exonum::{api::ApiContext, node::ApiSender, runtime::InstanceDescriptor};
/// #     use exonum_merkledb::TemporaryDB;
/// #     use futures::sync::mpsc;
/// #
/// #     let context = ApiContext::new(
/// #         TemporaryDB::new().into(),
/// #         crypto::gen_keypair(),
/// #         ApiSender::new(mpsc::channel(0).0),
/// #     );
/// #     let mut builder = ServiceApiBuilder::new(
/// #         context,
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
    context: ApiContext,
    public_scope: ServiceApiScope,
    private_scope: ServiceApiScope,
}

impl ServiceApiBuilder {
    /// Create a new service API builder for the specified service instance.
    #[doc(hidden)]
    pub fn new(context: ApiContext, instance_descriptor: InstanceDescriptor) -> Self {
        Self {
            context: context.clone(),
            public_scope: ServiceApiScope::new(context.clone(), instance_descriptor),
            private_scope: ServiceApiScope::new(context, instance_descriptor),
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

    /// Return a reference to the underlying API context.
    pub fn context(&self) -> &ApiContext {
        &self.context
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
