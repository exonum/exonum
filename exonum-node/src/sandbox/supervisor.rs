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

use exonum::{
    blockchain::ConsensusConfig,
    crypto::{PublicKey, SecretKey},
    helpers::Height,
    merkledb::BinaryValue,
    messages::{AnyTx, Verified},
    runtime::{ExecutionContext, ExecutionError, InstanceId, SUPERVISOR_INSTANCE_ID},
};
use exonum_derive::*;
use exonum_rust_runtime::{DefaultInstance, Service, TxStub};
use serde_derive::{Deserialize, Serialize};

#[exonum_interface(auto_ids)]
pub trait Supervisor<Ctx> {
    type Output;

    fn update_config(&self, ctx: Ctx, arg: TxConfig) -> Self::Output;
    fn stop_service(&self, ctx: Ctx, service_id: InstanceId) -> Self::Output;
}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_dispatcher(implements("Supervisor"))]
#[service_factory(artifact_name = "supervisor", artifact_version = "0.1.0")]
pub struct SupervisorService;

impl Supervisor<ExecutionContext<'_>> for SupervisorService {
    type Output = Result<(), ExecutionError>;

    fn update_config(&self, mut ctx: ExecutionContext<'_>, arg: TxConfig) -> Self::Output {
        ctx.supervisor_extensions()
            .writeable_core_schema()
            .consensus_config_entry()
            .set(ConsensusConfig::from_bytes(arg.config.into()).unwrap());
        Ok(())
    }

    fn stop_service(&self, mut ctx: ExecutionContext<'_>, service_id: InstanceId) -> Self::Output {
        ctx.supervisor_extensions()
            .initiate_stopping_service(service_id)
    }
}

impl Service for SupervisorService {}

impl SupervisorService {
    pub const ID: InstanceId = SUPERVISOR_INSTANCE_ID;
}

impl DefaultInstance for SupervisorService {
    const INSTANCE_ID: InstanceId = Self::ID;
    const INSTANCE_NAME: &'static str = "config-updater";
}

#[derive(Debug, Serialize, Deserialize, BinaryValue, ObjectHash)]
#[binary_value(codec = "bincode")]
pub struct TxConfig {
    from: PublicKey,
    config: Vec<u8>,
    actual_from: Height,
}

impl TxConfig {
    pub fn create_signed(
        from: PublicKey,
        config: &[u8],
        actual_from: Height,
        signer: &SecretKey,
    ) -> Verified<AnyTx> {
        let msg = Self {
            from,
            config: config.to_owned(),
            actual_from,
        };

        TxStub
            .update_config(SupervisorService::ID, msg)
            .sign(from, signer)
    }
}
