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

//! Actix-web API backend.

use actix_web::{self, AsyncResponder, FromRequest, HttpMessage, HttpResponse, Query};
use futures::{Future, IntoFuture};
use serde::de::DeserializeOwned;
use serde::Serialize;

use std::fmt;
use std::sync::Arc;

use api_ng::error::Error as ApiError;
use api_ng::{FutureResult, IntoApiBackend, NamedWith, Result, ServiceApiBackend, ServiceApiScope,
             ServiceApiState, ServiceApiStateMut};

/// Type alias for the concrete API http response
pub type FutureResponse = actix_web::FutureResponse<HttpResponse, actix_web::Error>;
/// Type alias for the concrete API http request.
pub type HttpRequest = actix_web::HttpRequest<ServiceApiStateMut>;
/// Type alias for the inner actix-web http requests handler.
pub type RawHandler = Fn(HttpRequest) -> FutureResponse + 'static + Send + Sync;

/// Raw actix-web backend requests handler.
#[derive(Clone)]
pub struct RequestHandler {
    /// Endpoint name.
    pub name: &'static str,
    /// Endpoint http method.
    pub method: actix_web::http::Method,
    /// Inner handler
    pub inner: Arc<RawHandler>,
}

impl fmt::Debug for RequestHandler {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("RequestHandler")
            .field("name", &self.name)
            .field("method", &self.method)
            .finish()
    }
}

/// API builder for the actix-web backend,
#[derive(Debug, Clone, Default)]
pub struct ApiBuilder {
    handlers: Vec<RequestHandler>,
}

impl ApiBuilder {
    /// Constructs a new backend builder instance.
    pub fn new() -> ApiBuilder {
        ApiBuilder::default()
    }

    /// Finishes API building and returns list of request handlers.
    pub fn finish(self) -> Vec<RequestHandler> {
        self.handlers
    }
}

impl ServiceApiBackend for ApiBuilder {
    type Handler = RequestHandler;
    type Scope = actix_web::Scope<ServiceApiStateMut>;

    fn raw_handler(&mut self, handler: Self::Handler) -> &mut Self {
        self.handlers.push(handler);
        self
    }

    fn wire(&self, mut output: Self::Scope) -> Self::Scope {
        for handler in self.handlers.clone() {
            output = output.route(handler.name, handler.method.clone(), move |request| {
                (handler.inner)(request)
            });
        }
        output
    }
}

impl<Q, I, F> From<NamedWith<ServiceApiState, Q, I, Result<I>, F>> for RequestHandler
where
    F: for<'r> Fn(&'r ServiceApiState, Q) -> Result<I> + 'static + Send + Sync,
    Q: DeserializeOwned + 'static,
    I: Serialize + 'static,
{
    fn from(f: NamedWith<ServiceApiState, Q, I, Result<I>, F>) -> Self {
        let handler = f.inner.handler;
        let index = move |request: HttpRequest| -> FutureResponse {
            let context = request.state();
            let future = Query::from_request(&request, &())
                .map(|query: Query<Q>| query.into_inner())
                .and_then(|query| handler(context, query).map_err(From::from))
                .and_then(|value| Ok(HttpResponse::Ok().json(value)))
                .into_future();
            Box::new(future)
        };

        RequestHandler {
            name: f.name,
            method: actix_web::http::Method::GET,
            inner: Arc::from(index) as Arc<RawHandler>,
        }
    }
}

impl<Q, I, F> From<NamedWith<ServiceApiStateMut, Q, I, Result<I>, F>> for RequestHandler
where
    F: for<'r> Fn(&'r ServiceApiStateMut, Q) -> Result<I> + 'static + Clone + Send + Sync,
    Q: DeserializeOwned + 'static,
    I: Serialize + 'static,
{
    fn from(f: NamedWith<ServiceApiStateMut, Q, I, Result<I>, F>) -> Self {
        let handler = f.inner.handler;
        let index = move |request: HttpRequest| -> FutureResponse {
            let handler = handler.clone();
            let context = request.state().clone();
            request
                .json()
                .from_err()
                .and_then(move |query: Q| {
                    handler(&context, query)
                        .map(|value| HttpResponse::Ok().json(value))
                        .map_err(From::from)
                })
                .responder()
        };

        RequestHandler {
            name: f.name,
            method: actix_web::http::Method::POST,
            inner: Arc::from(index) as Arc<RawHandler>,
        }
    }
}

impl<Q, I, F> From<NamedWith<ServiceApiState, Q, I, FutureResult<I>, F>> for RequestHandler
where
    F: for<'r> Fn(&'r ServiceApiState, Q) -> FutureResult<I> + 'static + Clone + Send + Sync,
    Q: DeserializeOwned + 'static,
    I: Serialize + 'static,
{
    fn from(f: NamedWith<ServiceApiState, Q, I, FutureResult<I>, F>) -> Self {
        let handler = f.inner.handler;
        let index = move |request: HttpRequest| -> FutureResponse {
            let context = request.state().clone();
            let handler = handler.clone();
            Query::from_request(&request, &())
                .map(move |query: Query<Q>| query.into_inner())
                .into_future()
                .and_then(move |query| handler(&context, query).map_err(From::from))
                .map(|value| HttpResponse::Ok().json(value))
                .responder()
        };

        RequestHandler {
            name: f.name,
            method: actix_web::http::Method::GET,
            inner: Arc::from(index) as Arc<RawHandler>,
        }
    }
}

impl<Q, I, F> From<NamedWith<ServiceApiStateMut, Q, I, FutureResult<I>, F>> for RequestHandler
where
    F: for<'r> Fn(&'r ServiceApiStateMut, Q) -> FutureResult<I> + 'static + Clone + Send + Sync,
    Q: DeserializeOwned + 'static,
    I: Serialize + 'static,
{
    fn from(f: NamedWith<ServiceApiStateMut, Q, I, FutureResult<I>, F>) -> Self {
        let handler = f.inner.handler;
        let index = move |request: HttpRequest| -> FutureResponse {
            let handler = handler.clone();
            let context = request.state().clone();
            request
                .json()
                .from_err()
                .and_then(move |query: Q| {
                    handler(&context, query)
                        .map(|value| HttpResponse::Ok().json(value))
                        .map_err(From::from)
                })
                .responder()
        };

        RequestHandler {
            name: f.name,
            method: actix_web::http::Method::POST,
            inner: Arc::from(index) as Arc<RawHandler>,
        }
    }
}

impl From<ApiError> for actix_web::Error {
    fn from(e: ApiError) -> Self {
        use actix_web::error;
        match e {
            ApiError::BadRequest(err) => error::ErrorBadRequest(err),
            ApiError::InternalError(err) => error::ErrorInternalServerError(err),
            ApiError::Io(err) => error::ErrorInternalServerError(err),
            ApiError::Storage(err) => error::ErrorInternalServerError(err),
            ApiError::NotFound(err) => error::ErrorNotFound(err),
            ApiError::Unauthorized => error::ErrorUnauthorized(""),
        }
    }
}

impl IntoApiBackend for actix_web::Scope<ServiceApiStateMut> {
    fn extend<'a, I>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = &'a (&'static str, ServiceApiScope)>,
    {
        for mut item in items {
            self = self.nested(item.0, move |scope| item.1.actix_backend.wire(scope))
        }
        self
    }
}
