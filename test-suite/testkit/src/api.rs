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

#![allow(clippy::needless_doctest_main)] // false positive

pub use exonum_api::ApiAccess;

use actix_web::{
    test::{self, TestServer},
    web, App,
};
use exonum::{
    blockchain::ApiSender,
    messages::{AnyTx, Verified},
};
use exonum_api::{self as api, ApiAggregator};
use exonum_proto::ProtobufConvert;
use reqwest::{
    redirect::Policy as RedirectPolicy, Client, ClientBuilder, RequestBuilder as ReqwestBuilder,
    Response, StatusCode,
};
use serde::{de::DeserializeOwned, Serialize};

use std::{
    collections::HashMap,
    fmt::{self, Display},
};

use crate::TestKit;

/// Kind of public or private REST API of an Exonum node.
///
/// `ApiKind` allows to use `get*` and `post*` methods of [`TestKitApi`] more safely.
///
/// [`TestKitApi`]: struct.TestKitApi.html
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
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
            Self::System => write!(formatter, "api/system"),
            Self::Explorer => write!(formatter, "api/explorer"),
            Self::RustRuntime => write!(formatter, "api/runtimes/rust"),
            Self::Service(name) => write!(formatter, "api/services/{}", name),
        }
    }
}

/// API encapsulation for the testkit. Allows to execute and asynchronously retrieve results
/// for REST-ful endpoints of services.
///
/// Note that `TestKitApi` instantiation spawns a new HTTP server. Hence, it is advised to reuse
/// existing instances unless it is impossible. The latter may be the case if changes
/// to the testkit modify the set of its HTTP endpoints, for example, if a new service is
/// instantiated.
///
/// The HTTP server uses `actix` under the hood, so in order to execute asynchronous methods,
/// the user should use this API inside the `actix_rt` or `tokio` runtime.
/// The easiest way to do that is to use `#[tokio::test]` or `#[actix_rt::test]` instead of
/// `#[test]`.
///
/// # Examples
///
/// ```
/// # use exonum_testkit::{ApiKind, TestKitBuilder};
/// use exonum_rust_runtime::{ProtoSourcesQuery, ProtoSourceFile};
///
/// #[tokio::test]
/// async fn test_api() {
/// # // Trick to execute the test code rather than dropping it; the content of `#[tokio::test]`s
/// # // is dropped in doc tests.
/// # }
/// # #[tokio::main]
/// # async fn main() {
///     let mut testkit = TestKitBuilder::validator().build();
///     let api = testkit.api();
///     // Without services / plugins, API only contains endpoints
///     // provided by the Rust runtime.
///     let proto_sources: Vec<ProtoSourceFile> = api
///         .public(ApiKind::RustRuntime)
///         .query(&ProtoSourcesQuery::Core)
///         .get("proto-sources")
///         .await
///         .expect("Request to the valid endpoint failed");
/// }
/// ```
pub struct TestKitApi {
    _test_server_handle: TestServer,
    test_client: TestKitApiClient,
    api_sender: ApiSender,
}

impl fmt::Debug for TestKitApi {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        formatter.debug_struct("TestKitApi").finish()
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
        let inner = ClientBuilder::new()
            .redirect(RedirectPolicy::none())
            .build()
            .unwrap();

        let test_server = create_test_server(aggregator);
        let test_client = TestKitApiClient {
            test_server_url: test_server.url(""),
            inner,
        };

        Self {
            _test_server_handle: test_server,
            test_client,
            api_sender,
        }
    }

    /// Sends a transaction to the node.
    pub async fn send<T>(&self, transaction: T)
    where
        T: Into<Verified<AnyTx>>,
    {
        self.api_sender
            .broadcast_transaction(transaction.into())
            .await
            .expect("Cannot broadcast transaction");
    }

    /// Returns the resolved URL for the public API.
    pub fn public_url(&self, url: &str) -> String {
        self.test_client.public_url(url)
    }

    /// Returns the resolved URL for the private API.
    pub fn private_url(&self, url: &str) -> String {
        self.test_client.private_url(url)
    }

    /// Creates a requests builder for the public API scope.
    pub fn public(&self, kind: impl Display) -> RequestBuilder<'_, '_> {
        self.test_client.public(kind)
    }

    /// Creates a requests builder for the private API scope.
    pub fn private(&self, kind: impl Display) -> RequestBuilder<'_, '_> {
        self.test_client.private(kind)
    }

    /// Return reference to the underlying API client.
    pub fn client(&self) -> &TestKitApiClient {
        &self.test_client
    }
}

/// An asynchronous API client to make Requests with.
///
/// This client is a wrapper around the `reqwest::Client` to provide more convenient
/// way to test API.
#[derive(Debug, Clone)]
pub struct TestKitApiClient {
    test_server_url: String,
    inner: Client,
}

impl TestKitApiClient {
    /// Returns the resolved URL for the public API.
    pub fn public_url(&self, url: &str) -> String {
        [&self.test_server_url, "public/", url].concat()
    }

    /// Returns the resolved URL for the private API.
    pub fn private_url(&self, url: &str) -> String {
        [&self.test_server_url, "private/", url].concat()
    }

    /// Creates a request builder for the public API scope.
    pub fn public(&self, kind: impl Display) -> RequestBuilder<'_, '_> {
        RequestBuilder::new(
            &self.test_server_url,
            &self.inner,
            ApiAccess::Public,
            kind.to_string(),
        )
    }

