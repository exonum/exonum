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
    with::{Actuality, FutureResult, Immutable, Mutable, NamedWith, Result, With},
};

pub mod backends;
pub mod error;
pub mod manager;
pub mod node;
pub mod websocket;

use std::{collections::BTreeMap, fmt};

use chrono::{Date, Utc};
use serde::{de::DeserializeOwned, Serialize};

use self::{
    backends::actix,
    node::{
        private::{NodeInfo, SystemApi as PrivateSystemApi},
        public::{ExplorerApi, SystemApi},
    },
};
use crate::{api::node::SharedNodeState, blockchain::Blockchain};

mod with;

/// Mutability of the endpoint. Used for auto-generated endpoints, e.g.
/// in `moved_permanently` method.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum EndpointMutability {
    /// Endpoint should process POST requests.
    Mutable,
    /// Endpoint should process GET requests.
    Immutable,
}

/// This trait is used to implement an API backend for Exonum.
pub trait ApiBackend: Sized {
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
        F: Fn(Q) -> R + 'static + Clone,
        E: Into<With<Q, I, R, F>>,
        Self::Handler: From<NamedWith<Q, I, R, F, Immutable>>,
    {
        let named_with = NamedWith::new(name, endpoint, Actuality::Actual);
        self.raw_handler(Self::Handler::from(named_with))
    }

    /// Adds the given mutable endpoint handler to the backend.
    fn endpoint_mut<N, Q, I, R, F, E>(&mut self, name: N, endpoint: E) -> &mut Self
    where
        N: Into<String>,
        Q: DeserializeOwned + 'static,
        I: Serialize + 'static,
        F: Fn(Q) -> R + 'static + Clone,
        E: Into<With<Q, I, R, F>>,
        Self::Handler: From<NamedWith<Q, I, R, F, Mutable>>,
    {
        let named_with = NamedWith::new(name, endpoint, Actuality::Actual);
        self.raw_handler(Self::Handler::from(named_with))
    }

    /// Adds the given endpoint handler to the backend, marking it as deprecated.
    fn deprecated_endpoint<N, Q, I, R, F, E>(
        &mut self,
        name: N,
        endpoint: E,
        discontinued_on: Option<Date<Utc>>,
    ) -> &mut Self
    where
        N: Into<String>,
        Q: DeserializeOwned + 'static,
        I: Serialize + 'static,
        F: Fn(Q) -> R + 'static + Clone,
        E: Into<With<Q, I, R, F>>,
        Self::Handler: From<NamedWith<Q, I, R, F, Immutable>>,
    {
        let named_with = NamedWith::new(name, endpoint, Actuality::Deprecated(discontinued_on));
        self.raw_handler(Self::Handler::from(named_with))
    }

    /// Adds the given mutable endpoint handler to the backend, marking it as deprecated.
    fn deprecated_endpoint_mut<N, Q, I, R, F, E>(
        &mut self,
        name: N,
        endpoint: E,
        discontinued_on: Option<Date<Utc>>,
    ) -> &mut Self
    where
        N: Into<String>,
        Q: DeserializeOwned + 'static,
        I: Serialize + 'static,
        F: Fn(Q) -> R + 'static + Clone,
        E: Into<With<Q, I, R, F>>,
        Self::Handler: From<NamedWith<Q, I, R, F, Mutable>>,
    {
        let named_with = NamedWith::new(name, endpoint, Actuality::Deprecated(discontinued_on));
        self.raw_handler(Self::Handler::from(named_with))
    }

    /// Creates an endpoint which will return "301 Moved Permanently" HTTP status code
    /// to the incoming requests.
    /// Responce will include a "Location" header denoting a new location of the resourse.
    fn moved_permanently(
        &mut self,
        name: &'static str,
        new_location: &'static str,
        mutability: EndpointMutability,
    ) -> &mut Self;

    /// Creates an endpoint which will return "410 Gone" HTTP status code
    /// to the incoming requests.
    fn gone(&mut self, name: &'static str, mutability: EndpointMutability) -> &mut Self;

    /// Add the raw endpoint handler for the given backend.
    fn raw_handler(&mut self, handler: Self::Handler) -> &mut Self;

    /// Bind API handlers to the given backend.
    fn wire(&self, output: Self::Backend) -> Self::Backend;
}

/// Exonum API builder for the concrete API scope or, in other words,
/// API access level (public or private).
///
/// Endpoints cannot be declared to the builder directly, first you need to
/// indicate the scope the endpoint(s) will belong to.
#[derive(Debug, Clone, Default)]
pub struct ApiScope {
    pub(crate) actix_backend: actix::ApiBuilder,
}

impl ApiScope {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds the given endpoint handler to the API scope. These endpoints
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

