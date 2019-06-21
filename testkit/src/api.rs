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

//! API encapsulation for the testkit.

pub use exonum::api::ApiAccess;

use actix_web::{test::TestServer, App};
use reqwest::{Client, Response, StatusCode, RequestBuilder as ReqwestBuilder};
use serde::{de::DeserializeOwned, Serialize};

use std::fmt::{self, Display};

use exonum::{
    api::{self, ApiAggregator, ServiceApiState},
    blockchain::SharedNodeState,
    messages::{RawTransaction, Signed},
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
    /// `api/explorer` endpoints of the built-in Exonum REST API.
    Explorer,
    /// Endpoints corresponding to a service with the specified string identifier.
    Service(&'static str),
}

impl fmt::Display for ApiKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ApiKind::System => write!(f, "api/system"),
            ApiKind::Explorer => write!(f, "api/explorer"),
            ApiKind::Service(name) => write!(f, "api/services/{}", name),
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
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.debug_struct("TestKitApi").finish()
    }
}

impl TestKitApi {
    /// Creates a new instance of API.
    pub fn new(testkit: &TestKit) -> Self {
        Self::from_raw_parts(
            ApiAggregator::new(testkit.blockchain().clone(), SharedNodeState::new(10_000)),
            testkit.api_sender.clone(),
        )
    }

    pub(crate) fn from_raw_parts(aggregator: ApiAggregator, api_sender: ApiSender) -> Self {
        trace!("Created testkit api: {:#?}", aggregator);

        TestKitApi {
            test_server: create_test_server(aggregator),
            test_client: Client::new(),
            api_sender,
        }
    }

    /// Sends a transaction to the node via `ApiSender`.
    pub fn send<T>(&self, transaction: T)
    where
        T: Into<Signed<RawTransaction>>,
    {
        self.api_sender
            .broadcast_transaction(transaction.into())
            .expect("Cannot broadcast transaction");
    }

    /// Creates a requests builder for the public API scope.
    pub fn public(&self, kind: impl Display) -> RequestBuilder {
        RequestBuilder::new(
            self.test_server.url(""),
            &self.test_client,
            ApiAccess::Public,
            kind.to_string(),
        )
    }

    /// Creates a requests builder for the private API scope.
    pub fn private(&self, kind: impl Display) -> RequestBuilder {
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
pub struct RequestBuilder<'a, 'b, Q = ()>
where
    Q: 'b,
{
    test_server_url: String,
    test_client: &'a Client,
    access: ApiAccess,
    prefix: String,
    query: Option<&'b Q>,
    modifier: Option<ReqwestModifier<'b>>,
}

impl<'a, 'b, Q> fmt::Debug for RequestBuilder<'a, 'b, Q>
where
    Q: 'b + fmt::Debug + Serialize,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
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
        }
    }

    /// Sets a query data of the current request.
    pub fn query<T>(&'a self, query: &'b T) -> RequestBuilder<'a, 'b, T> {
        RequestBuilder {
            test_server_url: self.test_server_url.clone(),
            test_client: self.test_client,
            access: self.access,
            prefix: self.prefix.clone(),
            query: Some(query),
            modifier: None,
        }
    }

    /// Allows to modify a request before sending it by executing a provided closure.
    pub fn with<F>(&self, f: F) -> Self
    where
        F: Fn(ReqwestBuilder) -> ReqwestBuilder + 'b,
    {
        RequestBuilder {
            test_server_url: self.test_server_url.clone(),
            test_client: self.test_client,
            access: self.access,
            prefix: self.prefix.clone(),
            query: self.query,
            modifier: Some(Box::new(f)),
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
        Self::response_to_api_result(response)
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

        match response.status() {
            StatusCode::OK => Ok({
                let body = response.text().expect("Unable to get response text");
                trace!("Body: {}", body);
                serde_json::from_str(&body).expect("Unable to deserialize body")
            }),
            StatusCode::FORBIDDEN | StatusCode::UNAUTHORIZED => Err(api::Error::Unauthorized),
            StatusCode::BAD_REQUEST => Err(api::Error::BadRequest(error(response))),
            StatusCode::NOT_FOUND => Err(api::Error::NotFound(error(response))),
            s if s.is_server_error() => Err(api::Error::InternalError(format_err!(
                "{}",
                error(response)
            ))),
            s => panic!("Received non-error response status: {}", s.as_u16()),
        }
    }
}

/// Creates a test server.
fn create_test_server(aggregator: ApiAggregator) -> TestServer {
    let server = TestServer::with_factory(move || {
        let state = ServiceApiState::new(aggregator.blockchain().clone());
        App::with_state(state.clone())
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
