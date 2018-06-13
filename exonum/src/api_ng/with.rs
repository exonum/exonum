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

/// Immutable endpoint marker.
#[derive(Debug)]
pub struct Immutable;

/// Mutable endpoint marker.
#[derive(Debug)]
pub struct Mutable;

/// API endpoint handler extractor which can extract handler from various entities.
#[derive(Debug)]
pub struct With<Q, I, R, F, K> {
    /// Extracted API handler.
    pub handler: F,
    _query_type: PhantomData<Q>,
    _item_type: PhantomData<I>,
    _result_type: PhantomData<R>,
    _kind: PhantomData<K>,
}

/// API Endpoint extractor that also contains endpoint name.
#[derive(Debug)]
pub struct NamedWith<Q, I, R, F, K> {
    /// Endpoint name.
    pub name: &'static str,
    /// Extracted endpoint handler.
    pub inner: With<Q, I, R, F, K>,
}

// Implementations for Result and query params.

impl<Q, I, F, K> From<F> for With<Q, I, Result<I>, F, K>
where
    F: for<'r> Fn(&'r ServiceApiState, Q) -> Result<I>,
{
    fn from(handler: F) -> Self {
        With {
            handler,
            _query_type: PhantomData,
            _item_type: PhantomData,
            _result_type: PhantomData,
            _kind: PhantomData,
        }
    }
}

// Implementations for FutureResult and query params.

impl<Q, I, F, K> From<F> for With<Q, I, FutureResult<I>, F, K>
where
    F: for<'r> Fn(&'r ServiceApiState, Q) -> FutureResult<I>,
{
    fn from(handler: F) -> Self {
        With {
            handler,
            _query_type: PhantomData,
            _item_type: PhantomData,
            _result_type: PhantomData,
            _kind: PhantomData,
        }
    }
}
