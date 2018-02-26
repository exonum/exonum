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

//! Transport-agnostic API for Exonum services.
//!
//! # Examples
//!
//! ```
//! # #[macro_use] extern crate exonum;
//! use exonum::api::ext::{ApiBuilder, ApiResult, Endpoint};
//! use exonum::api::iron;
//! # use exonum::blockchain::{ApiContext, Blockchain, ExecutionResult, Service, Transaction};
//! # use exonum::crypto::Hash;
//! # use exonum::encoding;
//! # use exonum::messages::RawTransaction;
//! # use exonum::storage::{Fork, Snapshot};
//!
//! // Transactions
//! transactions! {
//!     Any {
//!         const SERVICE_ID = 1000;
//!
//!         struct Foo {
//!             foo: u64,
//!         }
//!
//!         struct Bar {
//!             bar: &str,
//!             baz: i8,
//!         }
//!     }
//! }
//!
//! impl Transaction for Foo {
//!     // ...
//! #   fn verify(&self) -> bool { true }
//! #   fn execute(&self, _: &mut Fork) -> ExecutionResult { Ok(()) }
//! }
//!
//! impl Transaction for Bar {
//!     // ...
//! #   fn verify(&self) -> bool { true }
//! #   fn execute(&self, _: &mut Fork) -> ExecutionResult { Ok(()) }
//! }
//!
//! // Read requests
//! read_request! {
//!     @(ID = "read")
//!     pub Read(()) -> u64;
//! }
//!
//! impl Endpoint for Read {
//!     fn handle(&self, _: ()) -> ApiResult<u64> { Ok(42) }
//! }
//!
//! // In `Service` implementation:
//! # struct MyService;
//! impl Service for MyService {
//!     // ...
//! #   fn service_id(&self) -> u16 { 1000 }
//! #   fn service_name(&self) -> &str { "MyService" }
//! #   fn state_hash(&self, _: &Snapshot) -> Vec<Hash> { vec![] }
//! #   fn tx_from_raw(&self, _: RawTransaction) -> Result<Box<Transaction>, encoding::Error> {
//! #       unimplemented!()
//! #   }
//!
//!     fn public_api_handler(&self, context: &ApiContext) -> Option<Box<iron::Handler>> {
//!         let api = ApiBuilder::new(&context)
//!             .add::<Read>()
//!             .add_transactions::<Any>()
//!             .create();
//!         Some(iron::into_handler(api))
//!     }
//! }
//! # fn main() { }
//! ```

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{self, Value};

use std::collections::HashMap;
use std::{fmt, io};

use blockchain::{ApiContext, Transaction};
use crypto::Hash;
use node::{ApiSender, TransactionSend};

/// The identifier for the "standard" transaction sink.
///
/// Transaction sinks can be added to service APIs using the [`add_transactions`]
/// method in `ApiBuilder`; see it for more details
///
/// [`add_transactions`]: struct.ApiBuilder.html#method.add_transactions
pub const TRANSACTIONS_ID: &str = "transactions";

/// API-related errors.
#[derive(Debug, Fail)]
pub enum ApiError {
    /// Call to endpoint with unknown identifier.
    #[fail(display = "Unknown endpoint ID: {}", _0)]
    UnknownId(String),

    // TODO: split `serde::Error` / others?
    /// A request is malformed or otherwise cannot be processed.
    #[fail(display = "Bad request: {}", _0)]
    BadRequest(Box<::std::error::Error + Send + Sync>),

    /// Requested resource is not found.
    #[fail(display = "Not found")]
    NotFound,

    // TODO: split `VerificationFail` / others?
    /// A transaction processed by API has failed to be sent.
    #[fail(display = "Failed to send transaction: {}", _0)]
    TransactionNotSent(
        #[cause]
        io::Error
    ),

    /// Generic server-side error.
    #[fail(display = "Internal server error: {}", _0)]
    InternalError(Box<::std::error::Error + Send + Sync>),
}

/// Alias for the result type used within the `api` module.
pub type ApiResult<T> = Result<T, ApiError>;

/// Specification for service endpoints.
pub trait EndpointSpec {
    /// Request type accepted by the endpoint.
    type Request: Serialize + DeserializeOwned;
    /// Response type accepted by the endpoint.
    type Response: Serialize + DeserializeOwned;

    /// Retrieval style for the endpoint.
    const METHOD: Method;
    /// Endpoint identifier. Must be unique within the service.
    const ID: &'static str;
}

