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

//! Tests related to the API.

use exonum_api as api;
use exonum_testkit::{ApiKind, TestKit, TestKitApi};
use pretty_assertions::assert_eq;

use crate::api_service::{ApiService, PingQuery, SERVICE_ID, SERVICE_NAME};

mod api_service;

fn init_testkit() -> (TestKit, TestKitApi) {
    let mut testkit = TestKit::for_rust_service(ApiService, SERVICE_NAME, SERVICE_ID, ());
    let api = testkit.api();
    (testkit, api)
}

/// Performs basic get request to detect that API works at all.
#[test]
fn ping_pong() {
    let (_testkit, api) = init_testkit();

    let ping = PingQuery { value: 64 };
    let pong: u64 = api
        .public(ApiKind::Service("api-service"))
        .query(&ping)
        .get("ping-pong")
        .expect("Request to the valid endpoint failed");
    assert_eq!(ping.value, pong);
}

/// Checks that for deprecated endpoints the corresponding warning is added to the headers
/// of the response.
#[test]
fn deprecated() {
    let (_testkit, api) = init_testkit();

    let ping = PingQuery { value: 64 };
    const UNBOUND_WARNING: &str =
        "299 - \"Deprecated API: This endpoint is deprecated, \
         see the service documentation to find an alternative. \
         Currently there is no specific date for disabling this endpoint.\"";

    const WARNING_WITH_DEADLINE: &str =
        "299 - \"Deprecated API: This endpoint is deprecated, \
         see the service documentation to find an alternative. \
         The old API is maintained until Fri, 31 Dec 2055 23:59:59 GMT.\"";

    let pong: u64 = api
        .public(ApiKind::Service("api-service"))
        .query(&ping)
        .expect_header("Warning", UNBOUND_WARNING)
        .get("ping-pong-deprecated")
        .expect("Request to the valid endpoint failed");
    assert_eq!(ping.value, pong);

    let pong: u64 = api
        .public(ApiKind::Service("api-service"))
        .query(&ping)
        .expect_header("Warning", WARNING_WITH_DEADLINE)
        .get("ping-pong-deprecated-with-deadline")
        .expect("Request to the valid endpoint failed");
    assert_eq!(ping.value, pong);

    let pong: u64 = api
        .public(ApiKind::Service("api-service"))
        .query(&ping)
        .expect_header("Warning", UNBOUND_WARNING)
        .post("ping-pong-deprecated-mut")
        .expect("Request to the valid endpoint failed");
    assert_eq!(ping.value, pong);
}

/// Checks that endpoints marked as `Gone` return the corresponding HTTP error.
#[test]
fn gone() {
    let (_testkit, api) = init_testkit();

    let ping = PingQuery { value: 64 };

    let pong_error: api::Error = api
        .public(ApiKind::Service("api-service"))
        .query(&ping)
        .get::<u64>("gone-immutable")
        .expect_err("Request to the `Gone` endpoint succeed");

    assert_eq!(pong_error.http_code, api::HttpStatusCode::GONE);
    assert_eq!(
        pong_error.body.source,
        format!("{}:{}", SERVICE_ID, SERVICE_NAME)
    );

    let pong_error: api::Error = api
        .public(ApiKind::Service("api-service"))
        .query(&ping)
        .post::<u64>("gone-mutable")
        .expect_err("Request to the `Gone` endpoint succeed");

    assert_eq!(pong_error.http_code, api::HttpStatusCode::GONE);
    assert_eq!(
        pong_error.body.source,
        format!("{}:{}", SERVICE_ID, SERVICE_NAME)
    );
}

/// Checks that endpoints marked as `MovedPermanently` return the corresponding HTTP error, and
/// the response contains location in headers.
#[test]
fn moved() {
    let (_testkit, api) = init_testkit();

    let ping = PingQuery { value: 64 };

    let pong_error: api::Error = api
        .public(ApiKind::Service("api-service"))
        .query(&ping)
        .expect_header("Location", "../ping-pong?value=64")
        .get::<u64>("moved-immutable")
        .expect_err("Request to the `MovedPermanently` endpoint succeed");

    assert_eq!(pong_error.http_code, api::HttpStatusCode::MOVED_PERMANENTLY);
    assert_eq!(
        pong_error.body.source,
        format!("{}:{}", SERVICE_ID, SERVICE_NAME)
    );

    let pong_error: api::Error = api
        .public(ApiKind::Service("api-service"))
        .query(&ping)
        .expect_header("Location", "../ping-pong-deprecated-mut")
        .post::<u64>("moved-mutable")
        .expect_err("Request to the `MovedPermanently` endpoint succeed");

    assert_eq!(pong_error.http_code, api::HttpStatusCode::MOVED_PERMANENTLY);
    assert_eq!(
        pong_error.body.source,
        format!("{}:{}", SERVICE_ID, SERVICE_NAME)
    );
}

/// Checks response from endpoint with new error type.
#[test]
fn endpoint_with_new_error_type() {
    let (_testkit, api) = init_testkit();

    // Check OK response.
    let ok_query = PingQuery { value: 64 };
    let response: u64 = api
        .public(ApiKind::Service("api-service"))
        .query(&ok_query)
        .get("error")
        .expect("This request should be successful");
    assert_eq!(ok_query.value, response);

    // Check error response.
    let err_query = PingQuery { value: 63 };
    let error: api::Error = api
        .public(ApiKind::Service("api-service"))
        .query(&err_query)
        .get::<u64>("error")
        .expect_err("Should return error.");

    assert_eq!(error.http_code, api::HttpStatusCode::BAD_REQUEST);
    assert_eq!(error.body.docs_uri, "http://some-docs.com");
    assert_eq!(error.body.title, "Test endpoint error");
    assert_eq!(
        error.body.detail,
        format!("Test endpoint failed with query: {}", err_query.value)
    );
    assert_eq!(
        error.body.source,
        format!("{}:{}", SERVICE_ID, SERVICE_NAME)
    );
    assert_eq!(error.body.error_code, Some(42));
}
