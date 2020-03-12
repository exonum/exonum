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

use exonum::runtime::SUPERVISOR_INSTANCE_ID;
use exonum_api as api;
use exonum_rust_runtime::{RustRuntime, ServiceFactory};
use exonum_testkit::{ApiKind, Spec, TestKit, TestKitApi, TestKitBuilder};
use pretty_assertions::assert_eq;

use crate::{
    api_service::{ApiInterface, ApiService, ApiServiceV2, PingQuery, SERVICE_ID, SERVICE_NAME},
    supervisor::{StartMigration, Supervisor, SupervisorInterface},
};

mod api_service;
mod supervisor;

fn init_testkit() -> (TestKit, TestKitApi) {
    let mut testkit = TestKitBuilder::validator()
        .with(Spec::new(Supervisor).with_default_instance())
        .with(Spec::new(ApiService).with_default_instance())
        .with(Spec::migrating(ApiServiceV2))
        .build();
    let api = testkit.api();
    (testkit, api)
}

/// Performs basic get request to detect that API works at all.
#[tokio::test]
async fn ping_pong() {
    let (_testkit, api) = init_testkit();

    let ping = PingQuery { value: 64 };
    let pong: u64 = api
        .public(ApiKind::Service("api-service"))
        .query(&ping)
        .get("ping-pong")
        .await
        .expect("Request to the valid endpoint failed");
    assert_eq!(ping.value, pong);
}

#[tokio::test]
async fn submit_tx() {
    let (mut testkit, api) = init_testkit();

    let ping = PingQuery { value: 64 };
    api.public(ApiKind::Service("api-service"))
        .query(&ping)
        .post::<()>("submit-tx")
        .await
        .expect("Request to the valid endpoint failed");
    let block = testkit.create_block();
    assert_eq!(block.len(), 1);
    let expected_tx = testkit
        .us()
        .service_keypair()
        .do_nothing(SERVICE_ID, ping.value);
    assert_eq!(*block[0].message(), expected_tx);
}

/// Checks that for deprecated endpoints the corresponding warning is added to the headers
/// of the response.
#[tokio::test]
async fn deprecated() {
    let (_testkit, api) = init_testkit();

    let ping = PingQuery { value: 64 };
    const UNBOUND_WARNING: &str = "299 - \"Deprecated API: This endpoint is deprecated, \
         see the service documentation to find an alternative. \
         Currently there is no specific date for disabling this endpoint.\"";

    const WARNING_WITH_DEADLINE: &str = "299 - \"Deprecated API: This endpoint is deprecated, \
         see the service documentation to find an alternative. \
         The old API is maintained until Fri, 31 Dec 2055 23:59:59 GMT.\"";

    let pong: u64 = api
        .public(ApiKind::Service("api-service"))
        .query(&ping)
        .expect_header("Warning", UNBOUND_WARNING)
        .get("ping-pong-deprecated")
        .await
        .expect("Request to the valid endpoint failed");
    assert_eq!(ping.value, pong);

    let pong: u64 = api
        .public(ApiKind::Service("api-service"))
        .query(&ping)
        .expect_header("Warning", WARNING_WITH_DEADLINE)
        .get("ping-pong-deprecated-with-deadline")
        .await
        .expect("Request to the valid endpoint failed");
    assert_eq!(ping.value, pong);

    let pong: u64 = api
        .public(ApiKind::Service("api-service"))
        .query(&ping)
        .expect_header("Warning", UNBOUND_WARNING)
        .post("ping-pong-deprecated-mut")
        .await
        .expect("Request to the valid endpoint failed");
    assert_eq!(ping.value, pong);
}

/// Checks that endpoints marked as `Gone` return the corresponding HTTP error.
#[tokio::test]
async fn gone() {
    let (_testkit, api) = init_testkit();

    let ping = PingQuery { value: 64 };

    let pong_error: api::Error = api
        .public(ApiKind::Service("api-service"))
        .query(&ping)
        .get::<u64>("gone-immutable")
        .await
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
        .await
        .expect_err("Request to the `Gone` endpoint succeed");

    assert_eq!(pong_error.http_code, api::HttpStatusCode::GONE);
    assert_eq!(
        pong_error.body.source,
        format!("{}:{}", SERVICE_ID, SERVICE_NAME)
    );
}

/// Checks that endpoints marked as `MovedPermanently` return the corresponding HTTP error, and
/// the response contains location in headers.
#[tokio::test]
async fn moved() {
    let (_testkit, api) = init_testkit();

    let ping = PingQuery { value: 64 };

    let pong_error: api::Error = api
        .public(ApiKind::Service("api-service"))
        .query(&ping)
        .expect_header("Location", "../ping-pong?value=64")
        .get::<u64>("moved-immutable")
        .await
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
        .await
        .expect_err("Request to the `MovedPermanently` endpoint succeed");

    assert_eq!(pong_error.http_code, api::HttpStatusCode::MOVED_PERMANENTLY);
    assert_eq!(
        pong_error.body.source,
        format!("{}:{}", SERVICE_ID, SERVICE_NAME)
    );
}

