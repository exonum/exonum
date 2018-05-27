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

use bodyparser;
use exonum::api::ApiError;
use exonum::blockchain::{SharedNodeState, Transaction};
use exonum::node::{create_private_api_handler, create_public_api_handler, ApiSender,
                   TransactionSend};
use iron::headers::{ContentType, Headers};
use iron::status::{self, StatusClass};
use iron::{Chain, Handler, IronError, IronResult, Plugin, Request, Response};
use iron_test::{request, response};
use log::Level;
use mount::{Mount, OriginalUrl};
use serde::{Deserialize, Serialize};
use serde_json;
use serde_json::Value as JsonValue;

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
            public_handler: {
                let mut handler = create_public_api_handler(
                    blockchain.clone(),
                    api_state.clone(),
                    &testkit.api_config,
                );
                handler.link_after(|req: &mut Request, resp| {
                    log_request(&ApiAccess::Public, req, resp)
                });
                handler
            },

            private_handler: {
                let mut handler = create_private_api_handler(
                    blockchain.clone(),
                    api_state,
                    testkit.api_sender.clone(),
                    &testkit.api_config,
                );
                handler.link_after(|req: &mut Request, resp| {
                    log_request(&ApiAccess::Private, req, resp)
                });
                handler
            },

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

    pub(crate) fn into_handlers<H: Handler>(self, testkit_handler: H) -> (Chain, Chain) {
        let (public_handler, private_handler) = (self.public_handler, self.private_handler);

        let mut testkit_handler = Chain::new(testkit_handler);
        testkit_handler
            .link_after(|req: &mut Request, resp| log_request(&ApiAccess::Private, req, resp));

        let mut private_mount = Mount::new();
        private_mount.mount("api/testkit", testkit_handler);
        private_mount.mount("", private_handler);

        (public_handler, Chain::new(private_mount))
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

    fn get_internal<H, D>(handler: &H, endpoint: &str, expect_error: bool) -> D
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
        )
    }

    /// Gets an error from a public endpoint of the node.
    ///
    /// # Panics
    ///
    /// - Panics if the response has a non-error response status.
    pub fn get_err(&self, kind: ApiKind, endpoint: &str) -> ApiError {
        let url = format!("http://localhost:3000/{}/{}", kind.into_prefix(), endpoint);
        let response = match request::get(&url, Headers::new(), &self.public_handler) {
            Ok(response) | Err(IronError { response, .. }) => response,
        };
        TestKitApi::response_to_api_error(response)
    }

    fn post_internal<H, T, D>(handler: &H, endpoint: &str, data: &T) -> D
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
        )
    }

    /// Converts iron Response to ApiError.
    ///
    /// # Panics
    ///
    /// - Panics if the response has a non-error response status.
    fn response_to_api_error(response: Response) -> ApiError {
        fn extract_description(body: &str) -> Option<String> {
            match serde_json::from_str::<JsonValue>(body).ok()? {
                JsonValue::Object(ref object) if object.contains_key("description") => {
                    Some(object["description"].as_str()?.to_owned())
                }
                JsonValue::String(string) => Some(string),
                _ => None,
            }
        }

        fn error(response: Response) -> String {
            let body = response::extract_body_to_string(response);
            extract_description(&body).unwrap_or(body)
        }

        let status = response.status.expect("Status header is not set");

        match status {
            status::Forbidden => ApiError::Unauthorized,
            status::BadRequest => ApiError::BadRequest(error(response)),
            status::NotFound => ApiError::NotFound(error(response)),
            s if s.is_server_error() => ApiError::InternalError(error(response).into()),
            s => panic!("Received non-error response status: {}", s.to_u16()),
        }
    }
}

#[derive(Debug)]
enum ApiAccess {
    Public,
    Private,
}

impl fmt::Display for ApiAccess {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ApiAccess::Public => formatter.write_str("public"),
            ApiAccess::Private => formatter.write_str("private"),
        }
    }
}

// Logging middleware for `TestKitApi`.
fn log_request(
    access: &ApiAccess,
    request: &mut Request,
    mut response: Response,
) -> IronResult<Response> {
    use iron::headers::ContentLength;
    use iron::method::Method;
    use iron::response::WriteBody;

    fn has_body(method: &Method) -> bool {
        match *method {
            Method::Get | Method::Head | Method::Delete | Method::Trace => false,
            _ => true,
        }
    }

    if !log_enabled!(Level::Trace) {
        // Avoid expensive string allocations.
        return Ok(response);
    }

    let mut url = request
        .extensions
        .get::<OriginalUrl>()
        .unwrap_or(&request.url)
        .path()
        .join("/");
    if let Some(query) = request.url.query() {
        url += "?";
        url += query;
    }

    let req_body: Option<String> = if has_body(&request.method) {
        request
            .get::<bodyparser::Raw>()
            .expect("cannot read request body")
    } else {
        None
    };

    let response_body: Option<String> = {
        let len: u64 = response
            .headers
            .get::<ContentLength>()
            .map(|len| len.0)
            .unwrap_or_default();

        response.body.take().map(|mut body| {
            let mut buffer = Vec::with_capacity(len as usize);
            body.write_body(&mut buffer)
                .expect("cannot write response body to buffer");
            String::from_utf8(buffer).expect("cannot decode response body")
        })
    };

    trace!(
        "{method} ({access}) /{url}{req_body_tag}{req_body}\n\
         Response: {resp_status}\n{resp_body}\n",
        method = request.method,
        access = access,
        url = url,
        req_body_tag = if req_body.is_some() { "\nBody: " } else { "" },
        req_body = req_body.unwrap_or_default(),
        resp_status = response
            .status
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| "(no status)".to_string()),
        resp_body = response_body
            .as_ref()
            .map(String::as_ref)
            .unwrap_or("(no body)")
    );

    // Return the body to the response
    response.body = response_body.map(|body| Box::new(body) as Box<WriteBody>);
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_err_non_json() {
        let response = Response::with((status::NotFound, "Not found"));
        assert_matches!(
            TestKitApi::response_to_api_error(response),
            ApiError::NotFound(ref body) if body == "Not found"
        );
    }

    #[test]
    fn test_get_err_json_string() {
        let response = Response::with((status::NotFound, "\"Wallet not found\""));
        assert_matches!(
            TestKitApi::response_to_api_error(response),
            ApiError::NotFound(ref body) if body == "Wallet not found"
        );
    }

    #[test]
    fn test_get_err_json_object_with_description() {
        let response_body = r#"{ "debug": "Some debug info", "description": "Some description" }"#;
        let response = Response::with((status::BadRequest, response_body));
        assert_matches!(
            TestKitApi::response_to_api_error(response),
            ApiError::BadRequest(ref body) if body == "Some description"
        );
    }

    #[test]
    fn test_get_err_json_object_without_description() {
        let response_body = r#"{ "type": "unknown" }"#;
        let response = Response::with((status::BadRequest, response_body));
        assert_matches!(
            TestKitApi::response_to_api_error(response),
            ApiError::BadRequest(ref body) if body == response_body
        );
    }

    #[test]
    fn test_get_err_other_json() {
        let response_body = r#"[1, 2, 3]"#;
        let response = Response::with((status::BadRequest, response_body));
        assert_matches!(
            TestKitApi::response_to_api_error(response),
            ApiError::BadRequest(ref body) if body == response_body
        );
    }

    #[test]
    #[should_panic(expected = "Received non-error response status")]
    fn test_get_err_non_error_status() {
        let response = Response::with(status::Ok);
        TestKitApi::response_to_api_error(response);
    }
}
