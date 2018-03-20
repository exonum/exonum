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

use exonum::blockchain::{SharedNodeState, Transaction};
use exonum::node::{ApiSender, TransactionSend, create_public_api_handler,
                   create_private_api_handler};
use iron::{IronError, Handler, Chain};
use iron::headers::{ContentType, Headers};
use iron::status::StatusClass;
use iron_test::{request, response};
use serde::{Deserialize, Serialize};
use serde_json;

use std::fmt;

use super::TestKit;

/// Kind of public or private REST API of an Exonum node.
///
/// `ApiKind` allows to use `get*` and `post*` methods of [`TestKitApi`] more safely.
///
/// [`TestKitApi`]: struct.TestKitApi.html
#[derive(Debug)]
pub enum ApiKind {
    /// `api/system` endpoints of the built-in Exonum REST API.
    System,
    /// `api/explorer` endpoints of the built-in Exonum REST API.
    Explorer,
    /// Endpoints corresponding to a service with the specified string identifier.
    Service(&'static str),
}

impl ApiKind {
    fn into_prefix(self) -> String {
        match self {
            ApiKind::System => "api/system".to_string(),
            ApiKind::Explorer => "api/explorer".to_string(),
            ApiKind::Service(name) => format!("api/services/{}", name),
        }
    }
}

/// API encapsulation for the testkit. Allows to execute and synchronously retrieve results
/// for REST-ful endpoints of services.
pub struct TestKitApi {
    public_handler: Chain,
    private_handler: Chain,
    api_sender: ApiSender,
}

impl fmt::Debug for TestKitApi {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.debug_struct("TestKitApi").finish()
    }
}

impl TestKitApi {
    /// Creates a new instance of API.
    pub(crate) fn new(testkit: &TestKit) -> Self {

        let blockchain = &testkit.blockchain;
        let api_state = SharedNodeState::new(10_000);

        TestKitApi {
            public_handler: create_public_api_handler(
                blockchain.clone(),
                api_state.clone(),
                &testkit.api_config,
            ),

            private_handler: create_private_api_handler(
                blockchain.clone(),
                api_state,
                testkit.api_sender.clone(),
            ),

            api_sender: testkit.api_sender.clone(),
        }
    }

    /// Returns the mounting point for public APIs. Useful for intricate testing not covered
    /// by `get*` and `post*` functions.
    pub fn public_handler(&self) -> &Chain {
        &self.public_handler
    }

    /// Returns the mounting point for private APIs. Useful for intricate testing not covered
    /// by `get*` and `post*` functions.
    pub fn private_handler(&self) -> &Chain {
        &self.private_handler
    }

    /// Sends a transaction to the node via `ApiSender`.
    pub fn send<T>(&self, transaction: T)
    where
        T: Into<Box<Transaction>>,
    {
        self.api_sender.send(transaction.into()).expect(
            "Cannot send transaction",
        );
    }

    fn get_internal<H, D>(handler: &H, endpoint: &str, expect_error: bool, is_public: bool) -> D
    where
        H: Handler,
        for<'de> D: Deserialize<'de>,
    {
        let status_class = if expect_error {
            StatusClass::ClientError
        } else {
            StatusClass::Success
        };

        let url = format!("http://localhost:3000/{}", endpoint);
        let resp = request::get(&url, Headers::new(), handler);
        let resp = if expect_error {
            // Support either "normal" or erroneous responses.
            // For example, `Api.not_found_response()` returns the response as `Ok(..)`.
            match resp {
                Ok(resp) => resp,
                Err(IronError { response, .. }) => response,
            }
        } else {
            resp.expect("Got unexpected `Err(..)` response")
        };

        if let Some(ref status) = resp.status {
            if status.class() != status_class {
                panic!("Unexpected response status: {:?}", status);
            }
        } else {
            panic!("Response status not set");
        }

        let resp = response::extract_body_to_string(resp);

        let publicity = if is_public { "" } else { " private" };
        trace!("GET{} {}\nResponse:\n{}\n", publicity, endpoint, resp);

        serde_json::from_str(&resp).unwrap()
    }

