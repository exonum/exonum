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

//! Service with transaction removed from interface.
//! Used to test removed methods workflow.

use exonum::runtime::{ExecutionError, InstanceId};
use exonum_derive::*;
use exonum_rust_runtime::{DefaultInstance, ExecutionContext, Service};

pub const SERVICE_NAME: &str = "sample_service";
pub const SERVICE_ID: InstanceId = 100;

#[exonum_interface(removed_method_ids(0, 2))]
pub trait SampleServiceInterface<Ctx> {
    type Output;

    // Method with ID 0 was removed.

    /// Usual method.
    #[interface_method(id = 1)]
    fn method_b(&self, ctx: Ctx, arg: u64) -> Self::Output;

    // Method with ID 2 was removed.
}

impl SampleServiceInterface<ExecutionContext<'_>> for SampleService {
    type Output = Result<(), ExecutionError>;

    fn method_b(&self, _context: ExecutionContext<'_>, _arg: u64) -> Self::Output {
        Ok(())
    }
}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_factory(artifact_name = "sample-service", artifact_version = "1.0.0")]
#[service_dispatcher(implements("SampleServiceInterface"))]
pub struct SampleService;

impl DefaultInstance for SampleService {
    const INSTANCE_ID: u32 = SERVICE_ID;
    const INSTANCE_NAME: &'static str = SERVICE_NAME;
}

impl Service for SampleService {}
