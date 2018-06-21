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

use actix::{msgs::SystemExit, Addr, Arbiter, Syn, System};
use actix_web::{self,
                error::ResponseError,
                server::{HttpServer, IntoHttpHandler, StopServer},
                AsyncResponder,
                FromRequest,
                HttpMessage,
                HttpResponse,
                Query};
use failure;
use futures::{Future, IntoFuture};
use serde::de::DeserializeOwned;
use serde::Serialize;

use std::fmt;
use std::net::SocketAddr;
use std::result;
use std::sync::{mpsc, Arc};
use std::thread::{self, JoinHandle};

use api::error::Error as ApiError;
use api::{ApiAccess, ApiAggregator, FutureResult, Immutable, IntoApiBackend, Mutable, NamedWith,
          Result, ServiceApiBackend, ServiceApiScope, ServiceApiState};

/// Type alias for the concrete API http response.
pub type FutureResponse = actix_web::FutureResponse<HttpResponse, actix_web::Error>;
/// Type alias for the concrete API http request.
pub type HttpRequest = actix_web::HttpRequest<ServiceApiState>;
/// Type alias for the inner actix-web http requests handler.
pub type RawHandler = Fn(HttpRequest) -> FutureResponse + 'static + Send + Sync;
/// Type alias for the actix web App with the ServiceApiState.
pub type App = actix_web::App<ServiceApiState>;
/// Type alias for the actix web App configuration.
pub type AppConfig = Arc<Fn(App) -> App + 'static + Send + Sync>;

/// Type alias for the actix http server runtime address.
type HttpServerAddr = Addr<Syn, HttpServer<<App as IntoHttpHandler>::Handler>>;
/// Type alias for the actix system runtime address.
type SystemAddr = Addr<Syn, System>;

/// Raw actix-web backend requests handler.
#[derive(Clone)]
pub struct RequestHandler {
    /// Endpoint name.
    pub name: String,
    /// Endpoint http method.
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

/// API builder for the actix-web backend.
#[derive(Debug, Clone, Default)]
pub struct ApiBuilder {
    handlers: Vec<RequestHandler>,
}

impl ApiBuilder {
    /// Constructs a new backend builder instance.
    pub fn new() -> ApiBuilder {
        ApiBuilder::default()
    }
}

impl ServiceApiBackend for ApiBuilder {
    type Handler = RequestHandler;
    type Scope = actix_web::Scope<ServiceApiState>;

    fn raw_handler(&mut self, handler: Self::Handler) -> &mut Self {
        self.handlers.push(handler);
        self
    }

