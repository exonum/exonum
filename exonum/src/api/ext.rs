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
//! # Endpoints
//!
//! Endoint is the basic building block of service APIs. There are two kinds of endpoints:
//!
//! - **Constant** endpoints that only read data from the blockchain, (maybe) process it
//!   and serve to clients
//! - **Mutating** endpoints, which have a more complete access to blockchain.
//!
//! You can use [`TypedEndpoint`] for more object-oriented endpoint declaration, or
//! use `new` / `create_mut` constructors in [`Endpoint`] for a more functional approach.
//!
//! [`Endpoint`]: struct.Endpoint.html
//! [`TypedEndpoint`]: trait.TypedEndpoint.html
//!
//! # Examples
//!
//! ```
//! # #[macro_use] extern crate exonum;
//! use exonum::api::ext::*;
//! use exonum::api::iron::{self, IronAdapter};
//! # use exonum::blockchain::{ApiContext, Blockchain, ExecutionResult, Service, Transaction};
//! # use exonum::crypto::{Hash, PublicKey};
//! # use exonum::encoding;
//! # use exonum::messages::RawTransaction;
//! # use exonum::storage::{Fork, Snapshot};
//!
//! // Transactions
//! transactions! {
//!     Any {
//!         const SERVICE_ID = 1000;
//!
//!         struct CreateWallet { /* ... */ }
//!         struct Transfer { /* ... */ }
//!     }
//! }
//! # impl Transaction for CreateWallet {
//! #     fn verify(&self) -> bool { true }
//! #     fn execute(&self, _: &mut Fork) -> ExecutionResult { Ok(()) }
//! # }
//! # impl Transaction for Transfer {
//! #     fn verify(&self) -> bool { true }
//! #     fn execute(&self, _: &mut Fork) -> ExecutionResult { Ok(()) }
//! # }
//!
//! // Service schema containing wallet balances
//! struct Schema { /* ... */ }
//!
//! impl Schema {
//! #   fn new<S: AsRef<Snapshot>>(snapshot: S) -> Self { Schema { } }
//!     pub fn balance(&self, key: &PublicKey) -> Option<u64> {
//!         // ...
//! #       Some(42)
//!     }
//! }
//!
//! // Read request: get the balance of a specific wallet.
//! pub const BALANCE_SPEC: Spec = Spec {
//!     id: "balance",
//!     visibility: Visibility::Public,
//! };
//!
//! pub fn balance(ctx: &Context, key: PublicKey) -> ApiResult<Option<u64>> {
//!     let schema = Schema::new(ctx.snapshot());
//!     Ok(schema.balance(&key))
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
//!         api.set_transactions::<Any>();
//!         api.insert(BALANCE_SPEC, Endpoint::new(balance));
//!         Some(IronAdapter::with_context(context).create_handler(api))
//!     }
//! }
//! # fn main() { }
//! ```

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{self, Value};

use std::borrow::Borrow;
use std::collections::HashMap;
use std::{fmt, io};

use blockchain::{Blockchain, SendError, Transaction};
use crypto::{Hash, PublicKey};
use messages::RawMessage;
use storage::Snapshot;

/// The specification for the "standard" transaction sink.
///
/// Transaction sinks can be added to service APIs using the [`set_transactions`]
/// method in `ServiceApi`; see it for more details.
///
/// [`set_transactions`]: struct.ServiceApi.html#method.set_transactions
pub const TRANSACTIONS: Spec = Spec {
    id: "transactions",
    visibility: Visibility::Public,
};

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

/// Specification of an endpoint.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spec {
    /// Endpoint identifier.
    pub id: &'static str,

    /// Visibility level of the endpoint.
    ///
    /// Endpoints with lesser visibility may be hidden by the transport adapters;
    /// e.g., an HTTP adapter may serve them on a different port behind a firewall.
    pub visibility: Visibility,
}

/// Possible visibility levels of service endpoints.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Visibility {
    /// Endpoint should be available to the general audience.
    Public,
    /// The access to the endpoint should be restricted.
    Private,
}

/// Context supplied to endpoints.
#[derive(Debug)]
pub struct Context<'a> {
    blockchain: &'a Blockchain,
    mutable: Option<MutContext<'a>>,
}

impl<'a> Context<'a> {
    fn new(blockchain: &'a Blockchain) -> Self {
        Context {
            blockchain,
            mutable: None,
        }
    }

    fn mutable(blockchain: &'a Blockchain) -> Self {
        Context {
            blockchain,
            mutable: Some(MutContext::new(blockchain)),
        }
    }

    /// Gets a snapshot of the current blockchain state.
    pub fn snapshot(&self) -> Box<Snapshot> {
        self.blockchain.snapshot()
    }

