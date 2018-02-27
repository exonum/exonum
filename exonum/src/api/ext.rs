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
//! use exonum::api::ext::{ApiResult, EndpointContext, EndpointSpec, ReadRequest, ServiceApi};
//! use exonum::api::iron::{self, IronAdapter};
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
//! struct Read;
//!
//! impl EndpointSpec for Read {
//!     type Request = ();
//!     type Response = u64;
//!     const ID: &'static str = "read";
//! }
//!
//! impl ReadRequest for Read {
//!     fn handle(&self, _: &EndpointContext, _: ()) -> ApiResult<u64> {
//!         Ok(42)
//!     }
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
//!         let mut api = ServiceApi::new();
//!         api.insert_read(Read);
//!         api.set_transactions::<Any>();
//!         Some(IronAdapter::new(context.clone()).create_handler(api))
//!     }
//! }
//! # fn main() { }
//! ```

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{self, Value};

use std::collections::HashMap;
use std::{fmt, io};

use blockchain::{ApiContext, ApiSender, Blockchain, SendError, Transaction};
use crypto::Hash;
use storage::Snapshot;

/// The identifier for the "standard" transaction sink.
///
/// Transaction sinks can be added to service APIs using the [`add_transactions`]
/// method in `ServiceApi`; see it for more details.
///
/// [`add_transactions`]: struct.ServiceApi.html#method.add_transactions
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

    /// Transaction verification has failed.
    #[fail(display = "Transaction not verified: {:?}", _0)]
    VerificationFail(Box<Transaction>),

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

impl From<SendError> for ApiError {
    fn from(err: SendError) -> ApiError {
        use self::SendError::*;

        match err {
            VerificationFail(tx) => ApiError::VerificationFail(tx),
            Io(e) => ApiError::TransactionNotSent(e),
        }
    }
}

/// Alias for the result type used within the `api` module.
pub type ApiResult<T> = Result<T, ApiError>;

/// Specification for service endpoints.
pub trait EndpointSpec {
    /// Request type accepted by the endpoint.
    type Request: Serialize + DeserializeOwned;
    /// Response type accepted by the endpoint.
    type Response: Serialize + DeserializeOwned;

    /// Endpoint identifier. Must be unique within the service.
    const ID: &'static str;
}

/// Context supplied to endpoints.
#[derive(Debug)]
pub struct EndpointContext<'a> {
    blockchain: &'a Blockchain,
    channel: &'a ApiSender,
}

impl<'a> EndpointContext<'a> {
    fn new(api_context: &'a ApiContext) -> Self {
        EndpointContext {
            blockchain: api_context.blockchain(),
            channel: api_context.node_channel(),
        }
    }

    /// Gets a snapshot of the current blockchain state.
    pub fn snapshot(&self) -> Box<Snapshot> {
        self.blockchain.snapshot()
    }

    /// Queues a transaction for sending over the network and including into the blockchain.
    ///
    /// The transaction should already be signed.
    pub fn send<T>(&mut self, transaction: T) -> Result<(), ApiError>
    where
        T: Into<Box<Transaction>>,
    {
        self.channel.send(transaction.into()).map_err(
            ApiError::from,
        )
    }

    /// Queues a transaction for signing, sending over the network and including
    /// into the blockchain.
    ///
    /// The transaction is signed with the service secret key and returned.
    pub fn sign_and_send<T>(&mut self, transaction: T) -> Result<Hash, ApiError>
    where
        T: Into<Box<Transaction>>,
    {
        self.channel.sign_and_send(transaction.into()).map_err(
            ApiError::from,
        )
    }
}

/// Endpoint used to read information from the blockchain.
///
/// This is the main trait intended to be used by service developers.
/// Due to reliance on [`EndpointSpec`], the information about the endpoint
/// will be displayed in crate docs in the easily digestible form.
///
/// [`EndpointSpec`]: trait.EndpointSpec.html
///
/// # Examples
///
/// ```
/// # #[macro_use] extern crate exonum;
/// use exonum::api::ext::{ApiResult, EndpointContext, EndpointSpec, ReadRequest};
/// # use exonum::crypto::PublicKey;
/// # use exonum::storage::Snapshot;
///
/// pub struct GetBalance;
///
/// impl EndpointSpec for GetBalance {
///     type Request = PublicKey;
///     type Response = Option<u64>;
///     const ID: &'static str = "balance";
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
/// impl ReadRequest for GetBalance {
///     fn handle(&self, ctx: &EndpointContext, key: PublicKey) -> ApiResult<Option<u64>> {
///         let schema = Schema::new(ctx.snapshot());
///         Ok(schema.balance(&key))
///     }
/// }
/// # fn main() {}
/// ```
pub trait ReadRequest: EndpointSpec + Send + Sync {
    /// Handles a request to the endpoint.
    ///
    /// # Important note
    ///
    /// Unlike with transaction handling, the core does not catch panics during
    /// the execution of `handle()`. Thus, any panic will lead to stopping
    /// the entire request processing thread.
    fn handle(
        &self,
        context: &EndpointContext,
        request: Self::Request,
    ) -> ApiResult<Self::Response>;
}