/// Service endpoint.
///
/// This is the main trait intended to be used by service developers.
/// Due to reliance on [`EndpointSpec`], the information about the endpoint
/// will be displayed in crate docs in the easily digestible form.
///
/// Note that [`read_request!`] macro provides an even more convenient way for
/// implementing read requests.
///
/// [`EndpointSpec`]: trait.EndpointSpec.html
/// [`read_request!`]: ../../macro.read_request.html
///
/// # Examples
///
/// A custom transaction sender creating and signing transactions signed with
/// the service secret key.
///
/// ```
/// # #[macro_use] extern crate exonum;
/// # use exonum::api::ext::{ApiError, Endpoint, EndpointSpec, FromContext, Method};
/// # use exonum::blockchain::{ApiContext, ExecutionResult, Transaction};
/// # use exonum::crypto::{CryptoHash, Hash, SecretKey};
/// # use exonum::node::{ApiSender, TransactionSend};
/// # use exonum::storage::Fork;
/// // Suppose we have this transaction spec.
/// transactions! {
///     Any {
///         const SERVICE_ID = // ...
/// #                          1000;
///
///         struct MyTransaction {
///             foo: u64,
///             bar: &str,
///         }
///         // Other transactions...
///     }
/// }
/// # impl Transaction for MyTransaction {
/// #     fn verify(&self) -> bool { true }
/// #     fn execute(&self, _: &mut Fork) -> ExecutionResult { Ok(()) }
/// # }
///
/// // Sender for `MyTransaction`s.
/// pub struct SendTransaction {
///     channel: ApiSender,
///     secret_key: SecretKey,
/// }
///
/// impl FromContext for SendTransaction {
///     fn from_context(context: &ApiContext) -> Self {
///         SendTransaction {
///             channel: context.node_channel().clone(),
///             secret_key: context.secret_key().clone(),
///         }
///     }
/// }
///
/// impl EndpointSpec for SendTransaction {
///     type Request = (u64, String);
///     type Response = Hash;
///     const METHOD: Method = Method::Post;
///     const ID: &'static str = "send-transaction";
/// }
///
/// impl Endpoint for SendTransaction {
///     fn handle(&self, req: (u64, String)) -> Result<Hash, ApiError> {
///         let tx = MyTransaction::new(req.0, &req.1, &self.secret_key);
///         let tx_hash = tx.hash();
///         self.channel.send(tx.into()).map_err(ApiError::TransactionNotSent)?;
///         Ok(tx_hash)
///     }
/// }
/// # fn main() { }
/// ```
pub trait Endpoint: EndpointSpec + Send + Sync {
    /// Handles a request to the endpoint.
    ///
    /// # Important note
    ///
    /// Unlike with transaction handling, the core does not catch panics during
    /// the execution of `handle()`. Thus, any panic will lead to stopping
    /// the entire request processing thread.
    fn handle(&self, request: Self::Request) -> ApiResult<Self::Response>;
}

/// Internally used version of `Endpoint`.
///
/// The type rarely is needed to be used directly; the preferable way
/// of implementing endpoints is to implement the [`Endpoint`] trait.
///
/// [`Endpoint`]: trait.Endpoint.html
pub struct BoxedEndpoint {
    method: Method,
    id: String,
    handler: Box<Fn(Value) -> ApiResult<Value> + Send + Sync>,
}

impl fmt::Debug for BoxedEndpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        formatter
            .debug_struct("BoxedEndpoint")
            .field("method", &self.method)
            .field("id", &self.id)
            .finish()
    }
}

impl BoxedEndpoint {
    /// Returns the retrieval style of this endpoint.
    pub fn method(&self) -> Method {
        self.method
    }

    /// Returns the identifier of this endpoint.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Handles a request.
    pub fn handle(&self, request: Value) -> ApiResult<Value> {
        (self.handler)(request)
    }
}

