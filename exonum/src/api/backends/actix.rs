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
use failure::Error;
use futures::{sync::mpsc, Future, IntoFuture, Stream};
use serde::{
    de::{self, DeserializeOwned},
    ser, Serialize,
};

use std::{
    fmt,
    net::SocketAddr,
    result,
    str::FromStr,
    sync::Arc,
    thread::{self, JoinHandle},
};

use crate::api::{
    self,
    manager::{ApiManager, UpdateEndpoints},
    Actuality, ApiAccess, ApiAggregator, ApiBackend, ApiScope, EndpointMutability,
    ExtendApiBackend, FutureResult, NamedWith,
};

/// Type alias for the concrete `actix-web` HTTP response.
pub type FutureResponse = actix_web::FutureResponse<HttpResponse, actix_web::Error>;
/// Type alias for the concrete `actix-web` HTTP request.
pub type HttpRequest = actix_web::HttpRequest<()>;
/// Type alias for the inner `actix-web` HTTP requests handler.
pub type RawHandler = dyn Fn(HttpRequest) -> FutureResponse + 'static + Send + Sync;
/// Type alias for the `actix-web::App`.
pub type App = actix_web::App<()>;
/// Type alias for the `actix-web::App` configuration.
pub type AppConfig = Arc<dyn Fn(App) -> App + 'static + Send + Sync>;

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

    fn moved_permanently(
        &mut self,
        name: &'static str,
        new_location: &'static str,
        mutability: EndpointMutability,
    ) -> &mut Self {
        let handler = move |_request: HttpRequest| -> FutureResponse {
            let response = api::Error::MovedPermanently(new_location.to_owned()).into();
            let response_future = Err(response).into_future();

            Box::new(response_future)
        };

        self.mount_raw_handler(name, handler, mutability)
    }

    fn gone(&mut self, name: &'static str, mutability: EndpointMutability) -> &mut Self {
        let handler = move |_request: HttpRequest| -> FutureResponse {
            let response = api::Error::Gone.into();
            let response_future = Err(response).into_future();

            Box::new(response_future)
        };

        self.mount_raw_handler(name, handler, mutability)
    }

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

