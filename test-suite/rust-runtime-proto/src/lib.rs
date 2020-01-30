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

use exonum_rust_runtime::{ProtoSourceFile, ProtoSourcesQuery};
use exonum_testkit::{ApiKind, TestKit, TestKitApi, TestKitBuilder};
use pretty_assertions::assert_eq;
use std::collections::HashSet;

use crate::service::TestRuntimeApiService;

mod proto;
mod service;

#[cfg(test)]
mod tests;

/// Creates the TestKit and TestKitApi instances.
pub fn testkit_with_rust_service() -> (TestKit, TestKitApi) {
    let mut testkit = TestKitBuilder::validator()
        .with_logger()
        .with_default_rust_service(TestRuntimeApiService)
        .build();
    let api = testkit.api();
    (testkit, api)
}

/// Validates a list of proto descriptions retrieved from the rust-runtime API.
pub fn assert_exonum_core_protos(api: &TestKitApi) {
    let response: HashSet<String> = api
        .public(ApiKind::RustRuntime)
        .query(&ProtoSourcesQuery::Core)
        .get::<Vec<ProtoSourceFile>>("proto-sources")
        .expect("Rust runtime Api unexpectedly failed")
        .into_iter()
        .map(|proto_source| proto_source.name)
        .collect();

    let expected_files: HashSet<String> = vec![
        "exonum/key_value_sequence.proto",
        "exonum/blockchain.proto",
        "exonum/messages.proto",
        "exonum/proofs.proto",
        "exonum/runtime/auth.proto",
        "exonum/runtime/base.proto",
        "exonum/runtime/errors.proto",
        "exonum/runtime/lifecycle.proto",
        "exonum/common/bit_vec.proto",
        "exonum/crypto/types.proto",
        "exonum/proof/list_proof.proto",
        "exonum/proof/map_proof.proto",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();

    assert_eq!(response, expected_files);
}

#[test]
fn core_protos_with_service() {
    let (_, api) = testkit_with_rust_service();
    assert_exonum_core_protos(&api);
}

#[test]
#[should_panic] // TODO: Remove `should_panic` after fix (ECR-3948)
fn core_protos_without_services() {
    let mut testkit = TestKitBuilder::validator().build();
    assert_exonum_core_protos(&testkit.api());
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

    const EXPECTED_CONTENT: &str = include_str!("proto/tests.proto");

    assert_eq!(proto_files.len(), 1);
    assert_eq!(proto_files[0].name, "tests.proto".to_string());
    assert_eq!(proto_files[0].content, EXPECTED_CONTENT.to_string());
}

/// Rust-runtime API should return error in case of an incorrect artifact.
#[test]
fn service_protos_with_incorrect_service() {
    use exonum::runtime::{ArtifactId, RuntimeIdentifier};

    let (_, api) = testkit_with_rust_service();

    let artifact_id = ArtifactId::new(
        RuntimeIdentifier::Rust,
        "invalid-service",
        "0.0.1".parse().unwrap(),
    )
    .unwrap();
    let artifact_query = ProtoSourcesQuery::Artifact {
        name: artifact_id.name.clone(),
        version: artifact_id.version.clone(),
    };
    let error = api
        .public(ApiKind::RustRuntime)
        .query(&artifact_query)
        .get::<Vec<ProtoSourceFile>>("proto-sources")
        .expect_err("Rust runtime Api returns a fake source!");

    assert_eq!(&error.body.title, "Artifact sources not found");
    assert_eq!(
        error.body.detail,
        format!("Unable to find sources for artifact {}", artifact_id)
    );
}