/// Builder for `BoxedEndpoint`s, allowing to achieve more fine-grained control over endpoint
/// creation.
///
/// Consider using [`Endpoint`] for more user-friendly interface.
///
/// [`Endpoint`]: trait.Endpoint.html
///
/// # Examples
///
/// ```
/// # extern crate exonum;
/// # extern crate futures;
/// #[macro_use] extern crate serde_json;
/// # use exonum::api::ext::{EndpointBuilder, Method};
/// # use exonum::blockchain::{Blockchain, ExecutionResult, Transaction};
/// # use exonum::crypto::{self, PublicKey};
/// # use exonum::node::ApiSender;
/// # use exonum::storage::{Fork, MemoryDB, Snapshot};
/// # use futures::sync::mpsc;
/// use serde_json::Value;
///
/// // Service schema containing balances.
/// struct Schema { /* ... */ }
///
/// impl Schema {
/// #  fn new<S: AsRef<Snapshot>>(snapshot: S) -> Self { Schema { } }
///    pub fn balance(&self, key: &PublicKey) -> Option<u64> {
///       // ...
/// #     Some(42)
///   }
/// }
///
/// # fn main() {
/// # let (pubkey, key) = crypto::gen_keypair();
/// # let api_channel = mpsc::channel(4);
/// let alice_key: PublicKey = // ...
/// #   PublicKey::new([0; 32]);
/// let blockchain = // ...
/// #   Blockchain::new(
/// #       Box::new(MemoryDB::new()),
/// #       vec![],
/// #       pubkey,
/// #       key,
/// #       ApiSender::new(api_channel.0.clone()),
/// #   );
/// let endpoint = EndpointBuilder::read_request("wallet")
///     .handler(move |req: Value| {
///         let pubkey: PublicKey = serde_json::from_value(req)
///             .unwrap_or(alice_key);
///         let balance: Option<u64> = Schema::new(blockchain.snapshot())
///             .balance(&pubkey);
///         Ok(json!(balance))
///     })
///     .create();
///
/// assert_eq!(endpoint.id(), "wallet");
/// assert_eq!(endpoint.method(), Method::Get);
/// assert_eq!(endpoint.handle(json!("garbage")).unwrap(), json!(42));
/// # }
/// ```
pub struct EndpointBuilder {
    method: Method,
    id: String,
    handler: Option<Box<Fn(Value) -> ApiResult<Value> + Send + Sync>>,
}

impl EndpointBuilder {
    /// Sets up the builder for a read request.
    pub fn read_request<S: AsRef<str>>(id: S) -> Self {
        EndpointBuilder {
            method: Method::Get,
            id: id.as_ref().to_owned(),
            handler: None,
        }
    }

    /// Sets the endpoint handler.
    pub fn handler<F>(mut self, handler: F) -> Self
    where
        F: 'static + Fn(Value) -> ApiResult<Value> + Send + Sync,
    {
        self.handler = Some(Box::new(handler));
        self
    }

    /// Creates the endpoint.
    pub fn create(self) -> BoxedEndpoint {
        BoxedEndpoint {
            method: self.method,
            id: self.id,
            handler: self.handler.expect("Endpoint handler not set"),
        }
    }
}

impl fmt::Debug for EndpointBuilder {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        formatter
            .debug_struct("EndpointBuilder")
            .field("method", &self.method)
            .field("id", &self.id)
            .field("handler", &self.handler.as_ref().map(|_| ".."))
            .finish()
    }
}

impl<T: Endpoint + 'static> From<T> for BoxedEndpoint {
    fn from(endpoint: T) -> Self {
        BoxedEndpoint {
            method: T::METHOD,
            id: T::ID.to_owned(),
            handler: Box::new(move |req: Value| {
                let request: T::Request = serde_json::from_value(req).map_err(|e| {
                    ApiError::BadRequest(e.into())
                })?;
                let response = endpoint.handle(request)?;
                let response = serde_json::to_value(response).map_err(|e| {
                    ApiError::InternalError(e.into())
                })?;
                Ok(response)
            }),
        }
    }
}

/// Type that can be instantiated from an `ApiContext` reference.
///
/// # Notes
///
/// This trait is used by [`ApiBuilder`]; it allows to add [`Endpoint`]s
/// just by mentioning a type.
///
/// [`ApiBuilder`]: struct.ApiBuilder.html
/// [`Endpoint`]: trait.Endpoint.html
pub trait FromContext {
    /// Creates an instance from the API context.
    fn from_context(context: &ApiContext) -> Self;
}

/// Supported subset of HTTP methods.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Method {
    /// Marker for a safe endpoint, i.e., an endpoint without noticeable side effects.
    ///
    /// Named by analogy with HTTP `GET` method.
    Get,

    /// Marker for an unsafe endpoint, which is mainly called for side effects
    /// (e.g., pushing a transaction into the memory pool).
    ///
    /// Named by analogy with HTTP `POST` method.
    Post,
}

