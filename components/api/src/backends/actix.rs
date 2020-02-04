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

//! Actix-web API backend.
//!
//! [Actix-web](https://github.com/actix/actix-web) is an asynchronous backend
//! for HTTP API, based on the [Actix](https://github.com/actix/actix) framework.

pub use actix_web::middleware::cors::Cors;

use actix::{Actor, System};
use actix_web::{
    error::ResponseError, http::header, AsyncResponder, FromRequest, HttpMessage, HttpResponse,
    Query,
};
use failure::{ensure, format_err, Error};
use futures::{future::Either, sync::mpsc, Future, IntoFuture, Stream};
use serde::{de::DeserializeOwned, Serialize};

use std::{
    fmt,
    sync::Arc,
    thread::{self, JoinHandle},
};

use crate::{
    manager::{ApiManager, WebServerConfig},
    Actuality, AllowOrigin, ApiAccess, ApiAggregator, ApiBackend, ApiScope, EndpointMutability,
    Error as ApiError, ExtendApiBackend, FutureResult, NamedWith,
};

/// Type alias for the concrete `actix-web` HTTP response.
pub type FutureResponse = actix_web::FutureResponse<HttpResponse, actix_web::Error>;
/// Type alias for the concrete `actix-web` HTTP request.
pub type HttpRequest = actix_web::HttpRequest<()>;
/// Type alias for the inner `actix-web` HTTP requests handler.
pub type RawHandler = dyn Fn(HttpRequest) -> FutureResponse + 'static + Send + Sync;
/// Type alias for the `actix-web::App`.
pub type App = actix_web::App<()>;

/// Raw `actix-web` backend requests handler.
#[derive(Clone)]
pub struct RequestHandler {
    /// Endpoint name.
    pub name: String,
    /// Endpoint HTTP method.
    pub method: actix_web::http::Method,
    /// Inner handler.
    pub inner: Arc<RawHandler>,
}

impl fmt::Debug for RequestHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RequestHandler")
            .field("name", &self.name)
            .field("method", &self.method)
            .finish()
    }
}

/// API builder for the `actix-web` backend.
#[derive(Debug, Clone, Default)]
pub struct ApiBuilder {
    handlers: Vec<RequestHandler>,
}

impl ApiBuilder {
    /// Constructs a new backend builder instance.
    pub fn new() -> Self {
        Self::default()
    }
}

impl ApiBackend for ApiBuilder {
    type Handler = RequestHandler;
    type Backend = actix_web::Scope<()>;

    fn raw_handler(&mut self, handler: Self::Handler) -> &mut Self {
        self.handlers.push(handler);
        self
    }

    fn wire(&self, mut output: Self::Backend) -> Self::Backend {
        for handler in self.handlers.clone() {
            let inner = handler.inner;
            output = output.route(&handler.name, handler.method.clone(), move |request| {
                inner(request)
            });
        }
        output
    }
}

impl ExtendApiBackend for actix_web::Scope<()> {
    fn extend<'a, I>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = (&'a str, &'a ApiScope)>,
    {
        for item in items {
            self = self.nested(&item.0, move |scope| item.1.actix_backend.wire(scope))
        }
        self
    }
}

impl ResponseError for ApiError {
    fn error_response(&self) -> HttpResponse {
        let body = serde_json::to_value(&self.body).unwrap();
        let body = if body == serde_json::json!({}) {
            actix_web::Body::Empty
        } else {
            serde_json::to_string(&self.body).unwrap().into()
        };

        let mut response = HttpResponse::build(self.http_code)
            .header(header::CONTENT_TYPE, "application/problem+json")
            .body(body);

        response.headers_mut().extend(self.headers.clone());
        response
    }
}

/// Creates a `HttpResponse` object from the provided JSON value.
/// Depending on the `actuality` parameter value, the warning about endpoint
/// being deprecated can be added.
fn json_response<T: Serialize>(actuality: Actuality, json_value: T) -> HttpResponse {
    let mut response = HttpResponse::Ok();

    if let Actuality::Deprecated {
        ref discontinued_on,
        ref description,
    } = actuality
    {
        // There is a proposal for creating special deprecation header within HTTP,
        // but currently it's only a draft. So the conventional way to notify API user
        // about endpoint deprecation is setting the `Warning` header.
        let expiration_note = match discontinued_on {
            // Date is formatted according to HTTP-date format.
            Some(date) => format!(
                "The old API is maintained until {}.",
                date.format("%a, %d %b %Y %T GMT")
            ),
            None => "Currently there is no specific date for disabling this endpoint.".into(),
        };

        let mut warning_text = format!(
            "Deprecated API: This endpoint is deprecated, \
             see the service documentation to find an alternative. \
             {}",
            expiration_note
        );

        if let Some(description) = description {
            warning_text = format!("{} Additional information: {}.", warning_text, description);
        }

        let warning_string = create_warning_header(&warning_text);

        response.header(header::WARNING, warning_string);
    }

    response.json(json_value)
}

