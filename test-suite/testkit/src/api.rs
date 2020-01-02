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

pub use exonum::api::ApiAccess;

use actix_web::{test::TestServer, App};
use failure::format_err;
use log::{info, trace};
use reqwest::{
    header, Client, ClientBuilder, RedirectPolicy, RequestBuilder as ReqwestBuilder, Response,
    StatusCode,
};
use serde::{de::DeserializeOwned, Serialize};

use std::{
    collections::HashMap,
    fmt::{self, Display},
};

use exonum::{
    api::{self, node::public::system::DispatcherInfo, ApiAggregator},
    messages::{AnyTx, Verified},
    node::ApiSender,
};

use crate::TestKit;

/// Kind of public or private REST API of an Exonum node.
///
/// `ApiKind` allows to use `get*` and `post*` methods of [`TestKitApi`] more safely.
///
/// [`TestKitApi`]: struct.TestKitApi.html
#[derive(Debug, Clone, Copy)]
pub enum ApiKind {
    /// `api/system` endpoints of the built-in Exonum REST API.
    System,
    /// Endpoints of the REST API of the explorer service.
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

    /// Sends a transaction to the node via `ApiSender`.
    pub fn send<T>(&self, transaction: T)
    where
        T: Into<Verified<AnyTx>>,
    {
        self.api_sender
            .broadcast_transaction(transaction.into())
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

    /// Performs a GET request to the "/services" system endpoint.
    pub fn dispatcher_info(&self) -> DispatcherInfo {
        self.public(ApiKind::System).get("v1/services").unwrap()
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

    /// Sets a query data of the current request.
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

    /// Converts reqwest Response to api::Result.
    fn response_to_api_result<R>(mut response: Response) -> api::Result<R>
    where
        R: DeserializeOwned + 'static,
    {
        trace!("Response status: {}", response.status());

        fn extract_description(body: &str) -> Option<String> {
            trace!("Error: {}", body);
            match serde_json::from_str::<serde_json::Value>(body).ok()? {
                serde_json::Value::Object(ref object) if object.contains_key("description") => {
                    Some(object["description"].as_str()?.to_owned())
                }
                serde_json::Value::String(string) => Some(string),
                _ => None,
            }
        }

        fn error(mut response: Response) -> String {
            let body = response.text().expect("Unable to get response text");
            extract_description(&body).unwrap_or(body)
        }

        let error = match response.status() {
            StatusCode::OK => {
                let body = response.text().expect("Unable to get response text");
                trace!("Body: {}", body);
                let value = serde_json::from_str(&body).expect("Unable to deserialize body");

                return Ok(value);
            }
            StatusCode::FORBIDDEN | StatusCode::UNAUTHORIZED => api::Error::Unauthorized,
            StatusCode::BAD_REQUEST => api::Error::BadRequest(error(response)),
            StatusCode::NOT_FOUND => api::Error::NotFound(error(response)),
            StatusCode::MOVED_PERMANENTLY => {
                let location = response
                    .headers()
                    .get(header::LOCATION)
                    .expect("Received a MOVED_PERMANENTLY response without location header")
                    .to_str()
                    .unwrap()
                    .to_owned();
                api::Error::MovedPermanently(location)
            }
            StatusCode::GONE => api::Error::Gone,
            s if s.is_server_error() => {
                api::Error::InternalError(format_err!("{}", error(response)))
            }
            s => panic!("Received non-error response status: {}", s.as_u16()),
        };

        Err(error)
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

// FIXME: move to explorer service
/*
/// A convenience wrapper for Exonum node API to reduce the boilerplate code.
#[derive(Debug)]
pub struct ExonumNodeApi<'a> {
    pub inner: &'a TestKitApi,
}

impl<'a> ExonumNodeApi<'a> {
    pub fn new(api: &'a TestKitApi) -> Self {
        Self { inner: api }
    }

    /// Asserts that the transaction with the given hash has a specified status.
    pub fn assert_tx_status(&self, tx_hash: Hash, expected_status: Result<(), &ErrorMatch>) {
        let info: serde_json::Value = self
            .inner
            .public(ApiKind::Explorer)
            .query(&TransactionQuery::new(tx_hash))
            .get("v1/transactions")
            .unwrap();
        if let serde_json::Value::Object(info) = info {
            let tx_status_raw = info.get("status").unwrap().clone();
            let tx_status: ExecutionStatus = serde_json::from_value(tx_status_raw).unwrap();
            match expected_status {
                Ok(()) => tx_status.0.expect("Expected successful execution"),
                Err(e) => assert_eq!(*e, tx_status.0.expect_err("Expected execution error")),
            }
        } else {
            panic!("Invalid transaction info format, object expected");
        }
    }

    /// Asserts that the transaction with the given hash was executed successfully.
    pub fn assert_tx_success(&self, tx_hash: Hash) {
        self.assert_tx_status(tx_hash, Ok(()));
    }

    /// Same as `assert_tx_success`, but for a sequence of transactions.
    pub fn assert_txs_success(&self, tx_hashes: &[Hash]) {
        for &tx_hash in tx_hashes {
            self.assert_tx_success(tx_hash);
        }
    }
}
*/
