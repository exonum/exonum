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

//! API encapsulation for the testkit.

pub use exonum_api::ApiAccess;

use actix::{Addr, System};
use actix_net::server::{Server, StopServer};
use actix_web::{
    server::{HttpServer, IntoHttpHandler},
    App,
};
use exonum::{
    blockchain::ApiSender,
    messages::{AnyTx, Verified},
};
use exonum_api::{self as api, ApiAggregator};
use futures::Future;
use log::{info, trace};
use reqwest::{
    Client, ClientBuilder, RedirectPolicy, RequestBuilder as ReqwestBuilder, Response, StatusCode,
};
use serde::{de::DeserializeOwned, Serialize};

use std::{
    collections::HashMap,
    fmt::{self, Display},
    net,
    sync::mpsc,
    thread::{self, JoinHandle},
};

use crate::TestKit;

/// Kind of public or private REST API of an Exonum node.
///
/// `ApiKind` allows to use `get*` and `post*` methods of [`TestKitApi`] more safely.
///
/// [`TestKitApi`]: struct.TestKitApi.html
#[derive(Debug, Clone, Copy)]
pub enum ApiKind {
    /// `api/system` endpoints of the system API node plugin. To access endpoints, the plugin
    /// must be attached to the testkit.
    System,
    /// Endpoints of the REST API of the explorer service. The service must be included
    /// to the testkit in order for endpoints to work.
    Explorer,
    /// `api/runtimes/rust` endpoints corresponding to Rust runtime of the Exonum REST API.
    RustRuntime,
    /// Endpoints corresponding to a service with the specified string identifier.
    Service(&'static str),
}

impl fmt::Display for ApiKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiKind::System => write!(formatter, "api/system"),
            ApiKind::Explorer => write!(formatter, "api/explorer"),
            ApiKind::RustRuntime => write!(formatter, "api/runtimes/rust"),
            ApiKind::Service(name) => write!(formatter, "api/services/{}", name),
        }
    }
}

/// API encapsulation for the testkit. Allows to execute and synchronously retrieve results
/// for REST-ful endpoints of services.
///
/// Note that `TestKitApi` instantiation spawns a new HTTP server. Hence, it is advised to reuse
/// existing instances unless it is impossible. The latter may be the case if changes
/// to the testkit modify the set of its HTTP endpoints, for example, if a new service is
/// instantiated.
pub struct TestKitApi {
    test_server: TestServer,
    test_client: Client,
    api_sender: ApiSender,
}

impl fmt::Debug for TestKitApi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.debug_struct("TestKitApi").finish()
    }
}

impl TestKitApi {
    /// Creates a new instance of API.
    pub fn new(testkit: &mut TestKit) -> Self {
        Self::from_raw_parts(testkit.update_aggregator(), testkit.api_sender.clone())
    }

    pub(crate) fn from_raw_parts(aggregator: ApiAggregator, api_sender: ApiSender) -> Self {
        // Testkit is intended for manual testing, so we don't want `reqwest` to handle redirects
        // automatically.
        let test_client = ClientBuilder::new()
            .redirect(RedirectPolicy::none())
            .build()
            .unwrap();
        TestKitApi {
            test_server: create_test_server(aggregator),
            test_client,
            api_sender,
        }
    }

    /// Returns the resolved URL for the public API.
    pub fn public_url(&self, url: &str) -> String {
        self.test_server.url(&format!("public/{}", url))
    }

    /// Sends a transaction to the node.
    pub fn send<T>(&self, transaction: T)
    where
        T: Into<Verified<AnyTx>>,
    {
        self.api_sender
            .broadcast_transaction(transaction.into())
            .wait()
            .expect("Cannot broadcast transaction");
    }

    /// Creates a requests builder for the public API scope.
    pub fn public(&self, kind: impl Display) -> RequestBuilder<'_, '_> {
        RequestBuilder::new(
            self.test_server.url(""),
            &self.test_client,
            ApiAccess::Public,
            kind.to_string(),
        )
    }

