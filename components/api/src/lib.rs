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

//! High-level wrapper around a web server used by the Exonum framework.
//!
//! The core APIs of this crate are designed to be reasonably independent from the web server
//! implementation. [`actix`] is currently used as the server backend.
//!
//! The wrapper is used in [Rust services][rust-runtime] and in plugins
//! for the Exonum node. The Rust runtime provides its own abstractions based on the wrapper;
//! consult its docs for details. Node plugins use [`ApiBuilder`] directly.
//!
//! [`actix`]: https://crates.io/crates/actix
//! [rust-runtime]: https://crates.io/crates/exonum-rust-runtime
//! [`ApiBuilder`]: struct.ApiBuilder.html
//!
//! # Examples
//!
//! Providing HTTP API for a plugin:
//!
//! ```
//! use exonum_api::{ApiBuilder};
//! # use serde::{Deserialize, Serialize};
//!
//! #[derive(Serialize, Deserialize)]
//! pub struct SomeQuery {
//!     pub first: u64,
//!     pub second: u64,
//! }
//!
//! fn create_api() -> ApiBuilder {
//!     let mut builder = ApiBuilder::new();
//!     builder
//!         .public_scope()
//!         .endpoint("some", |query: SomeQuery| async move {
//!             Ok(query.first + query.second)
//!         });
//!     builder
//! }
//!
//! let builder = create_api();
//! // `builder` can now be passed to the node via plugin interface
//! // or via node channel.
//! ```

#![deny(
    unsafe_code,
    bare_trait_objects,
    missing_docs,
    missing_debug_implementations
)]

pub use self::{
    cors::AllowOrigin,
    error::{Error, ErrorBody, HttpStatusCode, MovedPermanentlyError},
    manager::{ApiManager, ApiManagerConfig, UpdateEndpoints, WebServerConfig},
    with::{Actuality, Deprecated, NamedWith, Result, With},
};

pub mod backends;
mod cors;
mod error;
mod manager;
mod with;

use serde::{de::DeserializeOwned, Serialize};

use std::{collections::BTreeMap, fmt, future::Future};

use crate::backends::actix;

/// Mutability of the endpoint. Used for auto-generated endpoints, e.g.
/// in `moved_permanently` method.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[non_exhaustive]
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
    fn endpoint<Q, I, R, F, E>(&mut self, name: &str, endpoint: E) -> &mut Self
    where
        Q: DeserializeOwned + 'static,
        I: Serialize + 'static,
        F: Fn(Q) -> R + 'static + Clone,
        E: Into<With<Q, I, R, F>>,
        Self::Handler: From<NamedWith<Q, I, R, F>>,
    {
        let named_with = NamedWith::immutable(name, endpoint);
        self.raw_handler(Self::Handler::from(named_with))
    }

    /// Adds the given mutable endpoint handler to the backend.
    fn endpoint_mut<Q, I, R, F, E>(&mut self, name: &str, endpoint: E) -> &mut Self
    where
        Q: DeserializeOwned + 'static,
        I: Serialize + 'static,
        F: Fn(Q) -> R + 'static + Clone,
        E: Into<With<Q, I, R, F>>,
        Self::Handler: From<NamedWith<Q, I, R, F>>,
    {
        let named_with = NamedWith::mutable(name, endpoint);
        self.raw_handler(Self::Handler::from(named_with))
    }

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
    ///   `first_param=value1&second_param=value2` form.
    /// - Response items should be encodable via `serde_json` crate.
    pub fn endpoint<Q, I, R, F, E>(&mut self, name: &str, endpoint: E) -> &mut Self
    where
        Q: DeserializeOwned + 'static,
        I: Serialize + 'static,
        F: Fn(Q) -> R + 'static + Clone + Send + Sync,
        E: Into<With<Q, I, R, F>>,
        R: Future<Output = crate::Result<I>>,
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
    pub fn endpoint_mut<Q, I, R, F, E>(&mut self, name: &str, endpoint: E) -> &mut Self
    where
        Q: DeserializeOwned + 'static,
        I: Serialize + 'static,
        F: Fn(Q) -> R + 'static + Clone + Send + Sync,
        E: Into<With<Q, I, R, F>>,
        R: Future<Output = crate::Result<I>>,
    {
        self.actix_backend.endpoint_mut(name, endpoint);
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
    /// Public API scope.
    pub public_scope: ApiScope,
    /// Private API scope.
    pub private_scope: ApiScope,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
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

/// Aggregator of `ApiBuilder`s. Each builder is associated with a mount point, which
/// is used to separate endpoints for different builders.
#[derive(Debug, Clone, Default)]
pub struct ApiAggregator {
    endpoints: BTreeMap<String, ApiBuilder>,
}

impl ApiAggregator {
    /// Creates an empty API aggregator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a handler for a set of endpoints with the given mount point.
    pub fn insert(&mut self, name: &str, api: ApiBuilder) {
        self.endpoints.insert(name.to_owned(), api);
    }

    /// Extends the list of endpoint handlers with the new specified handlers.
    pub fn extend(&mut self, endpoints: impl IntoIterator<Item = (String, ApiBuilder)>) {
        self.endpoints.extend(endpoints);
    }

    /// Extends the API backend with the handlers with the given access level.
    #[doc(hidden)] // used by testkit; logically not public
    pub fn extend_backend<B: ExtendApiBackend>(&self, access: ApiAccess, backend: B) -> B {
        let endpoints = self.endpoints.iter();
        match access {
            ApiAccess::Public => backend
                .extend(endpoints.map(|(name, builder)| (name.as_str(), &builder.public_scope))),
            ApiAccess::Private => backend
                .extend(endpoints.map(|(name, builder)| (name.as_str(), &builder.private_scope))),
        }
    }
}