impl ApiBuilder {
    /// Mounts a given handler to the endpoint, either mutable or immutable.
    fn mount_raw_handler<F>(
        &mut self,
        name: &'static str,
        handler: F,
        mutability: EndpointMutability,
    ) -> &mut Self
    where
        F: Fn(HttpRequest) -> FutureResponse + Send + Sync + 'static,
    {
        use actix_web::http;

        let method = match mutability {
            EndpointMutability::Mutable => http::Method::POST,
            EndpointMutability::Immutable => http::Method::GET,
        };

        self.raw_handler(RequestHandler {
            name: name.to_owned(),
            method,
            inner: Arc::from(handler),
        });

        self
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

impl ResponseError for api::Error {
    fn error_response(&self) -> HttpResponse {
        match self {
            api::Error::BadRequest(err) => HttpResponse::BadRequest().body(err.to_string()),
            api::Error::InternalError(err) => {
                HttpResponse::InternalServerError().body(err.to_string())
            }
            api::Error::Io(err) => HttpResponse::InternalServerError().body(err.to_string()),
            api::Error::Storage(err) => HttpResponse::InternalServerError().body(err.to_string()),
            api::Error::Gone => HttpResponse::Gone().finish(),
            api::Error::MovedPermanently(new_location) => HttpResponse::MovedPermanently()
                .header(header::LOCATION, new_location.clone())
                .finish(),
            api::Error::NotFound(err) => HttpResponse::NotFound().body(err.to_string()),
            api::Error::Unauthorized => HttpResponse::Unauthorized().finish(),
        }
    }
}

/// Creates a `HttpResponse` object from the provided JSON value.
/// Depending on the `actuality` parameter value, the warning about endpoint
/// being deprecated can be added.
fn json_response<T: Serialize>(actuality: Actuality, json_value: T) -> HttpResponse {
    let mut response = HttpResponse::Ok();

    if let Actuality::Deprecated(ref discontinued_on) = actuality {
        // There is a proposal for creating special deprecation header within HTTP,
        // but currently it's only a draft. So the conventional way to notify API user
        // about endpoint deprecation is setting the `Warning` header.
        let expiration_note = match discontinued_on {
            Some(date) => format!(
                "The old API is maintained until {}.",
                date.format("%Y-%m-%d")
            ),
            None => "Currently there is no specific date for disabling this endpoint.".into(),
        };

        let warning_text = format!(
            "Deprecated API: This endpoint is deprecated, \
             see the documentation to find an alternative. \
             {}",
            expiration_note
        );

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
    const WARNING_NUMBER: u16 = 299;
    const WARNING_AGENT: &str = "-";

    format!("{} {} \"{}\"", WARNING_NUMBER, WARNING_AGENT, warning_text)
}

impl From<EndpointMutability> for actix_web::http::Method {
    fn from(mutability: EndpointMutability) -> Self {
        match mutability {
            EndpointMutability::Immutable => actix_web::http::Method::GET,
            EndpointMutability::Mutable => actix_web::http::Method::POST,
        }
    }
}

impl<Q, I, F> From<NamedWith<Q, I, api::Result<I>, F>> for RequestHandler
where
    F: Fn(Q) -> api::Result<I> + 'static + Send + Sync + Clone,
    Q: DeserializeOwned + 'static,
    I: Serialize + 'static,
{
    fn from(f: NamedWith<Q, I, api::Result<I>, F>) -> Self {
        let handler = f.inner.handler;
        let actuality = f.actuality;
        let mutability = f.mutability;
        let index = move |request: HttpRequest| -> FutureResponse {
            let handler = handler.clone();
            let actuality = actuality.clone();
            match mutability {
                EndpointMutability::Immutable => {
                    // For immutable requests, extract query from query string.
                    let future = Query::from_request(&request, &Default::default())
                        .map(Query::into_inner)
                        .and_then(|query| handler(query).map_err(From::from))
                        .and_then(|value| Ok(json_response(actuality, value)))
                        .into_future();
                    Box::new(future)
                }
                EndpointMutability::Mutable => {
                    // For mutable requests, extract query from the request body as JSON.
                    request
                        .json()
                        .from_err()
                        .and_then(move |query: Q| {
                            handler(query)
                                .map(|value| json_response(actuality, value))
                                .map_err(From::from)
                        })
                        .responder()
                }
            }
        };

        Self {
            name: f.name,
            method: f.mutability.into(),
            inner: Arc::from(index) as Arc<RawHandler>,
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
        let actuality = f.actuality;
        let mutability = f.mutability;
        let index = move |request: HttpRequest| -> FutureResponse {
            let handler = handler.clone();
            let actuality = actuality.clone();
            match mutability {
                EndpointMutability::Immutable => {
                    // For immutable requests, extract query from query string.
                    Query::from_request(&request, &Default::default())
                        .map(Query::into_inner)
                        .into_future()
                        .and_then(move |query| handler(query).map_err(From::from))
                        .map(|value| json_response(actuality, value))
                        .responder()
                }
                EndpointMutability::Mutable => {
                    // For mutable requests, extract query from the request body as JSON.
                    request
                        .json()
                        .from_err()
                        .and_then(move |query: Q| {
                            handler(query)
                                .map(|value| json_response(actuality, value))
                                .map_err(From::from)
                        })
                        .responder()
                }
            }
        };

        Self {
            name: f.name,
            method: f.mutability.into(),
            inner: Arc::from(index) as Arc<RawHandler>,
        }
    }
}

/// Creates `actix_web::App` for the given aggregator and runtime configuration.
pub(crate) fn create_app(aggregator: &ApiAggregator, runtime_config: ApiRuntimeConfig) -> App {
    let app_config = runtime_config.app_config;
    let access = runtime_config.access;
    let mut app = App::new();
    app = app.scope("api", |scope| aggregator.extend_backend(access, scope));
    if let Some(app_config) = app_config {
        app = app_config(app);
    }
    app
}

/// Configuration parameters for the `App` runtime.
#[derive(Clone)]
pub struct ApiRuntimeConfig {
    /// The socket address to bind.
    pub listen_address: SocketAddr,
    /// API access level.
    pub access: ApiAccess,
    /// Optional App configuration.
    pub app_config: Option<AppConfig>,
}

impl ApiRuntimeConfig {
    /// Creates API runtime configuration for the given address and access level.
    pub fn new(listen_address: SocketAddr, access: ApiAccess) -> Self {
        Self {
            listen_address,
            access,
            app_config: Default::default(),
        }
    }
}

impl fmt::Debug for ApiRuntimeConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApiRuntimeConfig")
            .field("listen_address", &self.listen_address)
            .field("access", &self.access)
            .field("app_config", &self.app_config.as_ref().map(drop))
            .finish()
    }
}

/// Configuration parameters for the actix system runtime.
#[derive(Debug, Clone)]
pub struct SystemRuntimeConfig {
    /// Active API runtimes.
    pub api_runtimes: Vec<ApiRuntimeConfig>,
    /// API aggregator.
    pub api_aggregator: ApiAggregator,
    /// The interval in milliseconds between attempts of restarting HTTP-server in case
    /// the server failed to restart
    pub server_restart_retry_timeout: u64,
    /// The attempts counts of restarting HTTP-server in case the server failed to restart
    pub server_restart_max_retries: u16,
}

/// Actix system runtime handle.
pub struct SystemRuntime {
    system_thread: JoinHandle<result::Result<(), Error>>,
    system: System,
}

impl SystemRuntimeConfig {
    /// Starts actix system runtime along with all web runtimes.
    pub fn start(
        self,
        endpoints_rx: mpsc::Receiver<UpdateEndpoints>,
    ) -> result::Result<SystemRuntime, Error> {
        // Creates a system thread.
        let (system_tx, system_rx) = mpsc::unbounded();
        let system_thread = thread::spawn(move || -> result::Result<(), Error> {
            let system = System::new("http-server");
            system_tx.unbounded_send(System::current())?;
            ApiManager::new(self, endpoints_rx).start();

            // Starts actix-web runtime.
            let code = system.run();
            trace!("Actix runtime finished with code {}", code);
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
}

impl SystemRuntime {
    /// Stops the actix system runtime along with all web runtimes.
    pub fn stop(self) -> result::Result<(), Error> {
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

/// CORS header specification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllowOrigin {
    /// Allows access from any host.
    Any,
    /// Allows access only from the specified hosts.
    Whitelist(Vec<String>),
}

impl ser::Serialize for AllowOrigin {
    fn serialize<S>(&self, serializer: S) -> result::Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        match *self {
            AllowOrigin::Any => "*".serialize(serializer),
            AllowOrigin::Whitelist(ref hosts) => {
                if hosts.len() == 1 {
                    hosts[0].serialize(serializer)
                } else {
                    hosts.serialize(serializer)
                }
            }
        }
    }
}

impl<'de> de::Deserialize<'de> for AllowOrigin {
    fn deserialize<D>(d: D) -> result::Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = AllowOrigin;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a list of hosts or \"*\"")
            }

            fn visit_str<E>(self, value: &str) -> result::Result<AllowOrigin, E>
            where
                E: de::Error,
            {
                match value {
                    "*" => Ok(AllowOrigin::Any),
                    _ => Ok(AllowOrigin::Whitelist(vec![value.to_string()])),
                }
            }

            fn visit_seq<A>(self, seq: A) -> result::Result<AllowOrigin, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let hosts =
                    de::Deserialize::deserialize(de::value::SeqAccessDeserializer::new(seq))?;
                Ok(AllowOrigin::Whitelist(hosts))
            }
        }

        d.deserialize_any(Visitor)
    }
}

