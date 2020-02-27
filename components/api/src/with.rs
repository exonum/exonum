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

use chrono::{DateTime, Utc};

use std::{future::Future, marker::PhantomData};

use super::{error, EndpointMutability};

/// Type alias for the usual synchronous result.
pub type Result<I> = std::result::Result<I, error::Error>;

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
    Deprecated {
        /// Optional value denoting the endpoint expiration date.
        discontinued_on: Option<DateTime<Utc>>,
        /// Optional additional description.
        description: Option<String>,
    },
}

/// Wrapper over an endpoint handler, which marks it as deprecated.
#[derive(Debug, Clone)]
pub struct Deprecated<Q, I, R, F> {
    /// Underlying API handler.
    pub handler: F,
    /// Optional endpoint expiration date.
    pub discontinued_on: Option<DateTime<Utc>>,
    /// Optional additional note.
    pub description: Option<String>,
    _query_type: PhantomData<Q>,
    _item_type: PhantomData<I>,
    _result_type: PhantomData<R>,
}

impl<Q, I, R, F> Deprecated<Q, I, R, F> {
    /// Creates a new `Deprecated` object.
    pub fn new(handler: F) -> Self {
        Self {
            handler,
            discontinued_on: None,
            description: None,
            _query_type: PhantomData,
            _item_type: PhantomData,
            _result_type: PhantomData,
        }
    }

    /// Adds an expiration date for endpoint.
    pub fn with_date(self, discontinued_on: DateTime<Utc>) -> Self {
        Self {
            discontinued_on: Some(discontinued_on),
            ..self
        }
    }

    /// Adds a description note to the warning, e.g. link to the new API documentation.
    pub fn with_description<S: Into<String>>(self, description: S) -> Self {
        Self {
            description: Some(description.into()),
            ..self
        }
    }

    /// Replaces the used handler with a new one.
    pub fn with_different_handler<F1, R1>(self, handler: F1) -> Deprecated<Q, I, R1, F1>
    where
        F1: Fn(Q) -> R1,
        R1: Future<Output = Result<I>>,
    {
        Deprecated {
            handler,
            discontinued_on: self.discontinued_on,
            description: self.description,

            _query_type: PhantomData,
            _item_type: PhantomData,
            _result_type: PhantomData,
        }
    }
}

impl<Q, I, R, F> From<F> for Deprecated<Q, I, R, F>
where
    F: Fn(Q) -> R,
    R: Future<Output = Result<I>>,
{
    fn from(handler: F) -> Self {
        Self::new(handler)
    }
}

impl<'a, Q, I, R, F> From<Deprecated<Q, I, R, F>> for With<Q, I, R, F> {
    fn from(deprecated: Deprecated<Q, I, R, F>) -> Self {
        Self {
            handler: deprecated.handler,
            actuality: Actuality::Deprecated {
                discontinued_on: deprecated.discontinued_on,
                description: deprecated.description,
            },
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

impl<Q, I, R, F> From<F> for With<Q, I, R, F>
where
    F: Fn(Q) -> R,
    R: Future<Output = Result<I>>,
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
