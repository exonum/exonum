// Copyright 2018 The Exonum Team
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

//! API and corresponding utilities.

pub use self::error::Error;
pub use self::state::ServiceApiState;
pub use self::with::{FutureResult, Immutable, Mutable, NamedWith, Result, With};

use serde::de::DeserializeOwned;
use serde::Serialize;

use std::collections::BTreeMap;

use self::backends::actix;
use blockchain::{Blockchain, SharedNodeState};

pub mod backends;
pub mod error;
pub mod node;
mod state;
mod with;

/// Trait defines object that could be used as an API backend.
pub trait ServiceApiBackend: Sized {
    /// Concrete endpoint handler in the backend.
    type Handler;
    /// Concrete backend API builder.
    type Backend;

    /// Adds the given endpoint handler to the backend.
    fn endpoint<N, Q, I, R, F, E>(&mut self, name: N, endpoint: E) -> &mut Self
    where
        N: Into<String>,
        Q: DeserializeOwned + 'static,
        I: Serialize + 'static,
        F: for<'r> Fn(&'r ServiceApiState, Q) -> R + 'static + Clone,
        E: Into<With<Q, I, R, F, Immutable>>,
        Self::Handler: From<NamedWith<Q, I, R, F, Immutable>>,
    {
        let named_with = NamedWith {
            name: name.into(),
            inner: endpoint.into(),
        };
        self.raw_handler(Self::Handler::from(named_with))
    }

    /// Adds the given mutable endpoint handler to the backend.
    fn endpoint_mut<N, Q, I, R, F, E>(&mut self, name: N, endpoint: E) -> &mut Self
    where
        N: Into<String>,
        Q: DeserializeOwned + 'static,
        I: Serialize + 'static,
        F: for<'r> Fn(&'r ServiceApiState, Q) -> R + 'static + Clone,
        E: Into<With<Q, I, R, F, Mutable>>,
        Self::Handler: From<NamedWith<Q, I, R, F, Mutable>>,
    {
        let named_with = NamedWith {
            name: name.into(),
            inner: endpoint.into(),
        };
        self.raw_handler(Self::Handler::from(named_with))
    }

    /// Adds the raw endpoint handler for the given backend.
    fn raw_handler(&mut self, handler: Self::Handler) -> &mut Self;

    /// Binds API handlers with to given backend.
    fn wire(&self, output: Self::Backend) -> Self::Backend;
}

/// Service API builder for the concrete scope.
#[derive(Debug, Clone, Default)]
pub struct ServiceApiScope {
    pub(crate) actix_backend: actix::ApiBuilder,
}

impl ServiceApiScope {
    /// Creates a new service api scope.
    pub fn new() -> ServiceApiScope {
        ServiceApiScope::default()
    }

    /// Adds the given endpoint handler to the api scope.
    pub fn endpoint<Q, I, R, F, E>(&mut self, name: &'static str, endpoint: E) -> &mut Self
    where
        Q: DeserializeOwned + 'static,
        I: Serialize + 'static,
        F: for<'r> Fn(&'r ServiceApiState, Q) -> R + 'static + Clone,
        E: Into<With<Q, I, R, F, Immutable>>,
        actix::RequestHandler: From<NamedWith<Q, I, R, F, Immutable>>,
    {
        self.actix_backend.endpoint(name, endpoint);
        self
    }

    /// Adds the given mutable endpoint handler to the api scope.
    pub fn endpoint_mut<Q, I, R, F, E>(&mut self, name: &'static str, endpoint: E) -> &mut Self
    where
        Q: DeserializeOwned + 'static,
        I: Serialize + 'static,
        F: for<'r> Fn(&'r ServiceApiState, Q) -> R + 'static + Clone,
        E: Into<With<Q, I, R, F, Mutable>>,
        actix::RequestHandler: From<NamedWith<Q, I, R, F, Mutable>>,
    {
        self.actix_backend.endpoint_mut(name, endpoint);
        self
    }

    /// Returns reference to the underlying web backend.
    pub fn web_backend(&mut self) -> &mut actix::ApiBuilder {
        &mut self.actix_backend
    }
}