    /// Creates a requests builder for the private API scope.
    pub fn private(&self, kind: impl Display) -> RequestBuilder<'_, '_> {
        RequestBuilder::new(
            self.test_server.url(""),
            &self.test_client,
            ApiAccess::Private,
            kind.to_string(),
        )
    }
}

type ReqwestModifier<'b> = Box<dyn FnOnce(ReqwestBuilder) -> ReqwestBuilder + 'b>;

/// An HTTP requests builder. This type can be used to send requests to
/// the appropriate `TestKitApi` handlers.
pub struct RequestBuilder<'a, 'b, Q = ()> {
    test_server_url: String,
    test_client: &'a Client,
    access: ApiAccess,
    prefix: String,
    query: Option<&'b Q>,
    modifier: Option<ReqwestModifier<'b>>,
    expected_headers: HashMap<String, String>,
}

impl<'a, 'b, Q> fmt::Debug for RequestBuilder<'a, 'b, Q>
where
    Q: 'b + fmt::Debug + Serialize,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.debug_struct("RequestBuilder")
            .field("access", &self.access)
            .field("prefix", &self.prefix)
            .field("query", &self.query)
            .finish()
    }
}

impl<'a, 'b, Q> RequestBuilder<'a, 'b, Q>
where
    Q: 'b + Serialize,
{
    fn new(
        test_server_url: String,
        test_client: &'a Client,
        access: ApiAccess,
        prefix: String,
    ) -> Self {
        RequestBuilder {
            test_server_url,
            test_client,
            access,
            prefix,
            query: None,
            modifier: None,
            expected_headers: HashMap::new(),
        }
    }

    /// Sets a request data of the current request.
    ///
    /// For `GET` requests, it will be serialized as a query string parameters,
    /// and for `POST` requests, it will be serialized as a JSON in the request body.
    pub fn query<T>(self, query: &'b T) -> RequestBuilder<'a, 'b, T> {
        RequestBuilder {
            test_server_url: self.test_server_url,
            test_client: self.test_client,
            access: self.access,
            prefix: self.prefix,
            query: Some(query),
            modifier: self.modifier,
            expected_headers: self.expected_headers,
        }
    }

    /// Allows to modify a request before sending it by executing a provided closure.
    pub fn with<F>(self, f: F) -> Self
    where
        F: Fn(ReqwestBuilder) -> ReqwestBuilder + 'b,
    {
        Self {
            modifier: Some(Box::new(f)),
            ..self
        }
    }

    /// Allows to check that response will contain a specific header.
    pub fn expect_header(self, header: &str, value: &str) -> Self {
        let mut expected_headers = self.expected_headers;
        expected_headers.insert(header.into(), value.into());
        Self {
            expected_headers,
            ..self
        }
    }

    /// Sends a get request to the testing API endpoint and decodes response as
    /// the corresponding type.
    ///
    /// If query was specified, it is serialized as a query string parameters.
    pub fn get<R>(self, endpoint: &str) -> api::Result<R>
    where
        R: DeserializeOwned + 'static,
    {
        let params = self
            .query
            .as_ref()
            .map(|query| {
                format!(
                    "?{}",
                    serde_urlencoded::to_string(query).expect("Unable to serialize query.")
                )
            })
            .unwrap_or_default();
        let url = format!(
            "{url}{access}/{prefix}/{endpoint}{query}",
            url = self.test_server_url,
            access = self.access,
            prefix = self.prefix,
            endpoint = endpoint,
            query = params
        );

        trace!("GET {}", url);

        let mut builder = self.test_client.get(&url);
        if let Some(modifier) = self.modifier {
            builder = modifier(builder);
        }
        let response = builder.send().expect("Unable to send request");
        Self::verify_headers(self.expected_headers, &response);
        Self::response_to_api_result(response)
    }

    /// Sends a post request to the testing API endpoint and decodes response as
    /// the corresponding type.
    ///
    /// If query was specified, it is serialized as a JSON in the request body.
    pub fn post<R>(self, endpoint: &str) -> api::Result<R>
    where
        R: DeserializeOwned + 'static,
    {
        let url = format!(
            "{url}{access}/{prefix}/{endpoint}",
            url = self.test_server_url,
            access = self.access,
            prefix = self.prefix,
            endpoint = endpoint
        );

        trace!("POST {}", url);

        let builder = self.test_client.post(&url);
        let mut builder = if let Some(ref query) = self.query.as_ref() {
            trace!("Body: {}", serde_json::to_string_pretty(&query).unwrap());
            builder.json(query)
        } else {
            builder.json(&serde_json::Value::Null)
        };
        if let Some(modifier) = self.modifier {
            builder = modifier(builder);
        }
        let response = builder.send().expect("Unable to send request");
        Self::verify_headers(self.expected_headers, &response);
        Self::response_to_api_result(response)
    }

    // Checks that response contains headers expected by the request author.
    fn verify_headers(expected_headers: HashMap<String, String>, response: &Response) {
        let headers = response.headers();
        for (header, expected_value) in expected_headers.iter() {
            let header_value = headers.get(header).unwrap_or_else(|| {
                panic!(
                    "Response {:?} was expected to have header {}, but it isn't present",
                    response, header
                );
            });

            assert_eq!(
                header_value, expected_value,
                "Unexpected value of response header {}",
                header
            );
        }
    }

    /// Converts reqwest Response to `api::ApiResult`.
    fn response_to_api_result<R>(mut response: Response) -> api::Result<R>
    where
        R: DeserializeOwned + 'static,
    {
        let code = response.status();
        let body = response.text().expect("Unable to get response text");
        trace!("Body: {}", body);
        if code == StatusCode::OK {
            let value = serde_json::from_str(&body).expect("Unable to deserialize body");
            Ok(value)
        } else {
            let error = api::Error::parse(code, &body).expect("Unable to deserialize API error");
            Err(error)
        }
    }
}