/// Formats warning string according to the following format:
/// "<warn-code> <warn-agent> \"<warn-text>\" [<warn-date>]"
/// <warn-code> in our case is 299, which means a miscellaneous persistent warning.
/// <warn-agent> is optional, so we set it to "-".
/// <warn-text> is a warning description, which is taken as an only argument.
/// <warn-date> is not required.
/// For details you can see RFC 7234, section 5.5: Warning.
fn create_warning_header(warning_text: &str) -> String {
    format!("299 - \"{}\"", warning_text)
}

impl From<EndpointMutability> for actix_web::http::Method {
    fn from(mutability: EndpointMutability) -> Self {
        match mutability {
            EndpointMutability::Immutable => actix_web::http::Method::GET,
            EndpointMutability::Mutable => actix_web::http::Method::POST,
        }
    }
}

impl<Q, I, F> From<NamedWith<Q, I, crate::Result<I>, F>> for RequestHandler
where
    F: Fn(Q) -> crate::Result<I> + 'static + Send + Sync + Clone,
    Q: DeserializeOwned + 'static,
    I: Serialize + 'static,
{
    fn from(f: NamedWith<Q, I, crate::Result<I>, F>) -> Self {
        // Convert handler that returns a `Result` into handler that will return `FutureResult`.
        let handler = f.inner.handler;
        let future_endpoint = move |query| -> Box<dyn Future<Item = I, Error = ApiError>> {
            let future = handler(query).into_future();
            Box::new(future)
        };
        let named_with_future = NamedWith::new(f.name, future_endpoint, f.mutability);

        // Then we can create a `RequestHandler` with the `From` specialization for future result.
        RequestHandler::from(named_with_future)
    }
}

/// Takes `HttpRequest` as a parameter and extracts query:
/// - If request is immutable, the query is parsed from query string,
/// - If request is mutable, the query is parsed from the request body as JSON.
fn extract_query<Q>(
    request: HttpRequest,
    mutability: EndpointMutability,
) -> impl Future<Item = Q, Error = actix_web::error::Error>
where
    Q: DeserializeOwned + 'static,
{
    match mutability {
        EndpointMutability::Immutable => {
            let future = Query::from_request(&request, &Default::default())
                .map(Query::into_inner)
                .map_err(From::from)
                .into_future();

            Either::A(future)
        }
        EndpointMutability::Mutable => {
            let future = request.json().from_err();
            Either::B(future)
        }
    }
}

impl<Q, I, F> From<NamedWith<Q, I, FutureResult<I>, F>> for RequestHandler
where
    F: Fn(Q) -> FutureResult<I> + 'static + Clone + Send + Sync,
    Q: DeserializeOwned + 'static,
    I: Serialize + 'static,
{
    fn from(f: NamedWith<Q, I, FutureResult<I>, F>) -> Self {
        let handler = f.inner.handler;
        let actuality = f.inner.actuality;
        let mutability = f.mutability;
        let index = move |request: HttpRequest| -> FutureResponse {
            let handler = handler.clone();
            let actuality = actuality.clone();
            extract_query(request, mutability)
                .and_then(move |query| {
                    handler(query)
                        .map(|value| json_response(actuality, value))
                        .map_err(From::from)
                })
                .responder()
        };

        Self {
            name: f.name,
            method: f.mutability.into(),
            inner: Arc::from(index) as Arc<RawHandler>,
        }
    }
}

/// Creates `actix_web::App` for the given aggregator and runtime configuration.
pub(crate) fn create_app(
    aggregator: &ApiAggregator,
    access: ApiAccess,
    runtime_config: &WebServerConfig,
) -> App {
    let mut app = App::new();
    app = app.scope("api", |scope| aggregator.extend_backend(access, scope));
    if let Some(ref allow_origin) = runtime_config.allow_origin {
        let cors = Cors::from(allow_origin);
        app = app.middleware(cors);
    }
    app
}

/// Actix system runtime handle.
pub struct SystemRuntime {
    system_thread: JoinHandle<Result<(), Error>>,
    system: System,
}

impl SystemRuntime {
    /// Starts actix system runtime along with all web runtimes.
    pub fn start(manager: ApiManager) -> Result<Self, Error> {
        // Creates a system thread.
        let (system_tx, system_rx) = mpsc::unbounded();
        let system_thread = thread::spawn(move || -> Result<(), Error> {
            let system = System::new("http-server");
            system_tx.unbounded_send(System::current())?;
            manager.start();

            // Starts actix-web runtime.
            let code = system.run();
            log::trace!("Actix runtime finished with code {}", code);
            ensure!(
                code == 0,
                "Actix runtime finished with the non zero error code: {}",
                code
            );
            Ok(())
        });

        // Receives addresses of runtime items.
        let system = system_rx
            .wait()
            .next()
            .ok_or_else(|| format_err!("Unable to receive actix system handle"))?
            .map_err(|()| format_err!("Unable to receive actix system handle"))?;
        Ok(SystemRuntime {
            system_thread,
            system,
        })
    }

