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
    blockchain::{ExecutionError, InstanceCollection},
    crypto::Hash,
    runtime::{
        api::{self, ServiceApiBuilder},
        rust::{CallContext, Service},
        Caller, DispatcherError, InstanceDescriptor, InstanceId,
    },
};
use exonum_derive::{exonum_interface, BinaryValue, ObjectHash, ServiceDispatcher, ServiceFactory};
use exonum_merkledb::{Entry, IndexAccess, Snapshot};
use exonum_proto::ProtobufConvert;

use crate::proto;
use exonum_supervisor::Configure;

pub const SERVICE_ID: InstanceId = 512;
pub const SERVICE_NAME: &str = "inc";

#[derive(Debug)]
pub struct Schema<'a, T> {
    name: &'a str,
    access: T,
}

impl<'a, T: IndexAccess> Schema<'a, T> {
    pub fn new(name: &'a str, access: T) -> Self {
        Schema { name, access }
    }

    fn index_name(&self, name: &str) -> String {
        [SERVICE_NAME, ".", name].concat()
    }

    fn entry(&self) -> Entry<T, u64> {
        Entry::new(self.index_name("count"), self.access.clone())
    }

    pub fn count(&self) -> Option<u64> {
        self.entry().get()
    }

    fn inc(&mut self) -> u64 {
        let count = self
            .count()
            .unwrap_or(0)
            .checked_add(1)
            .expect("attempt to add with overflow");
        self.entry().set(count);
        count
    }
}

#[derive(
    Serialize, Deserialize, Clone, Debug, PartialEq, ProtobufConvert, BinaryValue, ObjectHash,
)]
#[protobuf_convert(source = "proto::TxInc")]
pub struct TxInc {
    pub seed: u64,
}

#[exonum_interface]
pub trait IncInterface {
    fn inc(&self, context: CallContext<'_>, arg: TxInc) -> Result<(), ExecutionError>;
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
    fn inc(&self, context: CallContext<'_>, _arg: TxInc) -> Result<(), ExecutionError> {
        let mut schema = Schema::new(context.instance().name, context.fork());
        schema.inc();
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PublicApi;

impl PublicApi {
    fn counter(state: &api::ServiceApiState<'_>, _query: ()) -> api::Result<u64> {
        let snapshot = state.snapshot();
        let schema = Schema::new(&state.instance.name, snapshot);
        schema
            .count()
            .ok_or_else(|| api::Error::NotFound("Counter is not set yet".to_owned()))
    }

    fn wire(builder: &mut ServiceApiBuilder) {
        builder.public_scope().endpoint("v1/counter", Self::counter);
    }
}

impl Service for IncService {
    fn wire_api(&self, builder: &mut ServiceApiBuilder) {
        PublicApi::wire(builder);
    }

    fn state_hash(&self, _instance: InstanceDescriptor<'_>, _snapshot: &dyn Snapshot) -> Vec<Hash> {
        vec![]
    }
}

impl From<IncService> for InstanceCollection {
    fn from(instance: IncService) -> Self {
        InstanceCollection::new(instance).with_instance(SERVICE_ID, SERVICE_NAME, Vec::default())
    }
}

impl Configure for IncService {
    type Params = String;

    fn verify_config(
        &self,
        context: CallContext<'_>,
        params: Self::Params,
    ) -> Result<(), ExecutionError> {
        context
            .verify_caller(Caller::as_supervisor)
            .ok_or(DispatcherError::UnauthorizedCaller)?;

        match params.as_ref() {
            "error" => Err(DispatcherError::malformed_arguments("Error!")).map_err(From::from),
            "panic" => panic!("Aaaa!"),
            _ => Ok(()),
        }
    }

    fn apply_config(
        &self,
        context: CallContext<'_>,
        params: Self::Params,
    ) -> Result<(), ExecutionError> {
        let (_, fork) = context
            .verify_caller(Caller::as_supervisor)
            .ok_or(DispatcherError::UnauthorizedCaller)?;

        Entry::new(format!("{}.params", context.instance().name), fork).set(params.clone());

        match params.as_ref() {
            "apply_error" => {
                Err(DispatcherError::malformed_arguments("Error!")).map_err(From::from)
            }
            "apply_panic" => panic!("Aaaa!"),
            _ => Ok(()),
        }
    }
}