/// Endpoint that receives a mutable reference to the `EndpointContext`,
/// allowing it to perform more actions.
///
/// # Examples
///
/// A custom transaction sender creating and signing transactions signed with
/// the service secret key.
///
/// ```
/// # #[macro_use] extern crate exonum;
/// # use exonum::api::ext::{ApiError, Endpoint, EndpointContext, EndpointSpec};
/// # use exonum::blockchain::{ApiContext, ExecutionResult, Transaction};
/// # use exonum::crypto::{CryptoHash, Hash, Signature};
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
/// pub struct SendTransaction;
///
/// impl EndpointSpec for SendTransaction {
///     type Request = (u64, String);
///     type Response = Hash;
///     const ID: &'static str = "send-transaction";
/// }
///
/// impl Endpoint for SendTransaction {
///     fn handle(
///         &self,
///         context: &mut EndpointContext,
///         req: (u64, String),
///     ) -> Result<Hash, ApiError> {
///         let tx = MyTransaction::new_with_signature(req.0, &req.1, &Signature::zero());
///         Ok(context.sign_and_send(tx)?)
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
    fn handle(
        &self,
        context: &mut EndpointContext,
        request: Self::Request,
    ) -> ApiResult<Self::Response>;
}

/// Internally used version of endpoints.
///
/// The type rarely is needed to be used directly; the preferable way
/// of implementing endpoints is to implement the [`ReadRequest`] or [`Endpoint`] traits.
///
/// [`Endpoint`]: trait.Endpoint.html
///
/// # Examples
///
/// ```
/// # extern crate exonum;
/// #[macro_use] extern crate serde_json;
/// # use exonum::api::ext::BoxedEndpoint;
/// # use exonum::blockchain::{Blockchain, ExecutionResult, Transaction};
/// # use exonum::crypto::{self, PublicKey};
/// # use exonum::storage::Snapshot;
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
/// let alice_key: PublicKey = // ...
/// #   PublicKey::new([0; 32]);
/// let endpoint = BoxedEndpoint::read_request_fn(
///     "wallet",
///     move |context, req: serde_json::Value| {
///         let pubkey: PublicKey = serde_json::from_value(req)
///             .unwrap_or(alice_key);
///         let balance = Schema::new(context.snapshot())
///             .balance(&pubkey);
///         Ok(balance)
///     },
/// );
///
/// assert_eq!(endpoint.id(), "wallet");
/// assert!(endpoint.readonly());
/// # }
/// ```
pub struct BoxedEndpoint {
    id: String,
    handler: BoxedHandler,
}

impl fmt::Debug for BoxedEndpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        formatter
            .debug_struct("BoxedEndpoint")
            .field("readonly", &self.readonly())
            .field("id", &self.id)
            .finish()
    }
}

enum BoxedHandler {
    Mutating(Box<Fn(&mut EndpointContext, Value) -> ApiResult<Value> + Send + Sync>),
    Readonly(Box<Fn(&EndpointContext, Value) -> ApiResult<Value> + Send + Sync>),
}

impl BoxedHandler {
    fn invoke(&self, context: &mut EndpointContext, request: Value) -> ApiResult<Value> {
        use self::BoxedHandler::*;

        match *self {
            Mutating(ref handler) => (handler)(context, request),
            Readonly(ref handler) => (handler)(&*context, request),
        }
    }
}

impl BoxedEndpoint {
    /// Creates a read request from a given closure.
    pub fn read_request_fn<T, U, F>(id: &str, handler: F) -> Self
    where
        T: DeserializeOwned,
        U: Serialize,
        F: 'static + Fn(&EndpointContext, T) -> ApiResult<U> + Send + Sync,
    {
        BoxedEndpoint {
            id: id.to_owned(),
            handler: BoxedHandler::Readonly(Box::new(move |ctx, req| {
                BoxedEndpoint::wrap(req, |typed_req| handler(ctx, typed_req))
            })),
        }
    }