    /// Gets the public key used for signing operations by services.
    pub fn service_public_key(&self) -> &PublicKey {
        self.blockchain.service_public_key()
    }

    /// Accesses the mutable context.
    ///
    /// # Panics
    ///
    /// This method will panic if invoked in a constant endpoint.
    pub fn as_mut(&self) -> &MutContext<'a> {
        self.mutable.as_ref().expect(
            "Cannot mutate context in constant endpoint",
        )
    }
}

/// Context supplied to mutating endpoints.
#[derive(Debug)]
pub struct MutContext<'a> {
    blockchain: &'a Blockchain,
}

impl<'a> MutContext<'a> {
    fn new(blockchain: &'a Blockchain) -> Self {
        MutContext { blockchain }
    }

    /// Queues a transaction for sending over the network and including into the blockchain.
    ///
    /// The transaction should already be signed.
    pub fn send<T>(&self, transaction: T) -> Result<(), ApiError>
    where
        T: Into<Box<Transaction>>,
    {
        self.blockchain
            .api_sender()
            .send(transaction.into())
            .map_err(ApiError::from)
    }

    /// Queues a transaction for signing, sending over the network and including
    /// into the blockchain.
    ///
    /// The transaction is signed with the service secret key; the hash of the signed
    /// transaction is returned.
    pub fn sign_and_send(&self, message: &RawMessage) -> Result<Hash, ApiError> {
        self.blockchain
            .api_sender()
            .sign_and_send(message)
            .map_err(ApiError::from)
    }
}

/// A concrete type that all endpoints can be converted into.
///
/// # Examples
///
/// Endpoint using a custom request parsing logic.
///
/// ```
/// # extern crate exonum;
/// #[macro_use] extern crate serde_json;
/// # use exonum::api::ext::Endpoint;
/// # use exonum::blockchain::{Blockchain, ExecutionResult, Transaction};
/// # use exonum::crypto::{self, PublicKey};
/// # use exonum::storage::Snapshot;
/// use serde_json::Value;
///
/// // Service schema containing balances.
/// struct Schema { /* ... */ }
///
/// impl Schema {
/// #   fn new<S: AsRef<Snapshot>>(snapshot: S) -> Self { Schema { } }
///     pub fn balance(&self, key: &PublicKey) -> Option<u64> {
///         // ...
/// #       Some(42)
///     }
/// }
///
/// # fn main() {
/// let alice_key: PublicKey = // ...
/// #   PublicKey::new([0; 32]);
/// let endpoint = Endpoint::new(move |context, req: serde_json::Value| {
///     let pubkey: PublicKey = serde_json::from_value(req)
///         .unwrap_or(alice_key);
///     let balance = Schema::new(context.snapshot())
///         .balance(&pubkey);
///     Ok(balance)
/// });
/// # }
/// ```
///
/// Custom transaction sender creating and signing transactions signed with
/// the service secret key.
///
/// ```
/// # #[macro_use] extern crate exonum;
/// # use exonum::api::ext::{ApiError, Endpoint, MutContext};
/// # use exonum::blockchain::{ApiContext, ExecutionResult, Transaction};
/// # use exonum::crypto::{self, CryptoHash, Hash};
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
/// # fn main() {
/// let secret_key = // ...
/// #                crypto::gen_keypair().1;
/// let sender = move |ctx: &MutContext, req: (u64, String)| {
///     let tx = MyTransaction::new(req.0, &req.1, &secret_key);
///     let tx_hash = tx.hash();
///     ctx.send(tx)?;
///     Ok(tx_hash)
/// };
/// let sender = Endpoint::create_mut(sender);
/// # }
/// ```
pub struct Endpoint {
    handler: Box<Fn(&Context, Value) -> ApiResult<Value> + Send + Sync>,
    constant: bool,
}

impl fmt::Debug for Endpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        formatter
            .debug_struct("Endpoint")
            .field("constant", &self.constant)
            .finish()
    }
}

impl Endpoint {
    /// Creates a constant endpoint from a given closure.
    pub fn new<T, U, F>(handler: F) -> Self
    where
        T: DeserializeOwned,
        U: Serialize,
        F: 'static + Fn(&Context, T) -> ApiResult<U> + Send + Sync,
    {
        Endpoint {
            handler: Box::new(move |ctx, req| {
                Endpoint::wrap(req, |typed_req| handler(ctx, typed_req))
            }),
            constant: true,
        }
    }

