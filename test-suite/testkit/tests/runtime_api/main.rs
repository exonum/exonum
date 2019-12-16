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

use std::collections::HashSet;

use crate::service::TestRuntimeApiService;
use exonum_testkit::{ApiKind, TestKit, TestKitApi, TestKitBuilder};

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

// Rust-runtime returns correct core source files
fn test_exonum_core_protos(api: &TestKitApi) {
    let response: HashSet<String> = api
        .public(ApiKind::RustRuntime)
        .query(&ProtoSourcesQuery::Core)
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

#[test]
fn core_protos_with_service() {
    let (_, api) = testkit_with_rust_service();
    test_exonum_core_protos(&api);
}

#[test]
#[should_panic] // TODO: Remove `should_panic` after fix (ECR-3948)
fn core_protos_without_services() {
    let mut testkit = TestKitBuilder::validator().with_validators(1).create();
    test_exonum_core_protos(&testkit.api());
}

/// Rust-runtime api returns correct source files of the specified artifact.
#[test]
fn service_protos_with_service() {
    let (_, api) = testkit_with_rust_service();

    let proto_files: Vec<ProtoSourceFile> = api
        .public(ApiKind::RustRuntime)
        .query(&ProtoSourcesQuery::Artifact {
            name: "test-runtime-api".to_owned(),
            version: "0.0.1".parse().unwrap(),
        })
        .get("proto-sources")
        .expect("Rust runtime Api unexpectedly failed");

    const EXPECTED_CONTENT: &str = include_str!("proto/test_service.proto");

    assert_eq!(proto_files.len(), 1);
    assert_eq!(proto_files[0].name, "test_service.proto".to_string());
    assert_eq!(proto_files[0].content, EXPECTED_CONTENT.to_string());
}

/// Rust-runtime API should return error in case of an incorrect artifact.
#[test]
fn service_protos_with_incorrect_service() {
    let (_, api) = testkit_with_rust_service();

    let err = api
        .public(ApiKind::RustRuntime)
        .query(&ProtoSourcesQuery::Artifact {
            name: "invalid-service".to_owned(),
            version: "0.0.1".parse().unwrap(),
        })
        .get::<Vec<ProtoSourceFile>>("proto-sources")
        .expect_err("Rust runtime Api returns a fake source!");

    const EXPECTED_ERROR: &str = "Unable to find sources for artifact";
    assert_matches!(
        err,
        ApiError::NotFound(ref actual_error) if actual_error.contains(EXPECTED_ERROR)
    )
}