/// Definition of an `Endpoint` used to read information from the blockchain.
///
/// The macro implements all that is necessary for working with the endpoint
/// with the exception of the [`Endpoint`] trait itself.
///
/// [`Endpoint`]: api/ext/trait.Endpoint.html
///
/// # Examples
///
/// ```
/// # #[macro_use] extern crate exonum;
/// use exonum::api::ext::{ApiResult, Endpoint};
/// # use exonum::crypto::PublicKey;
/// # use exonum::storage::Snapshot;
///
/// read_request! {
///     /// Gets the balance of a particular wallet.
///     @(ID = "balance")
///     pub GetBalance(PublicKey) -> Option<u64>;
/// }
///
/// // Service schema containing balances.
/// struct Schema { /* ... */ }
///
/// impl Schema {
/// #  fn new<S: AsRef<Snapshot>>(snapshot: S) -> Self { Schema { } }
///    pub fn balance(&self, key: &PublicKey) -> Option<u64> {
///       // ...
/// #     Some(42)
///   }
/// }
///
/// impl Endpoint for GetBalance {
///     fn handle(&self, key: PublicKey) -> ApiResult<Option<u64>> {
///         let schema = Schema::new(self.as_ref().snapshot());
///         Ok(schema.balance(&key))
///     }
/// }
/// # fn main() {}
/// ```
#[macro_export]
macro_rules! read_request {
    // No visibility specifier
    (
        $(#[$attr:meta])*
        @(ID = $id:expr) $name:ident($req:ty) -> $resp:ty;
    ) => {
        $(#[$attr])*
        #[derive(Debug, Clone)]
        pub struct $name($crate::blockchain::Blockchain);

        read_request!(@implement ($id), $name, $req, $resp);
    };

    // `pub` visibility specifier
    (
        $(#[$attr:meta])*
        @(ID = $id:expr) pub $name:ident($req:ty) -> $resp:ty;
    ) => {
        $(#[$attr])*
        #[derive(Debug, Clone)]
        pub struct $name($crate::blockchain::Blockchain);

        read_request!(@implement ($id), $name, $req, $resp);
    };

    // `pub(..)` visibility specifier
    // XXX: `pub(in ..)` processing is essentially a hack
    (
        $(#[$attr:meta])*
        @(ID = $id:expr) pub($(in)* $vis:path) $name:ident($req:ty) -> $resp:ty;
    ) => {
        $(#[$attr])*
        #[derive(Debug, Clone)]
        pub(in $vis) struct $name($crate::blockchain::Blockchain);

        read_request!(@implement ($id), $name, $req, $resp);
    };

    (@implement ($id:expr), $name:ident, $req:ty, $resp:ty) => {
        impl $crate::api::ext::FromContext for $name {
            fn from_context(context: &$crate::blockchain::ApiContext) -> Self {
                $name(context.blockchain().clone())
            }
        }

        impl $crate::api::ext::EndpointSpec for $name {
            type Request = $req;
            type Response = $resp;

            const METHOD: $crate::api::ext::Method = $crate::api::ext::Method::Get;
            const ID: &'static str = $id;
        }

        impl AsRef<$crate::blockchain::Blockchain> for $name {
            fn as_ref(&self) -> &$crate::blockchain::Blockchain {
                &self.0
            }
        }
    }
}

/// The response returned by `TransactionSink`s.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionResponse {
    /// Hash of the transaction.
    pub tx_hash: Hash,
}

#[derive(Debug)]
struct TransactionSink<T> {
    channel: ApiSender,
    _marker: ::std::marker::PhantomData<T>,
}

impl<T> EndpointSpec for TransactionSink<T>
where
    T: Into<Box<Transaction>>
        + Serialize
        + DeserializeOwned
        + Send
        + Sync,
{
    type Request = T;
    type Response = TransactionResponse;

    const METHOD: Method = Method::Post;
    const ID: &'static str = TRANSACTIONS_ID;
}

impl<T> Endpoint for TransactionSink<T>
where
    T: Into<Box<Transaction>>
        + Serialize
        + DeserializeOwned
        + Send
        + Sync,
{
    fn handle(&self, transaction: T) -> ApiResult<TransactionResponse> {
        let transaction: Box<Transaction> = transaction.into();
        let tx_hash = transaction.hash();
        self.channel.send(transaction).map_err(
            ApiError::TransactionNotSent,
        )?;
        Ok(TransactionResponse { tx_hash })
    }
}

impl<T> FromContext for TransactionSink<T> {
    fn from_context(context: &ApiContext) -> Self {
        TransactionSink {
            channel: context.node_channel().clone(),
            _marker: ::std::marker::PhantomData,
        }
    }
}

/// Builder for service APIs.
#[derive(Debug)]
pub struct ApiBuilder<'a> {
    context: &'a ApiContext,
    endpoints: HashMap<String, BoxedEndpoint>,
}

impl<'a> ApiBuilder<'a> {
    /// Creates a builder.
    pub fn new(context: &'a ApiContext) -> Self {
        ApiBuilder {
            context,
            endpoints: HashMap::new(),
        }
    }

    /// Adds an instantiated endpoint.
    ///
    /// # Panics
    ///
    /// Panics if the builder already contains an endpoint with the same identifier.
    pub fn add_endpoint<T>(mut self, endpoint: T) -> Self
    where
        T: Into<BoxedEndpoint>,
    {
        let endpoint = endpoint.into();
        let endpoint_id = endpoint.id().to_string();
        let old = self.endpoints.insert(endpoint_id.clone(), endpoint);
        assert!(old.is_none(), "Duplicate endpoint ID: {}", endpoint_id);
        self
    }

    /// Adds an endpoint by its type `T`.
    ///
    /// Primarily intended to be used with [`Endpoint`]s, for example,
    /// read requests defined with the [`read_request!`] macro.
    ///
    /// # Panics
    ///
    /// Panics if the builder already contains an endpoint with the same identifier.
    ///
    /// [`Endpoint`]: trait.Endpoint.html
    /// [`read_request!`]: ../../macro.read_request.html
    pub fn add<T>(self) -> Self
    where
        T: FromContext + Into<BoxedEndpoint>,
    {
        let endpoint = T::from_context(self.context);
        self.add_endpoint(endpoint)
    }

    /// Add a sink for transactions.
    ///
    /// # Notes
    ///
    /// Type `T` can be a separate [`Transaction`] or a [`TransactionSet`].
    ///
    /// # Panics
    ///
    /// Panics if the builder already contains an endpoint with the same identifier.
    ///
    /// [`Transaction`]: ../../blockchain/trait.Transaction.html
    /// [`TransactionSet`]: ../../blockchain/trait.TransactionSet.html
    pub fn add_transactions<T>(self) -> Self
    where
        T: 'static + Into<Box<Transaction>> + Serialize + DeserializeOwned + Send + Sync,
    {
        let endpoint = TransactionSink::<T>::from_context(self.context);
        self.add_endpoint(endpoint)
    }

    /// Creates the service API.
    pub fn create(self) -> ServiceApi {
        ServiceApi { endpoints: self.endpoints }
    }
}

/// Collection of named endpoints.
pub trait EndpointHolder {
    /// Tries to retrieve a reference to an endpoint with the specified identifier.
    fn endpoint(&self, id: &str) -> Option<&BoxedEndpoint>;
}

/// Full collection of endpoints for a particular service.
///
/// Use [`ApiBuilder`] to build instances.
///
/// [`ApiBuilder`]: struct.ApiBuilder.html
#[derive(Debug)]
pub struct ServiceApi {
    endpoints: HashMap<String, BoxedEndpoint>,
}

impl ServiceApi {
    /// Introduces a filter for this API.
    pub fn filter<F>(&self, predicate: F) -> Filter<F>
    where
        F: Fn(&BoxedEndpoint) -> bool,
    {
        Filter {
            base: self,
            predicate,
        }
    }
}

impl EndpointHolder for ServiceApi {
    fn endpoint(&self, id: &str) -> Option<&BoxedEndpoint> {
        self.endpoints.get(id)
    }
}

/// Lazily filtered collection of endpoints.
#[derive(Debug)]
pub struct Filter<'a, F> {
    base: &'a ServiceApi,
    predicate: F,
}

impl<'a, F> EndpointHolder for Filter<'a, F>
where
    F: Fn(&BoxedEndpoint) -> bool,
{
    fn endpoint(&self, id: &str) -> Option<&BoxedEndpoint> {
        let endpoint = self.base.endpoint(id)?;

        if (self.predicate)(endpoint) {
            Some(endpoint)
        } else {
            None
        }
    }
}

impl<'a> ::std::ops::Index<&'a str> for ServiceApi {
    type Output = BoxedEndpoint;

    fn index(&self, idx: &'a str) -> &BoxedEndpoint {
        self.endpoint(idx).expect(
            &format!("Unknown endpoint ID: {}", idx),
        )
    }
}

impl<'a, 's, F> ::std::ops::Index<&'s str> for Filter<'a, F>
where
    F: Fn(&BoxedEndpoint) -> bool,
{
    type Output = BoxedEndpoint;

    fn index(&self, idx: &'s str) -> &BoxedEndpoint {
        self.endpoint(idx).expect(
            &format!("Unknown endpoint ID: {}", idx),
        )
    }
}