    /// Adds the given mutable endpoint handler to the API scope. These endpoints
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

    /// Same as `endpoint`, but also add a warning about this endpoint being deprecated to the response.
    pub fn deprecated_endpoint<Q, I, R, F, E>(
        &mut self,
        name: &'static str,
        endpoint: E,
        discontinued_on: Option<Date<Utc>>,
    ) -> &mut Self
    where
        Q: DeserializeOwned + 'static,
        I: Serialize + 'static,
        F: Fn(Q) -> R + 'static + Clone,
        E: Into<With<Q, I, R, F>>,
        actix::RequestHandler: From<NamedWith<Q, I, R, F, Immutable>>,
    {
        self.actix_backend
            .deprecated_endpoint(name, endpoint, discontinued_on);
        self
    }

    /// Same as `endpoint_mut`, but also add a warning about this endpoint being deprecated to the response.
    pub fn deprecated_endpoint_mut<Q, I, R, F, E>(
        &mut self,
        name: &'static str,
        endpoint: E,
        discontinued_on: Option<Date<Utc>>,
    ) -> &mut Self
    where
        Q: DeserializeOwned + 'static,
        I: Serialize + 'static,
        F: Fn(Q) -> R + 'static + Clone,
        E: Into<With<Q, I, R, F>>,
        actix::RequestHandler: From<NamedWith<Q, I, R, F, Mutable>>,
    {
        self.actix_backend
            .deprecated_endpoint_mut(name, endpoint, discontinued_on);
        self
    }

    /// Creates an endpoint which will return "301 Moved Permanently" HTTP status code
    /// to the incoming requests.
    /// Responce will include a "Location" header denoting a new location of the resourse.
    pub fn moved_permanently(
        &mut self,
        name: &'static str,
        new_location: &'static str,
        mutability: EndpointMutability,
    ) -> &mut Self {
        self.actix_backend
            .moved_permanently(name, new_location, mutability);
        self
    }

    /// Creates an endpoint which will return "410 Gone" HTTP status code
    /// to the incoming requests.
    pub fn gone(&mut self, name: &'static str, mutability: EndpointMutability) -> &mut Self {
        self.actix_backend.gone(name, mutability);
        self
    }

    /// Returns a mutable reference to the underlying web backend.
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
    endpoints: BTreeMap<String, ApiBuilder>,
}

impl ApiAggregator {
    /// Creates an API aggregator for the given blockchain and node state.
    pub fn new(blockchain: Blockchain, node_state: SharedNodeState) -> Self {
        let mut endpoints = BTreeMap::new();
        endpoints.insert(
            "system".to_owned(),
            Self::system_api(blockchain.clone(), node_state.clone()),
        );
        endpoints.insert(
            "explorer".to_owned(),
            Self::explorer_api(blockchain, node_state),
        );
        Self { endpoints }
    }

    /// Inserts a handler for a set of endpoints with the given mount point.
    pub fn insert(&mut self, name: &str, api: ApiBuilder) {
        self.endpoints.insert(name.to_owned(), api);
    }

    /// Extends the list of endpoint handlers with the new specified handlers.
    pub fn extend(&mut self, endpoints: impl IntoIterator<Item = (String, ApiBuilder)>) {
        self.endpoints.extend(endpoints);
    }

    /// Extend the given API backend by handlers with the given access level.
    pub fn extend_backend<B: ExtendApiBackend>(&self, access: ApiAccess, backend: B) -> B {
        let endpoints = self.endpoints.iter();
        match access {
            ApiAccess::Public => backend
                .extend(endpoints.map(|(name, builder)| (name.as_str(), &builder.public_scope))),
            ApiAccess::Private => backend
                .extend(endpoints.map(|(name, builder)| (name.as_str(), &builder.private_scope))),
        }
    }

    fn explorer_api(blockchain: Blockchain, shared_node_state: SharedNodeState) -> ApiBuilder {
        let mut builder = ApiBuilder::new();
        ExplorerApi::new(blockchain).wire(builder.public_scope(), shared_node_state);
        builder
    }

    fn system_api(blockchain: Blockchain, shared_api_state: SharedNodeState) -> ApiBuilder {
        let mut builder = ApiBuilder::new();
        let sender = blockchain.sender().clone();
        PrivateSystemApi::new(sender, NodeInfo::new(), shared_api_state.clone())
            .wire(builder.private_scope());
        SystemApi::new(blockchain, shared_api_state).wire(builder.public_scope());
        builder
    }
}
