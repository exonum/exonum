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

<<<<<<< HEAD
//! `RESTful` API and corresponding utilities. This module does not describe
//! the API itself, but rather the entities which the service author needs to
//! add private and public API endpoints to the service.
//!
//! The REST architectural style encompasses a list of constrains and properties
//! based on HTTP. The most common operations available are GET, POST, PUT and
//! DELETE. The requests are placed to a URL which represents a resource. The
//! requests either retrieve information or define a change that is to be made.
=======
//! API and corresponding utilities.
>>>>>>> master

pub use self::error::Error;
pub use self::state::ServiceApiState;
pub use self::with::{FutureResult, Immutable, Mutable, NamedWith, Result, With};

use serde::{de::DeserializeOwned, Serialize};

use std::{collections::BTreeMap, fmt};

use self::backends::actix;
use blockchain::{Blockchain, SharedNodeState};

pub mod backends;
pub mod error;
pub mod node;
mod state;
pub(crate) mod websocket;
mod with;

<<<<<<< HEAD
/// List of possible API errors, which can be returned when processing an API
/// request.
#[derive(Fail, Debug)]
pub enum ApiError {
    /// Storage error. This error is returned, for example, if the requested data
    /// do not exist in the database; or if the requested data exists in the
    /// database but additional permissions are required to access it.
    #[fail(display = "Storage error: {}", _0)]
    Storage(#[cause] storage::Error),

    /// Input/output error.
    #[fail(display = "IO error: {}", _0)]
    Io(#[cause] ::std::io::Error),

    /// Bad request. This error is returned when the submitted request contains an
    /// invalid parameter.
    #[fail(display = "Bad request: {}", _0)]
    BadRequest(String),

    /// Not found. This error is returned when the path in the URL of the request
    /// is incorrect.
    #[fail(display = "Not found: {}", _0)]
    NotFound(String),

    /// Internal error. This this type of error can be defined
    #[fail(display = "Internal server error: {}", _0)]
    InternalError(Box<::std::error::Error + Send + Sync>),

    /// Unauthorized error. This error is returned when a user is not authorized
    /// in the system and does not have permissions to perform the API request.
    #[fail(display = "Unauthorized")]
    Unauthorized,
}

impl From<io::Error> for ApiError {
    fn from(e: io::Error) -> ApiError {
        ApiError::Io(e)
=======
/// Defines object that could be used as an API backend.
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
        E: Into<With<Q, I, R, F>>,
        Self::Handler: From<NamedWith<Q, I, R, F, Immutable>>,
    {
        let named_with = NamedWith::new(name, endpoint);
        self.raw_handler(Self::Handler::from(named_with))
>>>>>>> master
    }

    /// Adds the given mutable endpoint handler to the backend.
    fn endpoint_mut<N, Q, I, R, F, E>(&mut self, name: N, endpoint: E) -> &mut Self
    where
        N: Into<String>,
        Q: DeserializeOwned + 'static,
        I: Serialize + 'static,
        F: for<'r> Fn(&'r ServiceApiState, Q) -> R + 'static + Clone,
        E: Into<With<Q, I, R, F>>,
        Self::Handler: From<NamedWith<Q, I, R, F, Mutable>>,
    {
        let named_with = NamedWith::new(name, endpoint);
        self.raw_handler(Self::Handler::from(named_with))
    }

    /// Adds the raw endpoint handler for the given backend.
    fn raw_handler(&mut self, handler: Self::Handler) -> &mut Self;

    /// Binds API handlers to the given backend.
    fn wire(&self, output: Self::Backend) -> Self::Backend;
}

/// Service API builder for the concrete API scope or in other words
/// access level (public or private).
#[derive(Debug, Clone, Default)]
pub struct ServiceApiScope {
    pub(crate) actix_backend: actix::ApiBuilder,
}

impl ServiceApiScope {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds the given endpoint handler to the API scope.
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
        F: for<'r> Fn(&'r ServiceApiState, Q) -> R + 'static + Clone,
        E: Into<With<Q, I, R, F>>,
        actix::RequestHandler: From<NamedWith<Q, I, R, F, Immutable>>,
    {
        self.actix_backend.endpoint(name, endpoint);
        self
    }

    /// Adds the given mutable endpoint handler to the API scope.
    ///
    /// For now there is only web backend and it has the following requirements:
    ///
    /// - Query parameters should be decodable via `serde_json`.
    /// - Response items also should be encodable via `serde_json` crate.
    pub fn endpoint_mut<Q, I, R, F, E>(&mut self, name: &'static str, endpoint: E) -> &mut Self
    where
        Q: DeserializeOwned + 'static,
        I: Serialize + 'static,
        F: for<'r> Fn(&'r ServiceApiState, Q) -> R + 'static + Clone,
        E: Into<With<Q, I, R, F>>,
        actix::RequestHandler: From<NamedWith<Q, I, R, F, Mutable>>,
    {
        self.actix_backend.endpoint_mut(name, endpoint);
        self
    }

    /// Returns a mutable reference to the underlying web backend.
    pub fn web_backend(&mut self) -> &mut actix::ApiBuilder {
        &mut self.actix_backend
    }
}

/// Service API builder, which is used to add service-specific endpoints to the node API.
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
/// use exonum::crypto::Hash;
///
/// // Declares a type which describes an API specification and implementation.
/// pub struct MyApi;
///
/// // Declares structures for requests and responses.
///
/// // For the web backend `MyQuery` will be deserialized from the `block_height={number}` string.
/// #[derive(Deserialize, Clone, Copy)]
/// pub struct MyQuery {
///     pub block_height: u64
/// }
///
/// // For the web backend `BlockInfo` will be serialized into the JSON string.
/// #[derive(Serialize, Clone, Copy)]
/// pub struct BlockInfo {
///     pub hash: Hash,
/// }
///
/// // Creates API handlers.
/// impl MyApi {
///     // Immutable handler, which returns a hash of the block at the given height.
///     pub fn block_hash(state: &ServiceApiState, query: MyQuery) -> api::Result<Option<BlockInfo>> {
///         let schema = Schema::new(state.snapshot());
///         Ok(schema.block_hashes_by_height()
///             .get(query.block_height)
///             .map(|hash| BlockInfo { hash })
///         )
///     }
///
///     // Mutable handler which removes peer with the given address from the cache.
///     pub fn remove_peer(state: &ServiceApiState, query: SocketAddr) -> api::Result<()> {
///         let mut blockchain = state.blockchain().clone();
///         Ok(blockchain.remove_peer_with_addr(&query))
///     }
///
///     // Simple handler without any params.
///     pub fn ping(_state: &ServiceApiState, _query: ()) -> api::Result<()> {
///         Ok(())
///     }
///
///     // You may also creates an asynchronous handlers for the long requests.
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
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a mutable reference to the public API scope builder.
    pub fn public_scope(&mut self) -> &mut ServiceApiScope {
        &mut self.public_scope
    }