    /// Converts a read request into a boxed endpoint.
    pub fn read_request<T: 'static + ReadRequest>(read: T) -> Self {
        BoxedEndpoint::read_request_fn(T::ID, move |ctx, req| read.handle(ctx, req))
    }

    /// Creates a full-access endpoint from a given closure.
    pub fn endpoint_fn<T, U, F>(id: &str, handler: F) -> Self
    where
        T: DeserializeOwned,
        U: Serialize,
        F: 'static + Fn(&mut EndpointContext, T) -> ApiResult<U> + Send + Sync,
    {
        BoxedEndpoint {
            id: id.to_owned(),
            handler: BoxedHandler::Mutating(Box::new(move |ctx, req| {
                BoxedEndpoint::wrap(req, |typed_req| handler(ctx, typed_req))
            })),
        }
    }

    /// Converts a full-access endpoint into a boxed endpoint.
    pub fn endpoint<T: 'static + Endpoint>(endpoint: T) -> BoxedEndpoint {
        BoxedEndpoint::endpoint_fn(T::ID, move |ctx, req| endpoint.handle(ctx, req))
    }

    fn wrap<T, U, F>(req: Value, handler: F) -> ApiResult<Value>
    where
        T: DeserializeOwned,
        U: Serialize,
        F: FnOnce(T) -> ApiResult<U>,
    {
        let request: T = serde_json::from_value(req).map_err(|e| {
            ApiError::BadRequest(e.into())
        })?;
        let response = handler(request)?;
        let response = serde_json::to_value(response).map_err(|e| {
            ApiError::InternalError(e.into())
        })?;
        Ok(response)
    }

    /// Returns the retrieval style of this endpoint.
    pub fn readonly(&self) -> bool {
        match self.handler {
            BoxedHandler::Mutating(..) => false,
            BoxedHandler::Readonly(..) => true,
        }
    }

    /// Returns the identifier of this endpoint.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Handles a request.
    pub fn handle(&self, context: &mut EndpointContext, request: Value) -> ApiResult<Value> {
        self.handler.invoke(context, request)
    }

    /// Adds an `ApiContext` to this endpoint, allowing it to be executed
    /// given a request.
    pub fn with_context<'a, 'b>(&'a self, context: &'b ApiContext) -> EndpointWithContext<'a, 'b> {
        EndpointWithContext {
            endpoint: self,
            context,
        }
    }
}

/// An endpoint coupled with the `ApiContext`.
#[derive(Debug)]
pub struct EndpointWithContext<'a, 'b> {
    endpoint: &'a BoxedEndpoint,
    context: &'b ApiContext,
}

impl<'a, 'b> EndpointWithContext<'a, 'b> {
    /// Handles a request.
    pub fn handle(&self, request: Value) -> ApiResult<Value> {
        let mut ep_context = EndpointContext::new(self.context);
        let response = self.endpoint.handle(&mut ep_context, request)?;
        Ok(response)
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
    _marker: ::std::marker::PhantomData<T>,
}

impl<T> TransactionSink<T> {
    fn new() -> Self {
        TransactionSink { _marker: ::std::marker::PhantomData }
    }
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
    fn handle(&self, context: &mut EndpointContext, tx: T) -> ApiResult<TransactionResponse> {
        let tx = tx.into();
        let tx_hash = tx.hash();
        context.send(tx)?;
        Ok(TransactionResponse { tx_hash })
    }
}

/// Full collection of endpoints for a particular service.
#[derive(Debug, Default)]
pub struct ServiceApi {
    endpoints: HashMap<String, BoxedEndpoint>,
}

impl ServiceApi {
    /// Creates a new instance of service API with no endpoints.
    pub fn new() -> Self {
        ServiceApi::default()
    }

    /// Adds an instantiated endpoint.
    ///
    /// # Panics
    ///
    /// Panics if the API already contains an endpoint with the same identifier.
    pub fn insert(&mut self, endpoint: BoxedEndpoint) {
        let endpoint_id = endpoint.id().to_string();
        let old = self.endpoints.insert(endpoint_id.clone(), endpoint);
        assert!(old.is_none(), "Duplicate endpoint ID: {}", endpoint_id);
    }

    /// Adds a read request by its type `T`.
    ///
    /// # Panics
    ///
    /// Panics if the API already contains an endpoint with the same identifier.
    pub fn insert_read<T>(&mut self, read: T)
    where
        T: 'static + ReadRequest,
    {
        let endpoint = BoxedEndpoint::read_request(read);
        self.insert(endpoint);
    }

    /// Adds an endpoint by its type.
    ///
    /// # Panics
    ///
    /// Panics if the API already contains an endpoint with the same identifier.
    pub fn insert_endpoint<T>(&mut self, endpoint: T)
    where
        T: 'static + Endpoint,
    {
        let endpoint = BoxedEndpoint::endpoint(endpoint);
        self.insert(endpoint);
    }

    /// Add a sink for transactions.
    ///
    /// # Notes
    ///
    /// Type `T` can be a separate [`Transaction`] or a [`TransactionSet`].
    ///
    /// # Panics
    ///
    /// Panics if the API already contains an endpoint with the same identifier.
    /// This in particular means that the method can only be called once on an API instance.
    ///
    /// [`Transaction`]: ../../blockchain/trait.Transaction.html
    /// [`TransactionSet`]: ../../blockchain/trait.TransactionSet.html
    pub fn set_transactions<T>(&mut self)
    where
        T: 'static + Into<Box<Transaction>> + Serialize + DeserializeOwned + Send + Sync,
    {
        self.insert_endpoint(TransactionSink::<T>::new());
    }
}

/// Collection of named endpoints.
pub trait EndpointHolder {
    /// Tries to retrieve a reference to an endpoint with the specified identifier.
    fn endpoint(&self, id: &str) -> Option<&BoxedEndpoint>;

    /// Introduces a filter for this API.
    fn filter<F>(&self, predicate: F) -> Filter<Self, F>
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
pub struct Filter<'a, T: 'a + ?Sized, F> {
    base: &'a T,
    predicate: F,
}

impl<'a, T, F> EndpointHolder for Filter<'a, T, F>
where
    T: EndpointHolder,
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