    /// Creates a mutating endpoint from a given closure.
    pub fn create_mut<T, U, F>(handler: F) -> Self
    where
        T: DeserializeOwned,
        U: Serialize,
        F: 'static + Fn(&MutContext, T) -> ApiResult<U> + Send + Sync,
    {
        Endpoint {
            handler: Box::new(move |ctx, req| {
                Endpoint::wrap(req, |typed_req| handler(ctx.as_mut(), typed_req))
            }),
            constant: false,
        }
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
    pub fn constant(&self) -> bool {
        self.constant
    }

    /// Handles a request.
    pub fn handle<T: Borrow<Blockchain>>(
        &self,
        blockchain: &T,
        request: Value,
    ) -> ApiResult<Value> {
        let context = if self.constant {
            Context::new(blockchain.borrow())
        } else {
            Context::mutable(blockchain.borrow())
        };
        (self.handler)(&context, request)
    }
}

/// Strongly typed endpoint.
///
/// Implement this trait to get more structured endpoint documentation for endpoints of
/// your service.
///
/// # Examples
///
/// ## Constant endpoint
///
/// ```
/// # use exonum::api::ext::{ApiResult, Context, ServiceApi, TypedEndpoint, Visibility};
/// # use exonum::crypto::PublicKey;
/// struct GetBalance;
///
/// impl TypedEndpoint for GetBalance {
///     type Arg = PublicKey;
///     type Output = u64;
///
///     const ID: &'static str = "balance";
///     const VIS: Visibility = Visibility::Public;
///
///     fn call(&self, ctx: &Context, arg: PublicKey) -> ApiResult<u64> {
///         // ...
/// #       Ok(42)
///     }
/// }
///
/// let mut api = ServiceApi::new();
/// GetBalance.wire(&mut api);
/// ```
///
/// ## Mutating endpoint
///
/// ```
/// # #[macro_use] extern crate exonum;
/// # use exonum::api::ext::{ApiResult, Context, ServiceApi, TypedEndpoint, Visibility};
/// # use exonum::blockchain::{ExecutionResult, Transaction};
/// # use exonum::crypto::{self, CryptoHash, Hash, PublicKey, SecretKey};
/// # use exonum::storage::Fork;
/// // Suppose we have this transaction spec.
/// transactions! {
///     Any {
///         const SERVICE_ID = // ...
/// #                          1000;
///
///         struct MyTransaction {
///             public_key: &PublicKey,
///             data: u64,
///         }
///         // Other transactions...
///     }
/// }
/// # impl Transaction for MyTransaction {
/// #     fn verify(&self) -> bool { true }
/// #     fn execute(&self, _: &mut Fork) -> ExecutionResult { Ok(()) }
/// # }
///
/// // Transaction signer
/// pub struct TransactionSigner {
///     public_key: PublicKey,
///     secret_key: SecretKey,
/// }
///
/// impl TypedEndpoint for TransactionSigner {
///     type Arg = u64;
///     type Output = Hash;
///
///     const ID: &'static str = "send";
///     const VIS: Visibility = Visibility::Private;
///     const CONSTANT: bool = false;
///
///     fn call(&self, ctx: &Context, data: u64) -> ApiResult<Hash> {
///         let tx = MyTransaction::new(&self.public_key, data, &self.secret_key);
///         let tx_hash = tx.hash();
///         ctx.as_mut().send(tx)?;
///         Ok(tx_hash)
///     }
/// }
///
/// # fn main() {
/// let (public_key, secret_key) = crypto::gen_keypair();
/// let mut api = ServiceApi::new();
/// TransactionSigner { public_key, secret_key }.wire(&mut api);
/// # }
/// ```
pub trait TypedEndpoint: Send + Sync {
    /// Argument supplied to the endpoint during invocation.
    type Arg: 'static + DeserializeOwned;
    /// Output of the endpoint invocation.
    type Output: 'static + Serialize;

    /// Identifier of the endpoint.
    ///
    /// Equivalent to [`Spec.id`](struct.Spec.html#structfield.id).
    const ID: &'static str;

    /// Level of visibility of the endpoint.
    ///
    /// Endpoints with lesser visibility may be hidden by the transport adapters;
    /// e.g., an HTTP adapter may serve them on a different port behind a firewall.
    ///
    /// Equivalent to [`Spec.visibility`](struct.Spec.html#structfield.visibility).
    const VIS: Visibility;

    /// Is this endpoint constant? The default value is `true`.
    ///
    /// Constant endpoints only read from the storage. Non-constant (mutating) endpoints
    /// can also mutate the node state (e.g., by sending transactions to the network).
    const CONSTANT: bool = true;

    /// Invokes the endpoint.
    ///
    /// # Notes
    ///
    /// Endpoints with `CONSTANT` set to `true` cannot access the mutating
    /// methods of the context via `context.as_mut()`; doing so will lead to a panic.
    fn call(&self, context: &Context, arg: Self::Arg) -> ApiResult<Self::Output>;