    /// Creates a requests builder for the private API scope.
    pub fn private(&self, kind: impl Display) -> RequestBuilder<'_, '_> {
        RequestBuilder::new(
            &self.test_server_url,
            &self.inner,
            ApiAccess::Private,
            kind.to_string(),
        )
    }

    /// Return reference to the inner Reqwest client.
    pub fn inner(&self) -> &Client {
        &self.inner
    }
}

type ReqwestModifier<'b> = Box<dyn FnOnce(ReqwestBuilder) -> ReqwestBuilder + Send + 'b>;

/// An HTTP requests builder. This type can be used to send requests to
/// the appropriate `TestKitApi` handlers.
pub struct RequestBuilder<'a, 'b, Q = ()> {
    test_server_url: &'a str,
    test_client: &'a Client,
    access: ApiAccess,
    prefix: String,
    query: Option<&'b Q>,
    modifier: Option<ReqwestModifier<'b>>,
    expected_headers: HashMap<String, String>,
}

impl<'a, 'b, Q> fmt::Debug for RequestBuilder<'a, 'b, Q>
where
    Q: 'b + fmt::Debug,
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
    Q: 'b,
{
    fn new(
        test_server_url: &'a str,
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
        F: FnOnce(ReqwestBuilder) -> ReqwestBuilder + Send + 'b,
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

    // Checks that response contains headers expected by the request author.
    fn verify_headers(expected_headers: &HashMap<String, String>, response: &Response) {
        let headers = response.headers();
        for (header, expected_value) in expected_headers {
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
    async fn response_to_api_result<R>(response: Response) -> api::Result<R>
    where
        R: DeserializeOwned + 'static,
    {
        let code = response.status();
        let body = response.text().await.expect("Unable to get response text");
        log::trace!("Body: {}", body);
        if code == StatusCode::OK {
            let value = serde_json::from_str(&body).expect("Unable to deserialize body");
            Ok(value)
        } else {
            let error = api::Error::parse(code, &body).expect("Unable to deserialize API error");
            Err(error)
        }
    }
}

impl<Q> RequestBuilder<'_, '_, Q>
where
    Q: Serialize,
{
    /// Sends a GET request to the testing API endpoint and decodes response as
    /// the corresponding type.
    ///
    /// If query was specified, it is serialized as a query string parameters.
    pub async fn get<R>(self, endpoint: &str) -> api::Result<R>
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

        log::trace!("GET {}", url);

        let mut builder = self.test_client.get(&url);
        if let Some(modifier) = self.modifier {
            builder = modifier(builder);
        }
        let response = builder.send().await.expect("Unable to send request");
        Self::verify_headers(&self.expected_headers, &response);
        Self::response_to_api_result(response).await
    }

    /// Sends a POST request to the testing API endpoint and decodes response as
    /// the corresponding type.
    ///
    /// If query was specified, it is serialized as a JSON in the request body.
    pub async fn post<R>(self, endpoint: &str) -> api::Result<R>
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

        log::trace!("POST {}", url);

        let builder = self.test_client.post(&url);
        let mut builder = if let Some(query) = self.query.as_ref() {
            builder.json(query)
        } else {
            builder.json(&serde_json::Value::Null)
        };
        if let Some(modifier) = self.modifier {
            builder = modifier(builder);
        }
        let response = builder.send().await.expect("Unable to send request");
        Self::verify_headers(&self.expected_headers, &response);
        Self::response_to_api_result(response).await
    }
}

impl<Q> RequestBuilder<'_, '_, Q>
where
    Q: ProtobufConvert,
    Q::ProtoStruct: protobuf::Message,
{
    /// Sends a Protobuf-encoded POST request to the testing API endpoint and decodes response as
    /// the corresponding type.
    ///
    /// The query is serialized as Protobuf using `Content-Type: application/octet-stream`.
    /// If the query was not specified, an empty buffer is used.
    pub async fn post_pb<R>(self, endpoint: &str) -> api::Result<R>
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

        log::trace!("POST Protobuf {}", url);

        let body = self
            .query
            .map(|query| {
                let message = query.to_pb();
                protobuf::Message::write_to_bytes(&message)
                    .expect("Cannot write Protobuf message to `Vec<u8>`")
            })
            .unwrap_or_default();
        let mut builder = self
            .test_client
            .post(&url)
            .header("Content-Type", "application/octet-stream")
            .body(body);
        if let Some(modifier) = self.modifier {
            builder = modifier(builder);
        }

        let response = builder.send().await.expect("Unable to send request");
        Self::verify_headers(&self.expected_headers, &response);
        Self::response_to_api_result(response).await
    }
}

/// Create a test server.
fn create_test_server(aggregator: ApiAggregator) -> TestServer {
    let server = test::start(move || {
        let public_apis = aggregator.extend_backend(ApiAccess::Public, web::scope("public/api"));
        let private_apis = aggregator.extend_backend(ApiAccess::Private, web::scope("private/api"));
        App::new().service(public_apis).service(private_apis)
    });

    log::info!("Test server created on {}", server.addr());
    server
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TestKitBuilder;

    fn assert_send<T: Send>(_object: &T) {}

    #[test]
    fn assert_send_for_testkit_api() {
        let mut testkit = TestKitBuilder::validator().build();
        let api = testkit.api();
        assert_send(&api.public(ApiKind::Explorer).get::<()>("v1/transactions"));
        assert_send(&api.public(ApiKind::Explorer).post::<()>("v1/transactions"));
    }

    #[test]
    fn assert_send_for_testkit_client() {
        let api = TestKitBuilder::validator().build().api();
        let client = api.client().clone();
        assert_send(&client.public(ApiKind::Explorer).get::<()>("ping"));
    }
}
