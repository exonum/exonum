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

use serde_derive::{Deserialize, Serialize};

use exonum::{
    blockchain::ExecutionError,
    crypto::Hash,
    runtime::{
        rust::{
            api::{self, ServiceApiBuilder},
            CallContext, DefaultInstance, Service,
        },
        BlockchainData, DispatcherError, InstanceId,
    },
};
use exonum_derive::*;
use exonum_merkledb::{
    access::{Access, RawAccessMut},
    Entry, Snapshot,
};
use exonum_proto::ProtobufConvert;

use crate::proto;
use exonum_supervisor::Configure;

pub const SERVICE_ID: InstanceId = 512;
pub const SERVICE_NAME: &str = "inc";

#[derive(Debug, FromAccess)]
pub struct Schema<T: Access> {
    count: Entry<T::Base, u64>,
    params: Entry<T::Base, String>,
}

impl<T: Access> Schema<T> {
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

#[derive(
    Serialize, Deserialize, Clone, Debug, PartialEq, ProtobufConvert, BinaryValue, ObjectHash,
)]
#[protobuf_convert(source = "proto::TxInc")]
pub struct Inc {
    pub seed: u64,
}

#[exonum_interface]
pub trait IncInterface {
    fn inc(&self, context: CallContext<'_>, arg: Inc) -> Result<(), ExecutionError>;
}

/// Very simple test service that has one tx and one endpoint.
/// Basically, it just counts how many time the tx was received.
#[derive(Clone, Default, Debug, ServiceFactory, ServiceDispatcher)]
#[service_dispatcher(implements("IncInterface", "Configure<Params = String>"))]
#[service_factory(
    artifact_name = "inc",
    artifact_version = "1.0.0",
    proto_sources = "proto"
)]
pub struct IncService;

impl IncInterface for IncService {
    fn inc(&self, context: CallContext<'_>, _arg: Inc) -> Result<(), ExecutionError> {
        Schema::new(context.service_data()).inc();
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PublicApi;

impl PublicApi {
    fn counter(state: &api::ServiceApiState<'_>, _query: ()) -> api::Result<u64> {
        Schema::new(state.service_data())
            .count()
            .ok_or_else(|| api::Error::NotFound("Counter is not set yet".to_owned()))
    }

    fn wire(builder: &mut ServiceApiBuilder) {
        builder.public_scope().endpoint("v1/counter", Self::counter);
    }
}

impl Service for IncService {
    fn state_hash(&self, _data: BlockchainData<&dyn Snapshot>) -> Vec<Hash> {
        vec![]
    }

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
        context: CallContext<'_>,
        params: Self::Params,
    ) -> Result<(), ExecutionError> {
        context
            .caller()
            .as_supervisor()
            .ok_or(DispatcherError::UnauthorizedCaller)?;

        match params.as_ref() {
            "error" => {
                let details = "IncService: Configure error request";
                Err(DispatcherError::malformed_arguments(details))
            }
            "panic" => panic!("IncService: Configure panic request"),
            _ => Ok(()),
        }
    }

    fn apply_config(
        &self,
        context: CallContext<'_>,
        params: Self::Params,
    ) -> Result<(), ExecutionError> {
        context
            .caller()
            .as_supervisor()
            .ok_or(DispatcherError::UnauthorizedCaller)?;

        Schema::new(context.service_data())
            .params
            .set(params.clone());

        match params.as_str() {
            "apply_error" => {
                let details = "IncService: Configure error request";
                Err(DispatcherError::malformed_arguments(details))
            }
            "apply_panic" => panic!("IncService: Configure panic request"),
            _ => Ok(()),
        }
    }
}