    /// Returns the specification for this endpoint.
    fn spec() -> Spec {
        Spec {
            id: Self::ID,
            visibility: Self::VIS,
        }
    }

    /// Adds this endpoint into the service API.
    fn wire(self, api: &mut ServiceApi)
    where
        Self: 'static + Sized,
    {
        api.insert(Self::spec(), self);
    }
}

impl<T: TypedEndpoint + 'static> From<T> for Endpoint {
    fn from(endpoint: T) -> Self {
        Endpoint {
            handler: Box::new(move |ctx, req| {
                Endpoint::wrap(req, |typed_req| endpoint.call(ctx, typed_req))
            }),
            constant: T::CONSTANT,
        }
    }
}

/// The response returned by transaction sinks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionResponse {
    /// Hash of the transaction.
    pub tx_hash: Hash,
}

/// Default processing sink for transactions.
fn transaction_sink<T>(context: &MutContext, tx: T) -> ApiResult<TransactionResponse>
where
    T: Into<Box<Transaction>> + Serialize + DeserializeOwned + Send + Sync,
{
    let tx = tx.into();
    let tx_hash = tx.hash();
    context.send(tx)?;
    Ok(TransactionResponse { tx_hash })
}

/// Full collection of endpoints for a particular service.
#[derive(Debug, Default)]
pub struct ServiceApi {
    endpoints: HashMap<String, (Spec, Endpoint)>,
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
    pub fn insert<T: Into<Endpoint>>(&mut self, spec: Spec, endpoint: T) {
        let endpoint = endpoint.into();
        let old = self.endpoints.insert(spec.id.to_owned(), (spec, endpoint));
        assert!(old.is_none(), "Duplicate endpoint ID: {}", spec.id);
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
        self.insert(TRANSACTIONS, Endpoint::create_mut(transaction_sink::<T>));
    }

    /// Splits this API into two parts according to the given predicate.
    ///
    /// The endpoints satisfying the predicate go to the first struct returned by the method,
    /// and the ones not satisfying it to the second one.
    pub fn split_by<F>(self, mut predicate: F) -> (Self, Self)
    where
        F: FnMut(Spec) -> bool,
    {
        let mut matches = ServiceApi::new();
        let mut non_matches = ServiceApi::new();

        for (id, (spec, endpoint)) in self.endpoints {
            if predicate(spec) {
                matches.endpoints.insert(id, (spec, endpoint));
            } else {
                non_matches.endpoints.insert(id, (spec, endpoint));
            }
        }

        (matches, non_matches)
    }

    /// Returns the public part of the service API.
    pub fn public(mut self) -> Self {
        self.endpoints.retain(|_, &mut (spec, _)| {
            spec.visibility == Visibility::Public
        });
        self
    }

    /// Returns the private part of the service API.
    pub fn private(mut self) -> Self {
        self.endpoints.retain(|_, &mut (spec, _)| {
            spec.visibility == Visibility::Private
        });
        self
    }
}

/// Collection of named endpoints.
pub trait EndpointHolder {
    /// Tries to retrieve a reference to an endpoint with the specified identifier.
    fn endpoint(&self, id: &str) -> Option<&Endpoint>;

    /// Introduces a filter for this API.
    fn filter<F>(&self, predicate: F) -> Filter<Self, F>
    where
        F: Fn(&Endpoint) -> bool,
    {
        Filter {
            base: self,
            predicate,
        }
    }
}

impl EndpointHolder for ServiceApi {
    fn endpoint(&self, id: &str) -> Option<&Endpoint> {
        self.endpoints.get(id).map(|&(.., ref endpoint)| endpoint)
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
    F: Fn(&Endpoint) -> bool,
{
    fn endpoint(&self, id: &str) -> Option<&Endpoint> {
        let endpoint = self.base.endpoint(id)?;

        if (self.predicate)(endpoint) {
            Some(endpoint)
        } else {
            None
        }
    }
}

impl<'a> ::std::ops::Index<&'a str> for ServiceApi {
    type Output = Endpoint;

    fn index(&self, idx: &'a str) -> &Endpoint {
        self.endpoint(idx).expect(
            &format!("Unknown endpoint ID: {}", idx),
        )
    }
}

impl ::std::ops::Index<Spec> for ServiceApi {
    type Output = Endpoint;

    fn index(&self, idx: Spec) -> &Endpoint {
        self.endpoints
            .get(idx.id)
            .and_then(|&(spec, ref endpoint)| if spec == idx {
                Some(endpoint)
            } else {
                None
            })
            .expect(&format!("Unknown endpoint spec: {:?}", idx))
    }
}