/// Create a test server.
fn create_test_server(aggregator: ApiAggregator) -> TestServer {
    let server = TestServer::with_factory(move || {
        App::new()
            .scope("public/api", |scope| {
                trace!("Create public/api");
                aggregator.extend_backend(ApiAccess::Public, scope)
            })
            .scope("private/api", |scope| {
                trace!("Create private/api");
                aggregator.extend_backend(ApiAccess::Private, scope)
            })
    });

    info!("Test server created on {}", server.addr());
    server
}

/// The custom implementation of the test server, because there is an error in the default
/// implementation. It does not wait for the http server thread to complete during drop.
struct TestServer {
    addr: net::SocketAddr,
    backend: Addr<Server>,
    system: System,
    handle: Option<JoinHandle<()>>,
}

impl TestServer {
    /// Start new test server with application factory
    fn with_factory<F, H>(factory: F) -> Self
    where
        F: Fn() -> H + Send + Clone + 'static,
        H: IntoHttpHandler + 'static,
    {
        let (tx, rx) = mpsc::channel();

        // run server in separate thread
        let handle = thread::spawn(move || {
            let sys = System::new("actix-test-server");
            let tcp = net::TcpListener::bind("127.0.0.1:0").unwrap();
            let local_addr = tcp.local_addr().unwrap();

            let srv = HttpServer::new(factory)
                .disable_signals()
                .listen(tcp)
                .keep_alive(5)
                .workers(1)
                .start();

            tx.send((System::current(), local_addr, srv)).unwrap();
            sys.run();
        });

        let (system, addr, backend) = rx.recv().unwrap();

        Self {
            addr,
            backend,
            handle: Some(handle),
            system,
        }
    }

    /// Construct test server url.
    fn url(&self, uri: &str) -> String {
        if uri.starts_with('/') {
            format!("http://localhost:{}{}", self.addr.port(), uri)
        } else {
            format!("http://localhost:{}/{}", self.addr.port(), uri)
        }
    }

    /// Construct test server url.
    fn addr(&self) -> net::SocketAddr {
        self.addr
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        // Stop http server gracefully.
        let _ = self.backend.send(StopServer { graceful: true }).wait();
        self.system.stop();
        // Wait server thread.
        let _ = self.handle.take().unwrap().join();
    }
}
