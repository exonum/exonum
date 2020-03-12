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
use exonum_testkit::{ApiKind, Spec, TestKit, TestKitApi, TestKitBuilder};
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
        .with(Spec::new(TestRuntimeApiService).with_default_instance())
        .build();
    let api = testkit.api();
    (testkit, api)
}

/// Validates a list of proto descriptions retrieved from the rust-runtime API.
pub async fn assert_exonum_core_protos(api: &TestKitApi) {
    let response: HashSet<String> = api
        .public(ApiKind::RustRuntime)
        .query(&ProtoSourcesQuery::Core)
        .get::<Vec<ProtoSourceFile>>("proto-sources")
        .await
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
