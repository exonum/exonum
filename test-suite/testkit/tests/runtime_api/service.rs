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

use exonum::{
    crypto::Hash,
    runtime::{
        rust::{api::ServiceApiBuilder, DefaultInstance, Service},
        BlockchainData,
    },
};

use exonum_derive::*;
use exonum_merkledb::Snapshot;

#[exonum_interface]
pub trait TestServiceInterface {}

impl TestServiceInterface for TestRuntimeApiService {}

/// Define the service.
#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_factory(
    artifact_name = "test-runtime-api",
    artifact_version = "0.0.1",
    proto_sources = "crate::proto"
)]
#[service_dispatcher(implements("TestServiceInterface"))]
pub struct TestRuntimeApiService;

/// Implement a `Service` trait for the service.
impl Service for TestRuntimeApiService {
    fn wire_api(&self, _builder: &mut ServiceApiBuilder) {}
}

impl DefaultInstance for TestRuntimeApiService {
    const INSTANCE_ID: u32 = 101;
    const INSTANCE_NAME: &'static str = "test-runtime-api";
}
