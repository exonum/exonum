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

use exonum::runtime::{CommonError, ExecutionContext, ExecutionError, InstanceId};
use exonum_derive::*;
use exonum_merkledb::{
    access::{Access, FromAccess, RawAccessMut},
    Entry,
};
use exonum_rust_runtime::{
    api::{self, ServiceApiBuilder, ServiceApiState},
    DefaultInstance, Service,
};

use exonum_supervisor::Configure;

pub const SERVICE_ID: InstanceId = 512;
pub const SERVICE_NAME: &str = "inc";

#[derive(Debug, FromAccess)]
pub struct Schema<T: Access> {
    count: Entry<T::Base, u64>,
    params: Entry<T::Base, String>,
}

impl<T: Access> Schema<T> {
    pub fn new(access: T) -> Self {
        Self::from_root(access).unwrap()
    }

    pub fn count(&self) -> Option<u64> {
        self.count.get()
    }
}

impl<T> Schema<T>
where
    T: Access,
    T::Base: RawAccessMut,
{
    fn inc(&mut self) -> u64 {
        let new_count = self
            .count()
            .unwrap_or(0)
            .checked_add(1)
            .expect("attempt to add with overflow");
        self.count.set(new_count);
        new_count
    }
}

#[exonum_interface(auto_ids)]
pub trait IncInterface<Ctx> {
    type Output;
    fn inc(&self, context: Ctx, seed: u64) -> Self::Output;
}

/// Very simple test service that has one tx and one endpoint.
/// Basically, it just counts how many time a transaction was received.
#[derive(Clone, Default, Debug, ServiceFactory, ServiceDispatcher)]
#[service_dispatcher(implements("IncInterface", raw = "Configure<Params = String>"))]
#[service_factory(artifact_name = "inc", artifact_version = "1.0.0")]
pub struct IncService;

impl IncInterface<ExecutionContext<'_>> for IncService {
    type Output = Result<(), ExecutionError>;

    fn inc(&self, context: ExecutionContext<'_>, _seed: u64) -> Self::Output {
        Schema::new(context.service_data()).inc();
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PublicApi;

impl PublicApi {
    fn counter(state: &ServiceApiState<'_>, _query: ()) -> api::Result<u64> {
        Schema::new(state.service_data())
            .count()
            .ok_or_else(|| api::Error::not_found().title("Counter is not set yet"))
    }

    fn ping(_state: &ServiceApiState<'_>, _query: ()) -> api::Result<()> {
        Ok(())
    }

    fn wire(builder: &mut ServiceApiBuilder) {
        builder
            .public_scope()
            .endpoint("v1/counter", Self::counter)
            .endpoint("v1/ping", Self::ping);
    }
}

impl Service for IncService {
    fn wire_api(&self, builder: &mut ServiceApiBuilder) {
        PublicApi::wire(builder);
    }
}

impl DefaultInstance for IncService {
    const INSTANCE_ID: InstanceId = SERVICE_ID;
    const INSTANCE_NAME: &'static str = SERVICE_NAME;
}

impl Configure for IncService {
    type Params = String;

    fn verify_config(
        &self,
        context: ExecutionContext<'_>,
        params: Self::Params,
    ) -> Result<(), ExecutionError> {
        context
            .caller()
            .as_supervisor()
            .ok_or(CommonError::UnauthorizedCaller)?;

        match params.as_ref() {
            "error" => {
                let details = "IncService: Configure error request";
                Err(CommonError::malformed_arguments(details))
            }
            "panic" => panic!("IncService: Configure panic request"),
            _ => Ok(()),
        }
    }

    fn apply_config(
        &self,
        context: ExecutionContext<'_>,
        params: Self::Params,
    ) -> Result<(), ExecutionError> {
        context
            .caller()
            .as_supervisor()
            .ok_or(CommonError::UnauthorizedCaller)?;

        Schema::new(context.service_data())
            .params
            .set(params.clone());

        match params.as_str() {
            "apply_error" => {
                let details = "IncService: Configure error request";
                Err(CommonError::malformed_arguments(details))
            }
            "apply_panic" => panic!("IncService: Configure panic request"),
            _ => Ok(()),
        }
    }
}