    /// Returns a mutable reference to the private API scope builder.
    pub fn private_scope(&mut self) -> &mut ServiceApiScope {
        &mut self.private_scope
    }
}

<<<<<<< HEAD
/// `Api` trait defines `RESTful` API.
pub trait Api {
    /// Deserializes a URL fragment as `T`.
    fn url_fragment<T>(&self, request: &Request, name: &str) -> Result<T, ApiError>
    where
        T: FromStr,
        T::Err: fmt::Display,
    {
        let params = request.extensions.get::<Router>().unwrap();
        let fragment = params.find(name).ok_or_else(|| {
            ApiError::BadRequest(format!("Required parameter '{}' is missing", name))
        })?;
        let value = T::from_str(fragment)
            .map_err(|e| ApiError::BadRequest(format!("Invalid '{}' parameter: {}", name, e)))?;
        Ok(value)
    }

    /// Deserializes an optional parameter from a request body or `GET` parameters.
    fn optional_param<T>(&self, request: &mut Request, name: &str) -> Result<Option<T>, ApiError>
    where
        T: FromStr,
        T::Err: fmt::Display,
    {
        let map = request.get_ref::<params::Params>().unwrap();
        let value = match map.find(&[name]) {
            Some(&params::Value::String(ref param)) => {
                let value = T::from_str(param).map_err(|e| {
                    ApiError::BadRequest(format!("Invalid '{}' parameter: {}", name, e))
                })?;
                Some(value)
            }
            _ => None,
        };
        Ok(value)
=======
/// Exonum API access level.
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
>>>>>>> master
    }
}

<<<<<<< HEAD
    /// Deserializes a required parameter from a request body or `GET` parameters.
    fn required_param<T>(&self, request: &mut Request, name: &str) -> Result<T, ApiError>
=======
/// API backend extender.
pub trait ExtendApiBackend {
    /// Extends API backend by the given scopes.
    fn extend<'a, I>(self, items: I) -> Self
>>>>>>> master
    where
        I: IntoIterator<Item = (&'a str, &'a ServiceApiScope)>;
}

