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
//! use exonum::api::ext::{ApiResult, Endpoint, EndpointContext, EndpointSpec, ServiceApi};
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
//! enum Read {}
//!
//! impl EndpointSpec for Read {
//!     type Request = ();
//!     type Response = u64;
//!     const ID: &'static str = "read";
//! }
//!
//! impl Endpoint for Read {
//!     fn handle(_: &EndpointContext, _: ()) -> ApiResult<u64> { Ok(42) }
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
//!         let api = ServiceApi::new()
//!             .add::<Read>()
//!             .add_transactions::<Any>();
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

use blockchain::{ApiContext, Blockchain, Transaction};
use crypto::{Hash, SecretKey};
use node::TransactionSend;
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
pub struct EndpointContext {
    blockchain: Blockchain,
    queue: Vec<Box<Transaction>>,
    unsigned_queue: Vec<Box<Transaction>>,
}

impl EndpointContext {
    fn new(blockchain: Blockchain) -> Self {
        EndpointContext {
            blockchain,
            queue: vec![],
            unsigned_queue: vec![],
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
        let transaction = transaction.into();
        if !transaction.verify() {
            return Err(ApiError::VerificationFail(transaction));
        }
        self.queue.push(transaction);
        Ok(())
    }

    /// Queues a transaction for signing, sending over the network and including
    /// into the blockchain.
    ///
    /// The transaction is signed with the service secret key.
    pub fn sign_and_send<T>(&mut self, transaction: T) -> Result<(), ApiError>
    where
        T: Into<Box<Transaction>>,
    {
        let transaction = transaction.into();

        debug_assert_eq!(*transaction.raw().signature(), ::crypto::Signature::zero());
        self.unsigned_queue.push(transaction);
        Ok(())
    }

    fn finalize(self, context: &ApiContext) -> Result<(), ApiError> {
        use messages::{MessageBuffer, RawMessage};

        // XXX: Unbelievable hacks
        fn sign_transaction(
            tx: Box<Transaction>,
            secret_key: &SecretKey,
            blockchain: &Blockchain,
        ) -> Box<Transaction> {
            let buffer = tx.raw().as_ref().to_vec();
            let mut buffer = MessageBuffer::from_vec(buffer);
            buffer.sign(secret_key);
            blockchain.tx_from_raw(RawMessage::new(buffer)).unwrap()
        }

        for tx in self.queue {
            context.node_channel().send_unchecked(tx).map_err(
                ApiError::TransactionNotSent,
            )?;
        }

        let signed_txs = self.unsigned_queue.into_iter().map(|tx| {
            sign_transaction(tx, context.secret_key(), context.blockchain())
        });
        for tx in signed_txs {
            context.node_channel().send_unchecked(tx).map_err(
                ApiError::TransactionNotSent,
            )?;
        }
        Ok(())
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
/// use exonum::api::ext::{ApiResult, Endpoint, EndpointContext, EndpointSpec};
/// # use exonum::crypto::PublicKey;
/// # use exonum::storage::Snapshot;
///
/// pub enum GetBalance {}
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
/// impl Endpoint for GetBalance {
///     fn handle(ctx: &EndpointContext, key: PublicKey) -> ApiResult<Option<u64>> {
///         let schema = Schema::new(ctx.snapshot());
///         Ok(schema.balance(&key))
///     }
/// }
/// # fn main() {}
/// ```
pub trait Endpoint: EndpointSpec + Send + Sync {
    /// Handles a request to the endpoint.
    ///
    /// # Important note
    ///
    /// Unlike with transaction handling, the core does not catch panics during
    /// the execution of `handle()`. Thus, any panic will lead to stopping
    /// the entire request processing thread.
    fn handle(context: &EndpointContext, request: Self::Request) -> ApiResult<Self::Response>;
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
/// # use exonum::api::ext::{ApiError, EndpointContext, EndpointSpec, MutatingEndpoint};
/// # use exonum::blockchain::{ApiContext, ExecutionResult, Transaction};
/// # use exonum::crypto::{CryptoHash, Hash, Signature};
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
/// pub enum SendTransaction {}
///
/// impl EndpointSpec for SendTransaction {
///     type Request = (u64, String);
///     type Response = Hash;
///     const ID: &'static str = "send-transaction";
/// }
///
/// impl MutatingEndpoint for SendTransaction {
///     fn handle(context: &mut EndpointContext, req: (u64, String)) -> Result<Hash, ApiError> {
///         let tx = MyTransaction::new_with_signature(req.0, &req.1, &Signature::zero());
///         let tx_hash = tx.hash();
///         context.sign_and_send(tx)?;
///         Ok(tx_hash)
///     }
/// }
/// # fn main() { }
/// ```
pub trait MutatingEndpoint: EndpointSpec + Send + Sync {
    /// Handles a request to the endpoint.
    ///
    /// # Important note
    ///
    /// Unlike with transaction handling, the core does not catch panics during
    /// the execution of `handle()`. Thus, any panic will lead to stopping
    /// the entire request processing thread.
    fn handle(context: &mut EndpointContext, request: Self::Request) -> ApiResult<Self::Response>;
}

/// Internally used version of `Endpoint`.
///
/// The type rarely is needed to be used directly; the preferable way
/// of implementing endpoints is to implement the [`Endpoint`] trait.
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
/// let endpoint = BoxedEndpoint::read_request(
///     "wallet",
///     move |context, req| {
///         let pubkey: PublicKey = serde_json::from_value(req)
///             .unwrap_or(alice_key);
///         let balance: Option<u64> = Schema::new(context.snapshot())
///             .balance(&pubkey);
///         Ok(json!(balance))
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
    pub fn read_request<S, F>(id: S, handler: F) -> Self
    where
        S: AsRef<str>,
        F: 'static + Fn(&EndpointContext, Value) -> ApiResult<Value> + Send + Sync,
    {
        BoxedEndpoint {
            id: id.as_ref().to_owned(),
            handler: BoxedHandler::Readonly(Box::new(handler)),
        }
    }

    /// Creates a full-access endpoint from a given closure.
    pub fn new<S, F>(id: S, handler: F) -> Self
    where
        S: AsRef<str>,
        F: 'static + Fn(&mut EndpointContext, Value) -> ApiResult<Value> + Send + Sync,
    {
        BoxedEndpoint {
            id: id.as_ref().to_owned(),
            handler: BoxedHandler::Mutating(Box::new(handler)),
        }
    }

    fn wrap<T, C, F>(context: C, req: Value, handler: F) -> ApiResult<Value>
    where
        T: EndpointSpec,
        F: Fn(C, T::Request) -> ApiResult<T::Response>,
    {
        let request: T::Request = serde_json::from_value(req).map_err(|e| {
            ApiError::BadRequest(e.into())
        })?;
        let response = handler(context, request)?;
        let response = serde_json::to_value(response).map_err(|e| {
            ApiError::InternalError(e.into())
        })?;
        Ok(response)
    }

    fn from_endpoint<T: Endpoint>() -> Self {
        BoxedEndpoint::read_request(T::ID.to_owned(), |context, req| {
            BoxedEndpoint::wrap::<T, _, _>(context, req, T::handle)
        })
    }

    fn from_mut_endpoint<T: MutatingEndpoint>() -> Self {
        BoxedEndpoint::new(T::ID.to_owned(), |context, req| {
            BoxedEndpoint::wrap::<T, _, _>(context, req, T::handle)
        })
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

    /// Adds an `ApiContext` to this transaction, allowing it to be executed
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
        let mut ep_context = EndpointContext::new(self.context.blockchain().clone());
        let response = self.endpoint.handle(&mut ep_context, request)?;
        ep_context.finalize(&self.context)?;
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

impl<T> MutatingEndpoint for TransactionSink<T>
where
    T: Into<Box<Transaction>>
        + Serialize
        + DeserializeOwned
        + Send
        + Sync,
{
    fn handle(context: &mut EndpointContext, transaction: T) -> ApiResult<TransactionResponse> {
        let transaction = transaction.into();
        let tx_hash = transaction.hash();
        context.send(transaction)?;
        Ok(TransactionResponse { tx_hash })
    }
}

/// Full collection of endpoints for a particular service.
#[derive(Debug)]
pub struct ServiceApi {
    endpoints: HashMap<String, BoxedEndpoint>,
}

impl ServiceApi {
    /// Creates a new instance of service API with no endpoints.
    pub fn new() -> Self {
        ServiceApi { endpoints: HashMap::new() }
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
    /// # Panics
    ///
    /// Panics if the builder already contains an endpoint with the same identifier.
    pub fn add<T>(self) -> Self
    where
        T: Endpoint,
    {
        let endpoint = BoxedEndpoint::from_endpoint::<T>();
        self.add_endpoint(endpoint)
    }

    /// Adds a mutating endpoint by its type.
    ///
    /// # Panics
    ///
    /// Panics if the builder already contains an endpoint with the same identifier.
    pub fn add_mut<T>(self) -> Self
    where
        T: MutatingEndpoint,
    {
        let endpoint = BoxedEndpoint::from_mut_endpoint::<T>();
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
        self.add_mut::<TransactionSink<T>>()
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

impl<'a, 's, T, F> ::std::ops::Index<&'s str> for Filter<'a, T, F>
where
    T: EndpointHolder,
    F: Fn(&BoxedEndpoint) -> bool,
{
    type Output = BoxedEndpoint;

    fn index(&self, idx: &'s str) -> &BoxedEndpoint {
        self.endpoint(idx).expect(
            &format!("Unknown endpoint ID: {}", idx),
        )
    }
}
