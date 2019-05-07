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

use crate::crypto::Hash;
use crate::messages::{BinaryForm, MethodId};
use crate::runtime::{error::ExecutionError, rust::TransactionContext, RuntimeIdentifier};
use exonum_merkledb::{Fork, Snapshot};

use failure::Error;
use protobuf::well_known_types::Any;

use super::RustArtifactSpec;
use crate::api::ServiceApiBuilder;
use crate::runtime::configuration_new::{Deploy, Init};

pub trait ServiceDispatcher {
    fn call(
        &self,
        method: MethodId,
        ctx: TransactionContext,
        payload: &[u8],
    ) -> Result<Result<(), ExecutionError>, Error>;
}

pub trait Service: ServiceDispatcher + std::fmt::Debug {
    fn initialize(&mut self, _ctx: TransactionContext, _arg: Any) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn before_commit(&self, _fork: &mut Fork) {}

    fn after_commit(&self, _fork: &mut Fork) {}

    fn state_hash(&self, _snapshot: &dyn Snapshot) -> Vec<Hash> {
        vec![]
    }

    fn wire_api(&self, _builder: &mut ServiceApiBuilder) {}
    // TODO: add other hooks such as "on node startup", etc.
}

pub trait ServiceFactory: std::fmt::Debug {
    fn artifact(&self) -> RustArtifactSpec;
    fn new_instance(&self) -> Box<dyn Service>;
    fn genesis_init_info(&self) -> Vec<GenesisInitInfo> {
        Vec::new()
    }
}

#[derive(Debug)]
pub struct GenesisInitInfo {
    artifact: RustArtifactSpec,
    service_name: String,
    service_constructor: Any,
}

impl GenesisInitInfo {
    pub fn with_init_tx(
        artifact: RustArtifactSpec,
        service_name: &str,
        init_tx: impl BinaryForm,
    ) -> Self {
        Self {
            artifact,
            service_name: service_name.to_owned(),
            service_constructor: {
                let mut any = Any::new();
                any.set_value(init_tx.encode().unwrap());
                any
            },
        }
    }

    pub fn no_init_tx(artifact: RustArtifactSpec, service_name: &str) -> Self {
        Self {
            artifact,
            service_name: service_name.to_owned(),
            service_constructor: Any::new(),
        }
    }

    pub fn get_deploy_tx(&self) -> Deploy {
        Deploy {
            runtime_id: RuntimeIdentifier::Rust as u32,
            activation_height: 0,
            artifact_spec: self.artifact.into_pb_any(),
        }
    }

    pub fn get_init_tx(&self) -> Init {
        Init {
            runtime_id: RuntimeIdentifier::Rust as u32,
            artifact_spec: self.artifact.into_pb_any(),
            instance_name: self.service_name.clone(),
            constructor_data: self.service_constructor.clone(),
        }
    }
}

#[macro_export]
macro_rules! impl_service_dispatcher {
    ($struct_name:ident, $interface:ident) => {
        impl $crate::runtime::rust::service::ServiceDispatcher for $struct_name {
            fn call(
                &self,
                method: $crate::messages::MethodId,
                ctx: $crate::runtime::rust::TransactionContext,
                payload: &[u8],
            ) -> Result<Result<(), $crate::runtime::error::ExecutionError>, failure::Error> {
                <$struct_name as $interface>::_dispatch(self, ctx, method, payload)
            }
        }
    };
}