/// Service API builder, which is used to add service-specific endpoints to the node API.
///
/// # Examples
///
/// The example below provides a common practice of API implementation.
///
/// ```
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
/// use exonum::crypto::Hash;
///
/// // Declares type which describes an API specification and implementation.
/// pub struct MyApi;
///
/// // Declares structures for requests and responses.
/// #[derive(Deserialize, Clone, Copy)]
/// pub struct MyQuery {
///     block_height: u64
/// }
///
/// // Creates API handlers.
/// impl MyApi {
///     /// Immutable handler, which returns the hash for block with the given height.
///     pub fn block_hash(state: &ServiceApiState, query: MyQuery) -> api::Result<Option<Hash>> {
///         let schema = Schema::new(state.snapshot());
///         Ok(schema.block_hashes_by_height().get(query.block_height))
///     }
///
///     /// Mutable handler which removes peer with the given address from the cache.
///     pub fn remove_peer(state: &ServiceApiState, query: SocketAddr) -> api::Result<()> {
///         let mut blockchain = state.blockchain().clone();
///         Ok(blockchain.remove_peer_with_addr(&query))
///     }
///
///     /// Simple handler without any params.
///     pub fn ping(_state: &ServiceApiState, _query: ()) -> api::Result<()> {
///         Ok(())
///     }
///
///     /// You may also creates an asynchronous handlers for the long requests.
///     pub fn block_hash_async(state: &ServiceApiState, query: MyQuery)
///      -> api::FutureResult<Option<Hash>> {
///         let blockchain = state.blockchain().clone();
///         Box::new(futures::lazy(move || {
///             let schema = Schema::new(blockchain.snapshot());
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
#[derive(Debug, Clone, Default)]
pub struct ServiceApiBuilder {
    public_scope: ServiceApiScope,
    private_scope: ServiceApiScope,
}

impl ServiceApiBuilder {
    /// Creates a new service API builder.
    pub fn new() -> ServiceApiBuilder {
        ServiceApiBuilder::default()
    }

    /// Returns reference to the public api scope builder.
    pub fn public_scope(&mut self) -> &mut ServiceApiScope {
        &mut self.public_scope
    }

    /// Returns reference to the private api scope builder.
    pub fn private_scope(&mut self) -> &mut ServiceApiScope {
        &mut self.private_scope
    }
}

/// Exonum API access level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiAccess {
    /// Public API for end users.
    Public,
    /// Private API for maintainers.
    Private,
}

impl ::std::fmt::Display for ApiAccess {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        match *self {
            ApiAccess::Public => f.write_str("public"),
            ApiAccess::Private => f.write_str("private"),
        }
    }
}

/// API backend extender.
pub trait ExtendApiBackend {
    /// Extend API backend by the given scopes.
    fn extend<'a, I>(self, items: I) -> Self
    where
        I: IntoIterator<Item = (&'a str, &'a ServiceApiScope)>;
}

/// Exonum node API aggregator. Currently only HTTP v1 backend is available.
#[derive(Debug, Clone)]
pub struct ApiAggregator {
    blockchain: Blockchain,
    node_state: SharedNodeState,
    inner: BTreeMap<String, ServiceApiBuilder>,
}

impl ApiAggregator {
    /// Aggregates API for the given blockchain and node state.
    pub fn new(blockchain: Blockchain, node_state: SharedNodeState) -> ApiAggregator {
        let mut inner = BTreeMap::new();
        // Adds built-in APIs.
        inner.insert(
            "system".to_owned(),
            Self::system_api(&blockchain, node_state.clone()),
        );
        inner.insert("explorer".to_owned(), Self::explorer_api());
        // Adds services APIs.
        inner.extend(blockchain.service_map().iter().map(|(_, service)| {
            let mut builder = ServiceApiBuilder::new();
            service.wire_api(&mut builder);
            // TODO think about prefixes for non web backends.
            let prefix = format!("services/{}", service.service_name());
            (prefix, builder)
        }));

        ApiAggregator {
            inner,
            blockchain,
            node_state,
        }
    }

    /// Returns a reference to the blockchain of this node.
    pub fn blockchain(&self) -> &Blockchain {
        &self.blockchain
    }

    /// Extends given API backend by the handlers with the given access level.
    pub fn extend_backend<B: ExtendApiBackend>(&self, access: ApiAccess, backend: B) -> B {
        match access {
            ApiAccess::Public => backend.extend(
                self.inner
                    .iter()
                    .map(|(name, builder)| (name.as_ref(), &builder.public_scope)),
            ),
            ApiAccess::Private => backend.extend(
                self.inner
                    .iter()
                    .map(|(name, builder)| (name.as_ref(), &builder.private_scope)),
            ),
        }
    }

    /// Adds to aggregator API factory for the given prefix.
    pub fn insert<S: Into<String>>(&mut self, prefix: S, builder: ServiceApiBuilder) {
        self.inner.insert(prefix.into(), builder);
    }

    fn explorer_api() -> ServiceApiBuilder {
        let mut builder = ServiceApiBuilder::new();
        self::node::public::ExplorerApi::wire(builder.public_scope());
        builder
    }

    fn system_api(blockchain: &Blockchain, shared_api_state: SharedNodeState) -> ServiceApiBuilder {
        let mut builder = ServiceApiBuilder::new();
        let node_info = self::node::private::NodeInfo::new(
            blockchain.service_map().iter().map(|(_, service)| service),
        );
        self::node::private::SystemApi::new(node_info, shared_api_state.clone())
            .wire(builder.private_scope());
        self::node::public::SystemApi::new(shared_api_state).wire(builder.public_scope());
        builder
    }
}
