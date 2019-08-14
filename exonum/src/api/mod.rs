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

//! API and corresponding utilities.

pub use self::{
    error::Error,
    with::{FutureResult, Immutable, Mutable, NamedWith, Result, With},
};

pub mod backends;
pub mod error;
pub mod manager;
pub mod node;
pub mod websocket;

use exonum_merkledb::{Database, Snapshot};
use serde::{de::DeserializeOwned, Serialize};

use std::{collections::BTreeMap, fmt, sync::Arc};

use self::{
    backends::actix,
    node::{private::NodeInfo, public::ExplorerApi},
};
use crate::{
    api::node::SharedNodeState,
    blockchain::Blockchain,
    crypto::{PublicKey, SecretKey},
    node::ApiSender,
};

mod with;

/// This trait is used to implement an API backend for Exonum.
pub trait ApiBackend: Sized {
    /// Concrete endpoint handler in the backend.
    type Handler;
    /// Concrete backend API builder.
    type Backend;

    /// Add the given endpoint handler to the backend.
    fn endpoint<N, Q, I, R, F, E>(&mut self, name: N, endpoint: E) -> &mut Self
    where
        N: Into<String>,
        Q: DeserializeOwned + 'static,
        I: Serialize + 'static,
        F: Fn(Q) -> R + 'static + Clone,
        E: Into<With<Q, I, R, F>>,
        Self::Handler: From<NamedWith<Q, I, R, F, Immutable>>,
    {
        let named_with = NamedWith::new(name, endpoint);
        self.raw_handler(Self::Handler::from(named_with))
    }

    /// Add the given mutable endpoint handler to the backend.
    fn endpoint_mut<N, Q, I, R, F, E>(&mut self, name: N, endpoint: E) -> &mut Self
    where
        N: Into<String>,
        Q: DeserializeOwned + 'static,
        I: Serialize + 'static,
        F: Fn(Q) -> R + 'static + Clone,
        E: Into<With<Q, I, R, F>>,
        Self::Handler: From<NamedWith<Q, I, R, F, Mutable>>,
    {
        let named_with = NamedWith::new(name, endpoint);
        self.raw_handler(Self::Handler::from(named_with))
    }

    /// Add the raw endpoint handler for the given backend.
    fn raw_handler(&mut self, handler: Self::Handler) -> &mut Self;

    /// Bind API handlers to the given backend.
    fn wire(&self, output: Self::Backend) -> Self::Backend;
}

/// Exonum API builder for the concrete API scope or in other words
/// access level (public or private).
///
/// Endpoints cannot be declared to the builder directly, first you need to
/// indicate the scope the endpoint(s) will belong to.
#[derive(Debug, Clone, Default)]
pub struct ApiScope {
    pub(crate) actix_backend: actix::ApiBuilder,
}

impl ApiScope {
    /// Create a new instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add the given endpoint handler to the API scope. These endpoints
    /// are designed for reading operations.
    ///
    /// For now there is only web backend and it has the following requirements:
    ///
    /// - Query parameters should be decodable via `serde_urlencoded`, i.e. from the
    ///   "first_param=value1&second_param=value2" form.
    /// - Response items should be encodable via `serde_json` crate.
    pub fn endpoint<Q, I, R, F, E>(&mut self, name: &'static str, endpoint: E) -> &mut Self
    where
        Q: DeserializeOwned + 'static,
        I: Serialize + 'static,
        F: Fn(Q) -> R + 'static + Clone,
        E: Into<With<Q, I, R, F>>,
        actix::RequestHandler: From<NamedWith<Q, I, R, F, Immutable>>,
    {
        self.actix_backend.endpoint(name, endpoint);
        self
    }

    /// Add the given mutable endpoint handler to the API scope. These endpoints
    /// are designed for modification operations.
    ///
    /// For now there is only web backend and it has the following requirements:
    ///
    /// - Query parameters should be decodable via `serde_json`.
    /// - Response items also should be encodable via `serde_json` crate.
    pub fn endpoint_mut<Q, I, R, F, E>(&mut self, name: &'static str, endpoint: E) -> &mut Self
    where
        Q: DeserializeOwned + 'static,
        I: Serialize + 'static,
        F: Fn(Q) -> R + 'static + Clone,
        E: Into<With<Q, I, R, F>>,
        actix::RequestHandler: From<NamedWith<Q, I, R, F, Mutable>>,
    {
        self.actix_backend.endpoint_mut(name, endpoint);
        self
    }

    /// Return a mutable reference to the underlying web backend.
    pub fn web_backend(&mut self) -> &mut actix::ApiBuilder {
        &mut self.actix_backend
    }
}

/// Exonum API builder, which is used to add endpoints to the node API.
#[derive(Debug, Clone, Default)]
pub struct ApiBuilder {
    pub(crate) public_scope: ApiScope,
    pub(crate) private_scope: ApiScope,
}

impl ApiBuilder {
    /// Create a new API builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Return a mutable reference to the public API scope builder.
    pub fn public_scope(&mut self) -> &mut ApiScope {
        &mut self.public_scope
    }

    /// Return a mutable reference to the private API scope builder.
    pub fn private_scope(&mut self) -> &mut ApiScope {
        &mut self.private_scope
    }
}

/// Exonum API access level, either private or public.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiAccess {
    /// Public API for end users.
    Public,
    /// Private API for maintainers.
    Private,
}

impl fmt::Display for ApiAccess {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ApiAccess::Public => f.write_str("public"),
            ApiAccess::Private => f.write_str("private"),
        }
    }
}

