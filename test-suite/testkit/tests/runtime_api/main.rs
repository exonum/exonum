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

use assert_matches::assert_matches;

use exonum::{
    api::Error as ApiError,
    runtime::rust::{ProtoSourceFile, ProtoSourcesQuery},
};
use exonum_testkit::{TestKit, TestKitApi, TestKitBuilder};

use crate::service::TestRuntimeApiService;
use std::collections::HashSet;

mod proto;
mod service;

pub fn testkit_with_rust_service() -> (TestKit, TestKitApi) {
    let mut testkit = TestKitBuilder::validator()
        .with_logger()
        .with_validators(1)
        .with_default_rust_service(TestRuntimeApiService)
        .create();
    let api = testkit.api();
    (testkit, api)
}

// Rust-runtime api returns correct source files of Exonum
#[test]
fn test_exonum_protos_with_service() {
    let (_, api) = testkit_with_rust_service();

    let response: HashSet<String> = api
        .public(exonum_testkit::ApiKind::RustRuntime)
        .get::<Vec<ProtoSourceFile>>("proto-sources")
        .expect("Rust runtime Api unexpectedly failed")
        .into_iter()
        .map(|proto_source| proto_source.name)
        .collect();

    let expected_files: HashSet<String> = vec![
        "blockchain.proto",
        "consensus.proto",
        "doc_tests.proto",
        "runtime.proto",
        "tests.proto",
        "common.proto",
        "types.proto",
    ]
    .into_iter()
    .map(|s| s.to_owned())
    .collect();

    assert_eq!(response, expected_files);
}

// Rust-runtime without any services returns correct source files of Exonum
#[test]
// TODO: Remove should_panic after fix ECR-3948
#[should_panic]
fn test_exonum_protos_without_service() {
    let mut testkit = TestKitBuilder::validator().with_validators(1).create();

    let response: HashSet<String> = testkit
        .api()
        .public(exonum_testkit::ApiKind::RustRuntime)
        .get::<Vec<ProtoSourceFile>>("proto-sources")
        .expect("Rust runtime Api unexpectedly failed")
        .into_iter()
        .map(|proto_source| proto_source.name.clone())
        .collect();

    let expected_files: HashSet<String> = vec![
        "blockchain.proto",
        "consensus.proto",
        "doc_tests.proto",
        "runtime.proto",
        "tests.proto",
        "common.proto",
        "types.proto",
    ]
    .into_iter()
    .map(|s| s.to_owned())
    .collect();

    assert_eq!(response, expected_files);
}

// Rust-runtime api returns correct source files of the specified artifact
#[test]
fn test_service_protos_with_service() {
    let (_, api) = testkit_with_rust_service();

    let proto_files = api
        .public(exonum_testkit::ApiKind::RustRuntime)
        .query(&ProtoSourcesQuery {
            artifact: Some("test-runtime-api:0.0.1".to_owned()),
        })
        .get::<Vec<ProtoSourceFile>>("proto-sources")
        .expect("Rust runtime Api unexpectedly failed");

    const EXPECTED_CONTENT: &str = "syntax = \"proto3\";\n\
                                    package exonum.testkit;\n\
                                    message TxMessage { string message = 1; }\n";

    assert_eq!(proto_files.len(), 1);
    assert_eq!(proto_files[0].name, "test_service.proto".to_string());
    assert_eq!(proto_files[0].content, EXPECTED_CONTENT.to_string());
}

// Rust-runtime api should return error in case incorrect artifact
#[test]
fn test_service_protos_with_incorrect_service() {
    let (_, api) = testkit_with_rust_service();

    let err = api
        .public(exonum_testkit::ApiKind::RustRuntime)
        .query(&ProtoSourcesQuery {
            artifact: Some("invalid-service:0.0.1".to_owned()),
        })
        .get::<Vec<ProtoSourceFile>>("proto-sources")
        .expect_err("Rust runtime Api returns a fake source!");

    const EXPECTED_ERROR: &str = "Unable to find sources for artifact invalid-service:0.0.1";
    assert_matches!(err, ApiError::NotFound(ref actual_error) if actual_error == EXPECTED_ERROR)
}