    /// Stops the actix system runtime along with all web runtimes.
    pub fn stop(self) -> Result<(), Error> {
        // Stop actix system runtime.
        self.system.stop();
        self.system_thread.join().map_err(|e| {
            format_err!(
                "Unable to join actix web api thread, an error occurred: {:?}",
                e
            )
        })?
    }
}

impl fmt::Debug for SystemRuntime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SystemRuntime").finish()
    }
}

impl From<&AllowOrigin> for Cors {
    fn from(origin: &AllowOrigin) -> Self {
        match *origin {
            AllowOrigin::Any => Self::build().finish(),
            AllowOrigin::Whitelist(ref hosts) => {
                let mut builder = Self::build();
                for host in hosts {
                    builder.allowed_origin(host);
                }
                builder.finish()
            }
        }
    }
}

impl From<AllowOrigin> for Cors {
    fn from(origin: AllowOrigin) -> Self {
        Self::from(&origin)
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn assert_responses_eq(left: HttpResponse, right: HttpResponse) {
        assert_eq!(left.status(), right.status());
        assert_eq!(left.headers(), right.headers());
        assert_eq!(left.body(), right.body());
    }

    #[test]
    fn test_create_warning_header() {
        assert_eq!(
            &create_warning_header("Description"),
            "299 - \"Description\""
        );
    }

    #[test]
    fn json_responses() {
        use chrono::TimeZone;

        let actual_response = json_response(Actuality::Actual, 123);
        assert_responses_eq(actual_response, HttpResponse::Ok().json(123));

        let deprecated_response_no_deadline = json_response(
            Actuality::Deprecated {
                discontinued_on: None,
                description: None,
            },
            123,
        );
        let expected_warning_text =
            "Deprecated API: This endpoint is deprecated, \
             see the service documentation to find an alternative. \
             Currently there is no specific date for disabling this endpoint.";
        let expected_warning = create_warning_header(expected_warning_text);
        assert_responses_eq(
            deprecated_response_no_deadline,
            HttpResponse::Ok()
                .header(header::WARNING, expected_warning)
                .json(123),
        );

        let description = "Docs can be found on docs.rs".to_owned();
        let deprecated_response_with_description = json_response(
            Actuality::Deprecated {
                discontinued_on: None,
                description: Some(description),
            },
            123,
        );
        let expected_warning_text =
            "Deprecated API: This endpoint is deprecated, \
             see the service documentation to find an alternative. \
             Currently there is no specific date for disabling this endpoint. \
             Additional information: Docs can be found on docs.rs.";
        let expected_warning = create_warning_header(expected_warning_text);
        assert_responses_eq(
            deprecated_response_with_description,
            HttpResponse::Ok()
                .header(header::WARNING, expected_warning)
                .json(123),
        );

        let deadline = chrono::Utc.ymd(2020, 12, 31).and_hms(23, 59, 59);

        let deprecated_response_deadline = json_response(
            Actuality::Deprecated {
                discontinued_on: Some(deadline),
                description: None,
            },
            123,
        );
        let expected_warning_text =
            "Deprecated API: This endpoint is deprecated, \
             see the service documentation to find an alternative. \
             The old API is maintained until Thu, 31 Dec 2020 23:59:59 GMT.";
        let expected_warning = create_warning_header(expected_warning_text);
        assert_responses_eq(
            deprecated_response_deadline,
            HttpResponse::Ok()
                .header(header::WARNING, expected_warning)
                .json(123),
        );
    }

    #[test]
    fn api_error_to_http_response() {
        let response = ApiError::bad_request()
            .header(header::LOCATION, "location")
            .docs_uri("uri")
            .title("title")
            .detail("detail")
            .source("source")
            .error_code(42)
            .error_response();
        let body = crate::error::ErrorBody {
            docs_uri: "uri".into(),
            title: "title".into(),
            detail: "detail".into(),
            source: "source".into(),
            error_code: Some(42),
        };
        let expected = HttpResponse::build(crate::HttpStatusCode::BAD_REQUEST)
            .header(header::CONTENT_TYPE, "application/problem+json")
            .header(header::LOCATION, "location")
            .body(serde_json::to_string(&body).unwrap());
        assert_responses_eq(response, expected);
    }

    #[test]
    fn api_error_to_http_response_without_body() {
        let response = ApiError::bad_request().error_response();
        let expected = HttpResponse::build(crate::HttpStatusCode::BAD_REQUEST)
            .header(header::CONTENT_TYPE, "application/problem+json")
            .finish();
        assert_responses_eq(response, expected);
    }
}