    /// Gets information from a public endpoint of the node.
    ///
    /// # Panics
    ///
    /// - Panics if an error occurs during request processing (e.g., the requested endpoint is
    ///  unknown), or if the response has a non-20x response status.
    pub fn get<D>(&self, kind: ApiKind, endpoint: &str) -> D
    where
        for<'de> D: Deserialize<'de>,
    {
        TestKitApi::get_internal(
            &self.public_handler,
            &format!("{}/{}", kind.into_prefix(), endpoint),
            false,
            true,
        )
    }

    /// Gets information from a private endpoint of the node.
    ///
    /// # Panics
    ///
    /// - Panics if an error occurs during request processing (e.g., the requested endpoint is
    ///  unknown), or if the response has a non-20x response status.
    pub fn get_private<D>(&self, kind: ApiKind, endpoint: &str) -> D
    where
        for<'de> D: Deserialize<'de>,
    {
        TestKitApi::get_internal(
            &self.private_handler,
            &format!("{}/{}", kind.into_prefix(), endpoint),
            false,
            false,
        )
    }

    /// Gets an error from a public endpoint of the node.
    ///
    /// # Panics
    ///
    /// - Panics if the response has a non-40x response status.
    pub fn get_err<D>(&self, kind: ApiKind, endpoint: &str) -> D
    where
        for<'de> D: Deserialize<'de>,
    {
        TestKitApi::get_internal(
            &self.public_handler,
            &format!("{}/{}", kind.into_prefix(), endpoint),
            true,
            true,
        )
    }

    fn post_internal<H, T, D>(handler: &H, endpoint: &str, data: &T, is_public: bool) -> D
    where
        H: Handler,
        T: Serialize,
        for<'de> D: Deserialize<'de>,
    {
        let url = format!("http://localhost:3000/{}", endpoint);
        let body = serde_json::to_string(&data).expect("Cannot serialize data to JSON");
        let resp = request::post(
            &url,
            {
                let mut headers = Headers::new();
                headers.set(ContentType::json());
                headers
            },
            &body,
            handler,
        ).expect("Cannot send data");

        let resp = response::extract_body_to_string(resp);

        let publicity = if is_public { "" } else { " private" };
        trace!(
            "POST{} {}\nBody: \n{}\nResponse:\n{}\n",
            publicity,
            endpoint,
            body,
            resp
        );

        serde_json::from_str(&resp).expect("Cannot parse result")
    }

    /// Posts a transaction to the service using the public API. The returned value is the result
    /// of synchronous transaction processing, which includes running the API shim
    /// and `Transaction.verify()`. `Transaction.execute()` is not run until the transaction
    /// gets to a block via one of `create_block*()` methods.
    ///
    /// # Panics
    ///
    /// - Panics if an error occurs during request processing (e.g., the requested endpoint is
    ///  unknown).
    pub fn post<T, D>(&self, kind: ApiKind, endpoint: &str, transaction: &T) -> D
    where
        T: Serialize,
        for<'de> D: Deserialize<'de>,
    {
        TestKitApi::post_internal(
            &self.public_handler,
            &format!("{}/{}", kind.into_prefix(), endpoint),
            transaction,
            true,
        )
    }

    /// Posts a transaction to the service using the private API. The returned value is the result
    /// of synchronous transaction processing, which includes running the API shim
    /// and `Transaction.verify()`. `Transaction.execute()` is not run until the transaction
    /// gets to a block via one of `create_block*()` methods.
    ///
    /// # Panics
    ///
    /// - Panics if an error occurs during request processing (e.g., the requested endpoint is
    ///  unknown).
    pub fn post_private<T, D>(&self, kind: ApiKind, endpoint: &str, transaction: &T) -> D
    where
        T: Serialize,
        for<'de> D: Deserialize<'de>,
    {
        TestKitApi::post_internal(
            &self.private_handler,
            &format!("{}/{}", kind.into_prefix(), endpoint),
            transaction,
            false,
        )
    }
}