<<<<<<< HEAD
    /// Deserializes a request body as a structure of type `T`.
    fn parse_body<T: 'static>(&self, req: &mut Request) -> Result<T, ApiError>
    where
        T: Clone + for<'de> Deserialize<'de>,
    {
        match req.get::<bodyparser::Struct<T>>() {
            Ok(Some(param)) => Ok(param),
            Ok(None) => Err(ApiError::BadRequest("Body is empty".into())),
            Err(e) => Err(ApiError::BadRequest(format!("Invalid struct: {}", e))),
        }
    }

    /// Loads a hex value from the cookies.
    fn load_hex_value_from_cookie<'a>(
        &self,
        request: &'a Request,
        key: &str,
    ) -> storage::Result<Vec<u8>> {
        if let Some(&Cookie(ref cookies)) = request.headers.get() {
            for cookie in cookies.iter() {
                if let Ok(c) = CookiePair::parse(cookie.as_str()) {
                    if c.name() == key {
                        if let Ok(value) = FromHex::from_hex(c.value()) {
                            return Ok(value);
                        }
                    }
                }
            }
=======
/// Exonum node API aggregator. Currently only HTTP v1 backend is available.
#[derive(Debug, Clone)]
pub struct ApiAggregator {
    blockchain: Blockchain,
    node_state: SharedNodeState,
    inner: BTreeMap<String, ServiceApiBuilder>,
}

impl ApiAggregator {
    /// Aggregates API for the given blockchain and node state.
    pub fn new(blockchain: Blockchain, node_state: SharedNodeState) -> Self {
        let mut inner = BTreeMap::new();
        // Adds built-in APIs.
        inner.insert(
            "system".to_owned(),
            Self::system_api(&blockchain, node_state.clone()),
        );
        inner.insert(
            "explorer".to_owned(),
            Self::explorer_api(node_state.clone()),
        );
        // Adds services APIs.
        inner.extend(blockchain.service_map().iter().map(|(_, service)| {
            let mut builder = ServiceApiBuilder::new();
            service.wire_api(&mut builder);
            // TODO think about prefixes for non web backends. (ECR-1758)
            let prefix = format!("services/{}", service.service_name());
            (prefix, builder)
        }));

        Self {
            inner,
            blockchain,
            node_state,
>>>>>>> master
        }
    }

    /// Returns a reference to the blockchain used by the aggregator.
    pub fn blockchain(&self) -> &Blockchain {
        &self.blockchain
    }

<<<<<<< HEAD
    //TODO: Remove duplicate code
    /// Returns NotFound and a certain with cookies.
    fn not_found_response_with_cookies(
        &self,
        json: &serde_json::Value,
        cookies: Option<Vec<String>>,
    ) -> IronResult<Response> {
        let mut resp = Response::with((
            status::NotFound,
            serde_json::to_string_pretty(json).unwrap(),
        ));
        resp.headers.set(ContentType::json());
        if let Some(cookies) = cookies {
            resp.headers.set(SetCookie(cookies));
=======
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
>>>>>>> master
        }
    }

<<<<<<< HEAD
    /// Returns OK and a certain response with cookies.
    fn ok_response_with_cookies(
        &self,
        json: &serde_json::Value,
        cookies: Option<Vec<String>>,
    ) -> IronResult<Response> {
        let mut resp = Response::with((status::Ok, serde_json::to_string_pretty(json).unwrap()));
        resp.headers.set(ContentType::json());
        if let Some(cookies) = cookies {
            resp.headers.set(SetCookie(cookies));
        }
        Ok(resp)
    }

    /// Returns OK and a certain response.
    fn ok_response(&self, json: &serde_json::Value) -> IronResult<Response> {
        self.ok_response_with_cookies(json, None)
    }
    /// Returns NotFound and a certain response.
    fn not_found_response(&self, json: &serde_json::Value) -> IronResult<Response> {
        self.not_found_response_with_cookies(json, None)
    }

    /// Defines the URL through which certain internal methods can be applied.
    fn wire<'b>(&self, router: &'b mut Router);
=======
    /// Adds API factory with given prefix to the aggregator.
    pub fn insert<S: Into<String>>(&mut self, prefix: S, builder: ServiceApiBuilder) {
        self.inner.insert(prefix.into(), builder);
    }

    fn explorer_api(shared_node_state: SharedNodeState) -> ServiceApiBuilder {
        let mut builder = ServiceApiBuilder::new();
        self::node::public::ExplorerApi::wire(builder.public_scope(), shared_node_state);
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
>>>>>>> master
}
