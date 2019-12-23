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

pub use crate::proto::TxConfig;

use exonum::{
    blockchain::ConsensusConfig,
    crypto::{PublicKey, SecretKey},
    helpers::Height,
    messages::{AnyTx, Verified},
    runtime::{ExecutionError, InstanceId, SUPERVISOR_INSTANCE_ID},
};
use exonum_derive::*;
use exonum_merkledb::BinaryValue;
use exonum_proto::{impl_binary_value_for_pb_message, ProtobufConvert};
use exonum_rust_runtime::{CallContext, DefaultInstance, Service, TxStub};

#[exonum_interface]
pub trait ConfigUpdater<Ctx> {
    type Output;
    fn update_config(&self, ctx: Ctx, arg: TxConfig) -> Self::Output;
}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_dispatcher(implements("ConfigUpdater"))]
#[service_factory(
    artifact_name = "config_updater",
    artifact_version = "0.1.0",
    proto_sources = "crate::proto"
)]
pub struct ConfigUpdaterService;

impl ConfigUpdater<CallContext<'_>> for ConfigUpdaterService {
    type Output = Result<(), ExecutionError>;

    fn update_config(&self, ctx: CallContext<'_>, arg: TxConfig) -> Self::Output {
        ctx.writeable_core_schema()
            .consensus_config_entry()
            .set(ConsensusConfig::from_bytes(arg.config.into()).unwrap());
        Ok(())
    }
}

impl Service for ConfigUpdaterService {}

impl ConfigUpdaterService {
    pub const ID: InstanceId = SUPERVISOR_INSTANCE_ID;
}

impl DefaultInstance for ConfigUpdaterService {
    const INSTANCE_ID: InstanceId = ConfigUpdaterService::ID;
    const INSTANCE_NAME: &'static str = "config-updater";
}

impl TxConfig {
    pub fn create_signed(
        from: PublicKey,
        config: &[u8],
        actual_from: Height,
        signer: &SecretKey,
    ) -> Verified<AnyTx> {
        let mut msg = TxConfig::new();
        msg.set_from(from.to_pb());
        msg.set_config(config.to_vec());
        msg.set_actual_from(actual_from.0);
        TxStub
            .update_config(ConfigUpdaterService::ID, msg)
            .sign(from, signer)
    }
}

impl_binary_value_for_pb_message! { TxConfig }
