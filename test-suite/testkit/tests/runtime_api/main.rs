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

    let proto = testkit
        .api()
        .public(exonum_testkit::ApiKind::Runtime)
        .get::<Vec<ProtoSourceFile>>("proto-sources")
        .unwrap();
    assert_ne!(proto.len(), 0);
}

#[test]
fn test_exonum_protos_without_service() {
    let mut testkit = TestKitBuilder::validator().with_validators(1).create();

    let response = testkit
        .api()
        .public(exonum_testkit::ApiKind::Runtime)
        .get::<Vec<ProtoSourceFile>>("proto-sources")
        .unwrap_err();

    assert_matches!(
        response,
        ApiError::NotFound(ref body) if body == ""
    );
}

#[test]
fn test_service_protos_with_service() {
    let mut testkit = testkit_with_rust_service();

    let proto = testkit
        .api()
        .public(exonum_testkit::ApiKind::Runtime)
        .get::<Vec<ProtoSourceFile>>("proto-sources?artifact=test-runtime-api:0.0.1")
        .unwrap();

    assert_eq!(proto.len(), 1);
}

#[test]
fn test_service_protos_without_service() {
    let mut testkit = TestKitBuilder::validator().with_validators(1).create();

    let response = testkit
        .api()
        .public(exonum_testkit::ApiKind::Runtime)
        .get::<Vec<ProtoSourceFile>>("proto-sources")
        .unwrap_err();

    assert_matches!(
        response,
        ApiError::NotFound(ref body) if body == ""
    );
}