/// API backend extender.
///
/// This trait enables implementing additional API scopes, besides the built-in
/// private and public scopes.
pub trait ExtendApiBackend {
    /// Extend API backend by the given scopes.
    fn extend<'a, I>(self, items: I) -> Self
    where
        I: IntoIterator<Item = (&'a str, &'a ApiScope)>;
}

/// Exonum node API aggregator. This structure enables several API backends to
/// operate simultaneously. Currently, only HTTP v1 backend is available.
#[derive(Debug, Clone)]
pub struct ApiAggregator {
    blockchain: Blockchain,
    node_state: SharedNodeState,
    endpoints: BTreeMap<String, ApiBuilder>,
}

impl ApiAggregator {
    /// Aggregate API for the given blockchain and node state.
    pub fn new(blockchain: Blockchain, node_state: SharedNodeState) -> Self {
        let mut endpoints = BTreeMap::new();
        // Adds built-in APIs.
        let context = ApiContext::with_blockchain(&blockchain);
        endpoints.insert(
            "system".to_owned(),
            Self::system_api(context.clone(), node_state.clone()),
        );
        endpoints.insert(
            "explorer".to_owned(),
            Self::explorer_api(context.clone(), node_state.clone()),
        );
        Self {
            endpoints,
            blockchain,
            node_state,
        }
    }

    /// Return a reference to the blockchain used by the aggregator.
    pub fn blockchain(&self) -> &Blockchain {
        &self.blockchain
    }

    /// Extend the given API backend by handlers with the given access level.
    pub fn extend_backend<B: ExtendApiBackend>(&self, access: ApiAccess, backend: B) -> B {
        let mut endpoints = self.endpoints.clone();

        let blockchain = self.blockchain.clone();
        let dispatcher = self.blockchain.dispatcher();
        let context = ApiContext::with_blockchain(&blockchain);
        endpoints.extend(dispatcher.api_endpoints(&context));

        trace!("Create actix-web worker with api: {:#?}", endpoints);

        match access {
            ApiAccess::Public => backend.extend(
                endpoints
                    .iter()
                    .map(|(name, builder)| (name.as_ref(), &builder.public_scope)),
            ),
            ApiAccess::Private => backend.extend(
                endpoints
                    .iter()
                    .map(|(name, builder)| (name.as_ref(), &builder.private_scope)),
            ),
        }
    }

    /// Add API factory with the given prefix to the aggregator.
    pub fn insert<S: Into<String>>(&mut self, prefix: S, builder: ApiBuilder) {
        self.endpoints.insert(prefix.into(), builder);
    }

    /// Refresh shared node state.
    fn refresh(&self) {
        self.node_state.update_dispatcher_state(&self.blockchain);
    }

    fn explorer_api(context: ApiContext, shared_node_state: SharedNodeState) -> ApiBuilder {
        let mut builder = ApiBuilder::new();

        ExplorerApi::new(context).wire(builder.public_scope(), shared_node_state);
        builder
    }

    fn system_api(context: ApiContext, shared_api_state: SharedNodeState) -> ApiBuilder {
        let mut builder = ApiBuilder::new();

        let sender = context.sender().clone();
        self::node::private::SystemApi::new(sender, NodeInfo::new(), shared_api_state.clone())
            .wire(builder.private_scope());
        self::node::public::SystemApi::new(context, shared_api_state).wire(builder.public_scope());
        builder
    }
}

/// This entity provides the current state of the blockchain to the API handlers.
///
/// This context contains a necessary parts for the blockchain interaction
/// and may be shared between any kind of handlers.
#[derive(Debug, Clone)]
pub struct ApiContext {
    service_keypair: (PublicKey, SecretKey),
    api_sender: ApiSender,
    database: Arc<dyn Database>,
}

impl ApiContext {
    /// Create a new API context instance from the specified blockchain parts.
    #[doc(hidden)]
    pub fn new(
        database: Arc<dyn Database>,
        service_keypair: (PublicKey, SecretKey),
        sender: ApiSender,
    ) -> Self {
        Self {
            database,
            service_keypair,
            api_sender: sender,
        }
    }

    /// Create a new API context instance for the specified blockchain.
    #[doc(hidden)]
    pub fn with_blockchain(blockchain: &Blockchain) -> Self {
        Self {
            service_keypair: blockchain.service_keypair.clone(),
            api_sender: blockchain.api_sender.clone(),
            database: blockchain.db.clone(),
        }
    }

    /// Create a new blockchain database state snapshot.
    ///
    /// Be careful with this method! It is recommended to get a snapshot only once per request
    /// to ensure all the database accesses see the same state. When the snapshot is requested
    /// from a different thread than the one where changes are made to the blockchain,
    /// each invocation might get a snapshot corresponding to a different blockchain state,
    /// likely leading to a race condition.
    pub fn snapshot(&self) -> Box<dyn Snapshot> {
        self.database.snapshot()
    }

    /// Return reference to the transactions sender.
    pub fn sender(&self) -> &ApiSender {
        &self.api_sender
    }

    /// Return reference to the service key pair of the current node.
    pub fn service_keypair(&self) -> (&PublicKey, &SecretKey) {
        (&self.service_keypair.0, &self.service_keypair.1)
    }
}