    fn wire(&self, mut output: Self::Scope) -> Self::Scope {
        for handler in self.handlers.clone() {
            let inner = handler.inner;
            output = output.route(&handler.name, handler.method.clone(), move |request| {
                inner(request)
            });
        }
        output
    }
}

impl IntoApiBackend for actix_web::Scope<ServiceApiState> {
    fn extend<'a, I>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = (&'a str, &'a ServiceApiScope)>,
    {
        for mut item in items {
            self = self.nested(&item.0, move |scope| item.1.actix_backend.wire(scope))
        }
        self
    }
}

impl ResponseError for ApiError {
    fn error_response(&self) -> HttpResponse {
        match self {
            ApiError::BadRequest(err) => HttpResponse::BadRequest().body(err),
            ApiError::InternalError(err) => {
                HttpResponse::InternalServerError().body(err.to_string())
            }
            ApiError::Io(err) => HttpResponse::InternalServerError().body(err.to_string()),
            ApiError::Storage(err) => HttpResponse::InternalServerError().body(err.to_string()),
            ApiError::NotFound(err) => HttpResponse::NotFound().body(err),
            ApiError::Unauthorized => HttpResponse::Unauthorized().finish(),
        }
    }
}

impl<Q, I, F> From<NamedWith<Q, I, Result<I>, F, Immutable>> for RequestHandler
where
    F: for<'r> Fn(&'r ServiceApiState, Q) -> Result<I> + 'static + Send + Sync + Clone,
    Q: DeserializeOwned + 'static,
    I: Serialize + 'static,
{
    fn from(f: NamedWith<Q, I, Result<I>, F, Immutable>) -> Self {
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

impl<Q, I, F> From<NamedWith<Q, I, Result<I>, F, Mutable>> for RequestHandler
where
    F: for<'r> Fn(&'r ServiceApiState, Q) -> Result<I> + 'static + Send + Sync + Clone,
    Q: DeserializeOwned + 'static,
    I: Serialize + 'static,
{
    fn from(f: NamedWith<Q, I, Result<I>, F, Mutable>) -> Self {
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

impl<Q, I, F> From<NamedWith<Q, I, FutureResult<I>, F, Immutable>> for RequestHandler
where
    F: for<'r> Fn(&'r ServiceApiState, Q) -> FutureResult<I> + 'static + Clone + Send + Sync,
    Q: DeserializeOwned + 'static,
    I: Serialize + 'static,
{
    fn from(f: NamedWith<Q, I, FutureResult<I>, F, Immutable>) -> Self {
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

impl<Q, I, F> From<NamedWith<Q, I, FutureResult<I>, F, Mutable>> for RequestHandler
where
    F: for<'r> Fn(&'r ServiceApiState, Q) -> FutureResult<I> + 'static + Clone + Send + Sync,
    Q: DeserializeOwned + 'static,
    I: Serialize + 'static,
{
    fn from(f: NamedWith<Q, I, FutureResult<I>, F, Mutable>) -> Self {
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

/// Creates `actix_web::App` for the given aggregator and runtime configuration.
pub(crate) fn create_app(aggregator: &ApiAggregator, runtime_config: ApiRuntimeConfig) -> App {
    let app_config = runtime_config.app_config;
    let access = runtime_config.access;
    let state = ServiceApiState::new(aggregator.blockchain.clone());
    let mut app = App::with_state(state).scope("api", |scope| {
        aggregator.extend_api(access, scope)
        });
    if let Some(app_config) = app_config {
        app = app_config(app);
    }
    app
}

/// Configuration parameters for the `App` runtime.
#[derive(Clone)]
pub(crate) struct ApiRuntimeConfig {
    /// The socket address to bind.
    pub listen_address: SocketAddr,
    /// Api access level.
    pub access: ApiAccess,
    /// Optional App configuration.
    pub app_config: Option<AppConfig>,
}

impl ApiRuntimeConfig {
    pub fn new(listen_address: SocketAddr, access: ApiAccess) -> ApiRuntimeConfig {
        ApiRuntimeConfig {
            listen_address,
            access,
            app_config: Default::default(),
        }
    }
}

impl fmt::Debug for ApiRuntimeConfig {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ApiRuntimeConfig")
            .field("listen_address", &self.listen_address)
            .field("access", &self.access)
            .field("app_config", &self.app_config.as_ref().map(drop))
            .finish()
    }
}

/// Configuration parameters for the actix runtime.
pub(crate) struct SystemRuntimeConfig {
    pub api_runtimes: Vec<ApiRuntimeConfig>,
    pub api_aggregator: ApiAggregator,
}

/// Actix system runtime handle.
pub(crate) struct SystemRuntime {
    system_thread: JoinHandle<result::Result<(), failure::Error>>,
    api_runtime_addresses: Vec<HttpServerAddr>,
    system_address: SystemAddr,
}

impl SystemRuntimeConfig {
    pub fn start(self) -> result::Result<SystemRuntime, failure::Error> {
        SystemRuntime::new(self)
    }
}

impl SystemRuntime {
    fn new(config: SystemRuntimeConfig) -> result::Result<SystemRuntime, failure::Error> {
        // Creates system thread.
        let (system_address_tx, system_address_rx) = mpsc::channel();
        let (api_runtime_tx, api_runtime_rx) = mpsc::channel();
        let api_runtimes = config.api_runtimes.clone();
        let system_thread = thread::spawn(move || -> result::Result<(), failure::Error> {
            let system = System::new("http-server");

            let aggregator = config.api_aggregator.clone();
            trace!("Create actix system runtime with api: {:#?}", aggregator.inner);
            let api_handlers = config.api_runtimes.into_iter().map(|runtime_config| {
                let access = runtime_config.access;
                let listen_address = runtime_config.listen_address;
                info!("Starting web {} api on {}", access, listen_address);

                let aggregator = aggregator.clone();
                HttpServer::new(move || create_app(&aggregator, runtime_config.clone()))
                    .disable_signals()
                    .bind(listen_address)
                    .map(|server| server.start())
            });
            // Sends addresses to the control thread.
            system_address_tx.send(Arbiter::system())?;
            for api_handler in api_handlers {
                api_runtime_tx.send(api_handler?)?;
            }
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
        let system_address = system_address_rx
            .recv()
            .map_err(|_| format_err!("Unable to receive actix system address"))?;
        let api_runtime_addresses = {
            let mut api_runtime_addresses = Vec::new();
            for api_runtime in api_runtimes {
                let api_runtime_address = api_runtime_rx.recv().map_err(|_| {
                    format_err!(
                        "Unable to receive actix api system address for api: listen_addr {}, scope {}",
                        api_runtime.listen_address,
                        api_runtime.access,
                    )
                })?;
                api_runtime_addresses.push(api_runtime_address);
            }
            api_runtime_addresses
        };

        Ok(SystemRuntime {
            system_thread,
            system_address,
            api_runtime_addresses,
        })
    }

    pub fn stop(self) -> result::Result<(), failure::Error> {
        // Stop all actix web servers.
        for api_runtime_address in self.api_runtime_addresses {
            api_runtime_address
                .send(StopServer { graceful: true })
                .wait()?
                .map_err(|_| {
                    format_err!("Unable to send `StopServer` message to web api handler")
                })?;
        }
        self.system_address.send(SystemExit(0)).wait()?;
        // Stop actix system runtime.
        self.system_thread.join().map_err(|e| {
            format_err!(
                "Unable to join actix web api thread, an error occurred: {:?}",
                e
            )
        })?
    }
}
