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

use actix::Actor;
use actix::{Addr, System};
pub use actix_cors::Cors;
use actix_web::{
    dev::{Body, ServiceRequest, ServiceResponse},
    error::ResponseError,
    web::{scope, Payload, Query},
    App, Error, FromRequest, HttpRequest, HttpResponse, Scope,
};
use futures::{Future, IntoFuture, Stream};
use serde::{
    de::{self, DeserializeOwned},
    ser, Serialize,
};

use std::{
    fmt,
    net::SocketAddr,
    result,
    str::FromStr,
    sync::{mpsc, Arc},
    thread::{self, JoinHandle},
};

use crate::api::{
    error::Error as ApiError,
    manager::{ApiManager, RestartServer},
    ApiAccess, ApiAggregator, ApiBackend, ApiScope, ExtendApiBackend, FutureResult, Immutable,
    Mutable, NamedWith, Result,
};

/// Type alias for the concrete `actix-web` HTTP response.
pub type FutureResponse = Box<dyn Future<Item = HttpResponse, Error = Error>>;
/// Type alias for the inner `actix-web` HTTP requests handler.
pub type RawHandler = dyn Fn(HttpRequest, Payload) -> FutureResponse + 'static + Send + Sync;

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
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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
    type Backend = Scope;

    fn raw_handler(&mut self, handler: Self::Handler) -> &mut Self {
        self.handlers.push(handler);
        self
    }

    fn wire(&self, mut output: Self::Backend) -> Self::Backend {
        for handler in self.handlers.clone() {
            let inner = handler.inner;
            output = match handler.method {
                actix_web::http::Method::GET => output.service(
                    actix_web::web::resource(&handler.name)
                        .route(actix_web::web::get().to(move |r, p| inner(r, p))),
                ),
                actix_web::http::Method::POST => output.service(
                    actix_web::web::resource(&handler.name)
                        .route(actix_web::web::post().to(move |r, p| inner(r, p))),
                ),
                _ => {
                    // TODO: log error. unreachable ?
                    output
                }
            };
        }
        output
    }
}

impl ExtendApiBackend for Scope {
    fn extend<'a, I>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = (&'a str, &'a ApiScope)>,
    {
        for item in items {
            self = self.service(item.1.actix_backend.wire(scope(&item.0)));
        }
        self
    }
}

impl ResponseError for ApiError {
    fn error_response(&self) -> HttpResponse {
        match self {
            ApiError::BadRequest(err) => HttpResponse::BadRequest().body(err.to_string()),
            ApiError::InternalError(err) => {
                HttpResponse::InternalServerError().body(err.to_string())
            }
            ApiError::Io(err) => HttpResponse::InternalServerError().body(err.to_string()),
            ApiError::Storage(err) => HttpResponse::InternalServerError().body(err.to_string()),
            ApiError::NotFound(err) => HttpResponse::NotFound().body(err.to_string()),
            ApiError::Unauthorized => HttpResponse::Unauthorized().finish(),
        }
    }
}

impl<Q, I, F> From<NamedWith<Q, I, Result<I>, F, Immutable>> for RequestHandler
where
    F: Fn(Q) -> Result<I> + 'static + Send + Sync + Clone,
    Q: DeserializeOwned + 'static,
    I: Serialize + 'static,
{
    fn from(f: NamedWith<Q, I, Result<I>, F, Immutable>) -> Self {
        let handler = f.inner.handler;
        let index = move |request: HttpRequest, mut _payload: Payload| -> FutureResponse {
            let mut dummy_payload = actix_web::dev::Payload::None;
            let future = Query::from_request(&request, &mut dummy_payload)
                .map(Query::into_inner)
                .and_then(|query| handler(query).map_err(From::from))
                .and_then(|value| Ok(HttpResponse::Ok().json(value)))
                .into_future();
            Box::new(future)
        };

        Self {
            name: f.name,
            method: actix_web::http::Method::GET,
            inner: Arc::from(index) as Arc<RawHandler>,
        }
    }
}

impl<Q, I, F> From<NamedWith<Q, I, Result<I>, F, Mutable>> for RequestHandler
where
    F: Fn(Q) -> Result<I> + 'static + Send + Sync + Clone,
    Q: DeserializeOwned + 'static,
    I: Serialize + 'static,
{
    fn from(f: NamedWith<Q, I, Result<I>, F, Mutable>) -> Self {
        let handler = f.inner.handler;
        let index = move |_request: HttpRequest, payload: Payload| -> FutureResponse {
            let handler = handler.clone();
            Box::new(
                payload
                    .concat2()
                    .from_err()
                    .and_then(move |body| {
                        handler(serde_json::from_slice::<Q>(&body)?)
                            .map(|value| HttpResponse::Ok().json(value))
                            .map_err(From::from)
                    })
                    .into_future(),
            )
        };

        Self {
            name: f.name,
            method: actix_web::http::Method::POST,
            inner: Arc::from(index) as Arc<RawHandler>,
        }
    }
}

impl<Q, I, F> From<NamedWith<Q, I, FutureResult<I>, F, Immutable>> for RequestHandler
where
    F: Fn(Q) -> FutureResult<I> + 'static + Clone + Send + Sync,
    Q: DeserializeOwned + 'static,
    I: Serialize + 'static,
{
    fn from(f: NamedWith<Q, I, FutureResult<I>, F, Immutable>) -> Self {
        let handler = f.inner.handler;
        let index = move |request: HttpRequest, mut _payload: Payload| -> FutureResponse {
            let handler = handler.clone();
            let mut dummy_payload = actix_web::dev::Payload::None;
            Box::new(
                Query::from_request(&request, &mut dummy_payload)
                    .map(Query::into_inner)
                    .into_future()
                    .and_then(move |query| handler(query).map_err(From::from))
                    .map(|value| HttpResponse::Ok().json(value)),
            )
        };

        Self {
            name: f.name,
            method: actix_web::http::Method::GET,
            inner: Arc::from(index) as Arc<RawHandler>,
        }
    }
}

