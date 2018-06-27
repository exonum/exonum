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

use futures::Future;

use std::marker::PhantomData;

use super::{error, ServiceApiState};

/// Type alias for the usual synchronous result.
pub type Result<I> = ::std::result::Result<I, error::Error>;
/// Type alias for the asynchronous result that will appear in future.
pub type FutureResult<I> = Box<Future<Item = I, Error = error::Error>>;

/// API endpoint handler extractor which can extract handler from various entities.
/// The basic idea of this structure is to extract type parameters from the given handler,
/// thus, it becomes possible to distinguish different types of closures in compile time.
///
/// For example for handler with signature:
/// 
/// `Fn(state: &ServiceApiState, query: MyQuery) -> Result<MyResponse, api::Error>`
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
    _query_type: PhantomData<Q>,
    _item_type: PhantomData<I>,
    _result_type: PhantomData<R>,
}

/// Immutable endpoint marker. With it possible to create immutable kind of `NamedWith`.
#[derive(Debug)]
pub struct Immutable;

/// Mutable endpoint marker. With it possible to create mutable kind of `NamedWith`.
#[derive(Debug)]
pub struct Mutable;

/// API Endpoint extractor that also contains endpoint name and its kind.
#[derive(Debug)]
pub struct NamedWith<Q, I, R, F, K> {
    /// Endpoint name.
    pub name: String,
    /// Extracted endpoint handler.
    pub inner: With<Q, I, R, F>,
    _kind: PhantomData<K>,
}

impl<Q, I, R, F, K> NamedWith<Q, I, R, F, K> {
    /// Creates a new instance from the given handler.
    pub fn new<S, W>(name: S, inner: W) -> Self
    where
        S: Into<String>,
        W: Into<With<Q, I, R, F>>,
    {
        NamedWith {
            name: name.into(),
            inner: inner.into(),
            _kind: PhantomData::default(),
        }
    }
}

// Implementations for Result and query params.

impl<Q, I, F> From<F> for With<Q, I, Result<I>, F>
where
    F: for<'r> Fn(&'r ServiceApiState, Q) -> Result<I>,
{
    fn from(handler: F) -> Self {
        With {
            handler,
            _query_type: PhantomData,
            _item_type: PhantomData,
            _result_type: PhantomData,
        }
    }
}

// Implementations for FutureResult and query params.

impl<Q, I, F> From<F> for With<Q, I, FutureResult<I>, F>
where
    F: for<'r> Fn(&'r ServiceApiState, Q) -> FutureResult<I>,
{
    fn from(handler: F) -> Self {
        With {
            handler,
            _query_type: PhantomData,
            _item_type: PhantomData,
            _result_type: PhantomData,
        }
    }
}