impl FromStr for AllowOrigin {
    type Err = Error;

    fn from_str(s: &str) -> result::Result<Self, Self::Err> {
        if s == "*" {
            return Ok(AllowOrigin::Any);
        }

        let v: Vec<_> = s
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if v.is_empty() {
            bail!("Invalid AllowOrigin::Whitelist value");
        }

        Ok(AllowOrigin::Whitelist(v))
    }
}

impl<'a> From<&'a AllowOrigin> for Cors {
    fn from(origin: &'a AllowOrigin) -> Self {
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
    use super::*;

    #[test]
    fn allow_origin_from_str() {
        fn check(text: &str, expected: AllowOrigin) {
            let from_str = AllowOrigin::from_str(text).unwrap();
            assert_eq!(from_str, expected);
        }

        check(r#"*"#, AllowOrigin::Any);
        check(
            r#"http://example.com"#,
            AllowOrigin::Whitelist(vec!["http://example.com".to_string()]),
        );
        check(
            r#"http://a.org, http://b.org"#,
            AllowOrigin::Whitelist(vec!["http://a.org".to_string(), "http://b.org".to_string()]),
        );
        check(
            r#"http://a.org, http://b.org, "#,
            AllowOrigin::Whitelist(vec!["http://a.org".to_string(), "http://b.org".to_string()]),
        );
        check(
            r#"http://a.org,http://b.org"#,
            AllowOrigin::Whitelist(vec!["http://a.org".to_string(), "http://b.org".to_string()]),
        );
    }

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

        let deprecated_response_no_deadline = json_response(Actuality::Deprecated(None), 123);
        let expected_warning_text =
            "Deprecated API: This endpoint is deprecated, \
             see the documentation to find an alternative. \
             Currently there is no specific date for disabling this endpoint.";
        let expected_warning = create_warning_header(expected_warning_text);
        assert_responses_eq(
            deprecated_response_no_deadline,
            HttpResponse::Ok()
                .header(header::WARNING, expected_warning)
                .json(123),
        );

        let deadline = chrono::Utc.ymd(2020, 12, 31);

        let deprecated_response_deadline =
            json_response(Actuality::Deprecated(Some(deadline)), 123);
        let expected_warning_text = "Deprecated API: This endpoint is deprecated, \
                                     see the documentation to find an alternative. \
                                     The old API is maintained until 2020-12-31.";
        let expected_warning = create_warning_header(expected_warning_text);
        assert_responses_eq(
            deprecated_response_deadline,
            HttpResponse::Ok()
                .header(header::WARNING, expected_warning)
                .json(123),
        );
    }
}