impl<Q, I, F> From<NamedWith<Q, I, FutureResult<I>, F, Mutable>> for RequestHandler
where
    F: Fn(Q) -> FutureResult<I> + 'static + Clone + Send + Sync,
    Q: DeserializeOwned + 'static,
    I: Serialize + 'static,
{
    fn from(f: NamedWith<Q, I, FutureResult<I>, F, Mutable>) -> Self {
        let handler = f.inner.handler;
        let index = move |_request: HttpRequest, payload: Payload| -> FutureResponse {
            let handler = handler.clone();

            Box::new(
                payload
                    .concat2()
                    .from_err()
                    .and_then(move |body| {
                        handler(serde_json::from_slice::<Q>(&body).unwrap()) // TODO rid off unwrap
                            .map(|value| HttpResponse::Ok().json(value))
                            .map_err(From::from)
                    })
                    .into_future(),
            )
        };

        Self {
            name: f.name,
            method: actix_web::http::Method::POST,
            inner: Arc::from(index) as Arc<RawHandler>,
        }
    }
}

/// Creates `actix_web::App` for the given aggregator and runtime configuration.
pub(crate) fn create_app(
    aggregator: &ApiAggregator,
    runtime_config: ApiRuntimeConfig,
) -> App<
    impl actix_service::NewService<
        Config = (),
        Request = ServiceRequest,
        Response = ServiceResponse<Body>,
        Error = Error,
        InitError = (),
    >,
    Body,
> {
    let access = runtime_config.access;
    let cors = match runtime_config.allow_origin {
        Some(origin) => Cors::from(origin.clone()),
        None => Cors::new(),
    };

    App::new()
        .wrap(cors)
        .service(aggregator.extend_backend(access, actix_web::web::scope("api")))
}

/// Configuration parameters for the `App` runtime.
#[derive(Clone)]
pub struct ApiRuntimeConfig {
    /// The socket address to bind.
    pub listen_address: SocketAddr,
    /// API access level.
    pub access: ApiAccess,
    /// Optional allow origin configuration
    pub allow_origin: Option<AllowOrigin>,
}

impl ApiRuntimeConfig {
    /// Creates API runtime configuration for the given address and access level.
    pub fn new(listen_address: SocketAddr, access: ApiAccess) -> Self {
        Self {
            listen_address,
            access,
            allow_origin: Default::default(),
        }
    }
}

impl fmt::Debug for ApiRuntimeConfig {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ApiRuntimeConfig")
            .field("listen_address", &self.listen_address)
            .field("access", &self.access)
            .field("allow_origin", &self.allow_origin)
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
}

/// Actix system runtime handle.
pub struct SystemRuntime {
    system_thread: JoinHandle<result::Result<(), failure::Error>>,
    system: System,
    api_manager: Addr<ApiManager>,
}

impl SystemRuntimeConfig {
    /// Starts actix system runtime along with all web runtimes.
    pub fn start(self) -> result::Result<SystemRuntime, failure::Error> {
        SystemRuntime::new(self)
    }
}

impl SystemRuntime {
    pub(crate) fn new(config: SystemRuntimeConfig) -> result::Result<Self, failure::Error> {
        // Creates a system thread.
        let (system_tx, system_rx) = mpsc::channel();
        let (api_runtime_tx, api_runtime_rx) = mpsc::channel();
        let system_thread = thread::spawn(move || -> result::Result<(), failure::Error> {
            let system = System::new("http-server");

            let api_manager = ApiManager::new(config).start();

            // Sends addresses to the control thread.
            system_tx.send(System::current())?;
            api_runtime_tx.send(api_manager)?;
            // Starts actix-web runtime.
            let result = system.run();

            trace!("Actix runtime finished with result {:?}", result);
            if let Err(err) = result {
                error!("Actix runtime finished with error: {}", err)
            };
            Ok(())
        });
        // Receives addresses of runtime items.
        let system = system_rx
            .recv()
            .map_err(|_| format_err!("Unable to receive actix system handle"))?;
        let api_manager = api_runtime_rx
            .recv()
            .map_err(|e| format_err!("Unable to receive api manager address: {}", e))?;

        Ok(Self {
            system_thread,
            system,
            api_manager,
        })
    }

    /// Stops the actix system runtime along with all web runtimes.
    pub fn stop(self) -> result::Result<(), failure::Error> {
        // Stop actix system runtime.
        self.system.stop();
        self.system_thread.join().map_err(|e| {
            format_err!(
                "Unable to join actix web api thread, an error occurred: {:?}",
                e
            )
        })?
    }

    /// Restart actix web-api servers.
    pub fn restart(&self) {
        self.api_manager.do_send(RestartServer);
    }
}

impl fmt::Debug for SystemRuntime {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
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
    type Err = failure::Error;

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
            AllowOrigin::Any => Self::new(),
            AllowOrigin::Whitelist(ref hosts) => {
                let mut builder = Self::new();
                for host in hosts {
                    builder = builder.allowed_origin(host);
                }
                builder
            }
        }
    }
}

impl From<AllowOrigin> for Cors {
    fn from(origin: AllowOrigin) -> Self {
        Self::from(&origin)
    }
}

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
