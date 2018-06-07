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

use super::{error, ServiceApiState, ServiceApiStateMut};

/// Type alias for the usual synchronous result.
pub type Result<I> = ::std::result::Result<I, error::Error>;
/// Type alias for the asynchronous result that will appear in future.
pub type FutureResult<I> = Box<Future<Item = I, Error = error::Error>>;

/// API endpoint handler extractor which can extract handler from various entities.
#[derive(Debug)]
pub struct With<S, Q, I, R, F> {
    /// Extracted API handler.
    pub handler: F,
    _context_type: ::std::marker::PhantomData<S>,
    _query_type: ::std::marker::PhantomData<Q>,
    _item_type: ::std::marker::PhantomData<I>,
    _result_type: ::std::marker::PhantomData<R>,
}

/// API Endpoint extractor that also contains endpoint name.
#[derive(Debug)]
pub struct NamedWith<S, Q, I, R, F> {
    /// Endpoint name.
    pub name: &'static str,
    /// Extracted endpoint handler.
    pub inner: With<S, Q, I, R, F>,
}

impl<S, Q, I, R, F> NamedWith<S, Q, I, R, F> {
    /// Creates the named endpoint extractor from the given handler.
    pub fn new<H>(name: &'static str, handler: H) -> Self
    where
        H: Into<With<S, Q, I, R, F>>,
    {
        NamedWith {
            name,
            inner: handler.into(),
        }
    }
}

impl<Q, I, F> From<F> for With<ServiceApiState, Q, I, Result<I>, F>
where
    F: for<'r> Fn(&'r ServiceApiState, Q) -> Result<I>,
{
    fn from(handler: F) -> Self {
        With {
            handler,
            _context_type: ::std::marker::PhantomData,
            _query_type: ::std::marker::PhantomData,
            _item_type: ::std::marker::PhantomData,
            _result_type: ::std::marker::PhantomData,
        }
    }
}

impl<Q, I, F> From<F> for With<ServiceApiStateMut, Q, I, Result<I>, F>
where
    F: for<'r> Fn(&'r ServiceApiStateMut, Q) -> Result<I>,
{
    fn from(handler: F) -> Self {
        With {
            handler,
            _context_type: ::std::marker::PhantomData,
            _query_type: ::std::marker::PhantomData,
            _item_type: ::std::marker::PhantomData,
            _result_type: ::std::marker::PhantomData,
        }
    }
}

impl<Q, I, F> From<F> for With<ServiceApiState, Q, I, FutureResult<I>, F>
where
    F: for<'r> Fn(&'r ServiceApiState, Q) -> FutureResult<I>,
{
    fn from(handler: F) -> Self {
        With {
            handler,
            _context_type: ::std::marker::PhantomData,
            _query_type: ::std::marker::PhantomData,
            _item_type: ::std::marker::PhantomData,
            _result_type: ::std::marker::PhantomData,
        }
    }
}

impl<Q, I, F> From<F> for With<ServiceApiStateMut, Q, I, FutureResult<I>, F>
where
    F: for<'r> Fn(&'r ServiceApiStateMut, Q) -> FutureResult<I>,
{
    fn from(handler: F) -> Self {
        With {
            handler,
            _context_type: ::std::marker::PhantomData,
            _query_type: ::std::marker::PhantomData,
            _item_type: ::std::marker::PhantomData,
            _result_type: ::std::marker::PhantomData,
        }
    }
}