/// Checks response from endpoint with new error type.
#[tokio::test]
async fn endpoint_with_new_error_type() {
    let (_testkit, api) = init_testkit();

    // Check OK response.
    let ok_query = PingQuery { value: 64 };
    let response: u64 = api
        .public(ApiKind::Service("api-service"))
        .query(&ok_query)
        .get("error")
        .await
        .expect("This request should be successful");
    assert_eq!(ok_query.value, response);

    // Check error response.
    let err_query = PingQuery { value: 63 };
    let error: api::Error = api
        .public(ApiKind::Service("api-service"))
        .query(&err_query)
        .get::<u64>("error")
        .await
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

#[tokio::test]
async fn submit_tx_when_service_is_stopped() {
    let (mut testkit, api) = init_testkit();
    let keys = testkit.us().service_keypair();

    let tx = keys.stop_service(SUPERVISOR_INSTANCE_ID, SERVICE_ID);
    let block = testkit.create_block_with_transaction(tx);
    block[0].status().expect("Cannot stop service");

    let ping = PingQuery { value: 64 };
    let err = api
        .public(ApiKind::Service("api-service"))
        .query(&ping)
        .post::<()>("submit-tx")
        .await
        .expect_err("Request to the valid endpoint should fail");
    assert_eq!(err.http_code, api::HttpStatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(err.body.title, "Service is not active");

    let block = testkit.create_block();
    assert!(block.is_empty());
}

#[tokio::test]
async fn submit_tx_when_service_is_frozen() {
    let (mut testkit, api) = init_testkit();
    let keys = testkit.us().service_keypair();

    let tx = keys.freeze_service(SUPERVISOR_INSTANCE_ID, SERVICE_ID);
    let block = testkit.create_block_with_transaction(tx);
    block[0].status().expect("Cannot freeze service");

    let ping = PingQuery { value: 64 };
    let err = api
        .public(ApiKind::Service("api-service"))
        .query(&ping)
        .post::<()>("submit-tx")
        .await
        .expect_err("Request to the valid endpoint should fail");
    assert_eq!(err.http_code, api::HttpStatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(err.body.title, "Service is not active");

    let block = testkit.create_block();
    assert!(block.is_empty());
}

#[tokio::test]
async fn error_after_migration() {
    let (mut testkit, api) = init_testkit();
    let keys = testkit.us().service_keypair();

    let tx = keys.freeze_service(SUPERVISOR_INSTANCE_ID, SERVICE_ID);
    let block = testkit.create_block_with_transaction(tx);
    block[0].status().unwrap();

    // Check that API endpoints are available.
    let pong: u64 = api
        .public(ApiKind::Service(SERVICE_NAME))
        .query(&PingQuery { value: 10 })
        .get("ping-pong")
        .await
        .expect("API should work fine after restart");
    assert_eq!(pong, 10);

    let tx = keys.start_migration(
        SUPERVISOR_INSTANCE_ID,
        StartMigration {
            instance_id: SERVICE_ID,
            new_artifact: ApiServiceV2.artifact_id(),
            migration_len: 0,
        },
    );
    let block = testkit.create_block_with_transaction(tx);
    block[0].status().unwrap();

    // Check that API endpoints return 50x errors now.
    let error = api
        .public(ApiKind::Service(SERVICE_NAME))
        .query(&PingQuery { value: 10 })
        .get::<u64>("ping-pong")
        .await
        .expect_err("API should return errors now");
    assert_eq!(error.http_code, api::HttpStatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        error.body.title,
        "Service has been upgraded, but its HTTP handlers are not rebooted yet"
    );
    assert!(error
        .body
        .detail
        .contains("Service `3:api-service` was upgraded to version 2.0.0"));

    let new_api = testkit.api();
    let pong: u64 = new_api
        .public(ApiKind::Service(SERVICE_NAME))
        .query(&PingQuery { value: 10 })
        .get("ping-pong")
        .await
        .expect("API should work fine after restart");
    assert_eq!(pong, 11);
}

async fn test_no_old_artifact_after_unload(unload: bool) {
    let (mut testkit, _) = init_testkit();
    let keys = testkit.us().service_keypair();

    // Freeze and migrate the service.
    let tx = keys.freeze_service(SUPERVISOR_INSTANCE_ID, SERVICE_ID);
    let block = testkit.create_block_with_transaction(tx);
    block[0].status().unwrap();

    let tx = keys.start_migration(
        SUPERVISOR_INSTANCE_ID,
        StartMigration {
            instance_id: SERVICE_ID,
            new_artifact: ApiServiceV2.artifact_id(),
            migration_len: 0,
        },
    );
    let block = testkit.create_block_with_transaction(tx);
    block[0].status().unwrap();

    if unload {
        // Unload the old service artifact.
        let tx = keys.unload_artifact(SUPERVISOR_INSTANCE_ID, ApiService.artifact_id());
        let block = testkit.create_block_with_transaction(tx);
        block[0].status().unwrap();
    }

    // Restart the testkit.
    let stopped = testkit.stop();
    let runtime = RustRuntime::builder()
        .with_factory(Supervisor)
        .with_factory(ApiServiceV2); // We don't need migration capabilities now.
    let mut testkit = stopped.resume(runtime);
    let api = testkit.api();

    // Check the HTTP API of the updated service.
    let pong: u64 = api
        .public(ApiKind::Service(SERVICE_NAME))
        .query(&PingQuery { value: 10 })
        .get("ping-pong")
        .await
        .expect("API should work fine after testkit restart");
    assert_eq!(pong, 11);
}

#[tokio::test]
async fn no_old_artifact_after_unload() {
    test_no_old_artifact_after_unload(true).await;
}

#[tokio::test]
#[should_panic(expected = "artifact `0:api-service:1.0.0` failed to deploy")]
async fn no_old_artifact_without_unload() {
    test_no_old_artifact_after_unload(false).await;
}
