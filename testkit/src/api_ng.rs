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

//! API encapsulation for the testkit.

pub use api::ApiKind;

use actix_web::http::Method;
use actix_web::{test::TestServer, App, HttpMessage};
use failure;
use serde_urlencoded;

use std::fmt;

use exonum::{api_ng::{ApiAggregator, ServiceApiState},
             blockchain::{SharedNodeState, Transaction},
             encoding::serialize::reexport::{DeserializeOwned, Serialize},
             node::{ApiSender, TransactionSend}};

use api::ApiAccess;
use TestKit;

/// TODO
pub type Error = failure::Error;

/// Creates test server.
fn create_test_server(aggregator: ApiAggregator) -> TestServer {
    let server = TestServer::with_factory(move || {
        let state = ServiceApiState::new(aggregator.blockchain());
        App::with_state(state.clone())
            .scope("public/api", |scope| {
                aggregator.extend_api(ApiAccess::Public, scope)
            })
            .scope("private/api", |scope| {
                aggregator.extend_api(ApiAccess::Private, scope)
            })
    });

    info!("Test server created on {}", server.addr());

    server
}

/// API encapsulation for the testkit. Allows to execute and synchronously retrieve results
/// for REST-ful endpoints of services.
pub struct TestKitApi {
    test_server: TestServer,
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
        let aggregator =
            ApiAggregator::new(testkit.blockchain().clone(), SharedNodeState::new(10_000));

        TestKitApi {
            test_server: create_test_server(aggregator),
            api_sender: testkit.api_sender.clone(),
        }
    }

    /// Sends a transaction to the node via `ApiSender`.
    pub fn send<T>(&self, transaction: T)
    where
        T: Into<Box<Transaction>>,
    {
        self.api_sender
            .send(transaction.into())
            .expect("Cannot send transaction");
    }

    /// TODO
    pub fn public(&mut self, kind: ApiKind) -> RequestBuilder {
        RequestBuilder::new(&mut self.test_server, ApiAccess::Public, kind)
    }

    /// TODO
    pub fn private(&mut self, kind: ApiKind) -> RequestBuilder {
        RequestBuilder::new(&mut self.test_server, ApiAccess::Private, kind)
    }
}

/// TODO
pub struct RequestBuilder<'a, Q = ()> {
    test_server: &'a mut TestServer,
    access: ApiAccess,
    kind: ApiKind,
    query: Option<Q>,
}

impl<'a, Q> fmt::Debug for RequestBuilder<'a, Q>
where
    Q: fmt::Debug + Serialize,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.debug_struct("RequestBuilder")
            .field("access", &self.access)
            .field("kind", &self.kind)
            .field("query", &self.query)
            .finish()
    }
}

impl<'a, Q> RequestBuilder<'a, Q>
where
    Q: Serialize,
{
    /// TODO
    pub fn new(test_server: &'a mut TestServer, access: ApiAccess, kind: ApiKind) -> Self {
        RequestBuilder {
            test_server,
            access,
            kind,
            query: None,
        }
    }

    ///TODO
    pub fn query<T>(self, query: T) -> RequestBuilder<'a, T> {
        RequestBuilder {
            test_server: self.test_server,
            access: self.access,
            kind: self.kind,
            query: Some(query),
        }
    }

    /// TODO
    pub fn get<R>(&mut self, endpoint: &str) -> Result<R, Error>
    where
        R: DeserializeOwned + 'static,
    {
        let kind = self.kind;
        let access = self.access;

        let params = self.query
            .as_ref()
            .map(|query| serde_urlencoded::to_string(query).expect("Unable to serialize query."))
            .unwrap_or_default();
        let path = format!("{}/{}/{}{}", access, kind, endpoint, params);

        trace!("GET: {}", self.test_server.url(&path));

        let request = self.test_server
            .client(Method::GET, &path)
            .finish()
            .expect("WTF")
            .send();

        let response = self.test_server.execute(request)?;

        trace!("Response: {:?}", response);

        self.test_server
            .execute(response.json())
            .map_err(From::from)
    }

    /// TODO
    pub fn post<R>(&mut self, endpoint: &str) -> Result<R, Error>
    where
        R: DeserializeOwned + 'static,
    {
        let kind = self.kind;
        let access = self.access;
        let path = format!("{}/{}/{}", access, kind, endpoint);

        trace!("POST: {}", self.test_server.url(&path));

        let mut request = self.test_server.client(Method::POST, &path);
        let request = if let Some(ref query) = self.query.as_ref() {
            request.json(query)
        } else {
            request.json(&())
        }.expect("WTF")
            .send();

        let response = self.test_server.execute(request)?;

        trace!("Response: {:?}", response);

        self.test_server
            .execute(response.json())
            .map_err(From::from)
    }
}
