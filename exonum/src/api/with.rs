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

use futures::Future;

use std::marker::PhantomData;

use super::{error, EndpointMutability};

/// Type alias for the usual synchronous result.
pub type Result<I> = std::result::Result<I, error::Error>;
/// Type alias for the asynchronous result that will be ready in the future.
pub type FutureResult<I> = Box<dyn Future<Item = I, Error = error::Error>>;

/// API endpoint handler extractor which can extract a handler from various entities.
///
/// The basic idea of this structure is to extract type parameters from the given handler,
/// thus, it becomes possible to distinguish different types of closures in compile time.
/// This structure allows applying anonymous functions to endpoints.
///
/// For example, for a handler with signature:
///
/// `Fn(query: MyQuery) -> Result<MyResponse, api::Error>`
///
/// Extracted types are:
///
/// - `Q` is `MyQuery`, i.e. type of query.
/// - `I` is `MyResponse`, i.e. type of response item.
/// - `R` is `Result<I, api::Error>`, i.e. complete type of result.
#[derive(Debug)]
pub struct With<Q, I, R, F> {
    /// Underlying API handler.
    pub handler: F,
    /// Endpoint actuality.
    pub actuality: Actuality,
    _query_type: PhantomData<Q>,
    _item_type: PhantomData<I>,
    _result_type: PhantomData<R>,
}

/// Endpoint actuality.
#[derive(Debug, Clone)]
pub enum Actuality {
    /// Endpoint is suitable for use.
    Actual,
    /// Endpoint is not recommended to use, the support of it will end soon.
    /// Contains optional value denoting the endpoint expiration date.
    Deprecated(Option<chrono::Date<chrono::Utc>>),
}

/// Wrapper over an endpoint handler, which marks it as deprecated.
#[derive(Debug, Clone)]
pub struct Deprecated<Q, I, R, F> {
    /// Underlying API handler.
    pub handler: F,
    /// Optional endpoint expiration date.
    pub deprecates_on: Option<chrono::Date<chrono::Utc>>,
    _query_type: PhantomData<Q>,
    _item_type: PhantomData<I>,
    _result_type: PhantomData<R>,
}

impl<Q, I, R, F> Deprecated<Q, I, R, F> {
    /// Adds an expiration date for endpoint.
    pub fn with_date(self, deprecates_on: chrono::Date<chrono::Utc>) -> Self {
        Self {
            deprecates_on: Some(deprecates_on),
            ..self
        }
    }
}

impl<Q, I, F> From<F> for Deprecated<Q, I, Result<I>, F>
where
    F: Fn(Q) -> Result<I>,
{
    fn from(handler: F) -> Self {
        Self {
            handler,
            deprecates_on: None,
            _query_type: PhantomData,
            _item_type: PhantomData,
            _result_type: PhantomData,
        }
    }
}

impl<Q, I, F> From<F> for Deprecated<Q, I, FutureResult<I>, F>
where
    F: Fn(Q) -> FutureResult<I>,
{
    fn from(handler: F) -> Self {
        Self {
            handler,
            deprecates_on: None,
            _query_type: PhantomData,
            _item_type: PhantomData,
            _result_type: PhantomData,
        }
    }
}

impl<Q, I, R, F> From<Deprecated<Q, I, R, F>> for With<Q, I, FutureResult<I>, F> {
    fn from(deprecated: Deprecated<Q, I, R, F>) -> Self {
        Self {
            handler: deprecated.handler,
            actuality: Actuality::Deprecated(deprecated.deprecates_on),
            _query_type: PhantomData,
            _item_type: PhantomData,
            _result_type: PhantomData,
        }
    }
}

/// API Endpoint extractor that also contains the endpoint name and its kind.
#[derive(Debug)]
pub struct NamedWith<Q, I, R, F> {
    /// Endpoint name.
    pub name: String,
    /// Extracted endpoint handler.
    pub inner: With<Q, I, R, F>,
    /// Endpoint mutability.
    pub mutability: EndpointMutability,
}

impl<Q, I, R, F> NamedWith<Q, I, R, F> {
    /// Creates a new instance from the given handler.
    pub fn new<S, W>(name: S, inner: W, mutability: EndpointMutability) -> Self
    where
        S: Into<String>,
        W: Into<With<Q, I, R, F>>,
    {
        Self {
            name: name.into(),
            inner: inner.into(),
            mutability,
        }
    }

    /// Creates a new mutable `NamedWith` from the given handler.
    pub fn mutable<S, W>(name: S, inner: W) -> Self
    where
        S: Into<String>,
        W: Into<With<Q, I, R, F>>,
    {
        Self {
            name: name.into(),
            inner: inner.into(),
            mutability: EndpointMutability::Mutable,
        }
    }

    /// Creates a new mutable `NamedWith` from the given handler.
    pub fn immutable<S, W>(name: S, inner: W) -> Self
    where
        S: Into<String>,
        W: Into<With<Q, I, R, F>>,
    {
        Self {
            name: name.into(),
            inner: inner.into(),
            mutability: EndpointMutability::Immutable,
        }
    }
}

// Implementations for `Result` and `query` parameters.

impl<Q, I, F> From<F> for With<Q, I, Result<I>, F>
where
    F: Fn(Q) -> Result<I>,
{
    fn from(handler: F) -> Self {
        Self {
            handler,
            actuality: Actuality::Actual,
            _query_type: PhantomData,
            _item_type: PhantomData,
            _result_type: PhantomData,
        }
    }
}

// Implementations for `FutureResult` and `query` parameters.

impl<Q, I, F> From<F> for With<Q, I, FutureResult<I>, F>
where
    F: Fn(Q) -> FutureResult<I>,
{
    fn from(handler: F) -> Self {
        Self {
            handler,
            actuality: Actuality::Actual,
            _query_type: PhantomData,
            _item_type: PhantomData,
            _result_type: PhantomData,
        }
    }
}
