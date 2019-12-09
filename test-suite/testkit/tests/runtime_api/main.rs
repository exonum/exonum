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

#[macro_use]
extern crate assert_matches;

use exonum::{api::Error as ApiError, runtime::rust::ProtoSourceFile};
use exonum_testkit::{TestKit, TestKitBuilder};

use crate::service::TestRuntimeApiService;

mod proto;
mod service;

pub fn testkit_with_rust_service() -> TestKit {
    TestKitBuilder::validator()
        .with_logger()
        .with_validators(1)
        .with_default_rust_service(TestRuntimeApiService)
        .create()
}

#[test]
fn test_exonum_protos_with_service() {
    let mut testkit = testkit_with_rust_service();

    let response = testkit
        .api()
        .public(exonum_testkit::ApiKind::RustRuntime)
        .get::<Vec<ProtoSourceFile>>("proto-sources");

    match response {
        Ok(proto_files) => {
            assert_eq!(proto_files.len(), 7);
            let proto_names = [
                "blockchain.proto",
                "consensus.proto",
                "doc_tests.proto",
                "runtime.proto",
                "tests.proto",
                "common.proto",
                "types.proto",
            ];
            proto_files
                .iter()
                .for_each(|proto| assert!(proto_names.contains(&proto.name.as_ref())));
        }
        Err(err) => panic!("Rust runtime Api failed with: {}", err),
    }
}

#[test]
fn test_exonum_protos_without_service() {
    let mut testkit = TestKitBuilder::validator().with_validators(1).create();

    let response = testkit
        .api()
        .public(exonum_testkit::ApiKind::RustRuntime)
        .get::<Vec<ProtoSourceFile>>("proto-sources");

    match response {
        // TODO make corresponding check after fix ECR-3948
        Ok(_) => panic!("Unexpected OK"),
        Err(err) => assert_matches!(err, ApiError::NotFound(ref body) if body == ""),
    }
}

#[test]
fn test_service_protos_with_service() {
    let mut testkit = testkit_with_rust_service();

    let response = testkit
        .api()
        .public(exonum_testkit::ApiKind::RustRuntime)
        .get::<Vec<ProtoSourceFile>>("proto-sources?artifact=test-runtime-api:0.0.1");

    match response {
        Ok(proto_files) => {
            assert_eq!(proto_files.len(), 1);
            assert_eq!(proto_files[0].name, "test_service.proto".to_string());
        }
        Err(err) => panic!("Rust runtime Api unexpectedly failed with: {}", err),
    }
}

#[test]
fn test_service_protos_with_incorrect_service() {
    let mut testkit = testkit_with_rust_service();

    let response = testkit
        .api()
        .public(exonum_testkit::ApiKind::RustRuntime)
        .get::<Vec<ProtoSourceFile>>("proto-sources?artifact=invalid-service:0.0.1");

    match response {
        Ok(_) => panic!("Unexpected OK"),
        Err(err) => assert_matches!(err, ApiError::NotFound(ref body) if body == "Unable to find sources for artifact invalid-service:0.0.1"),
    }
}
