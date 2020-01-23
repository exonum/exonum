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
pub trait ConfigUpdater<Ctx> {
    type Output;
    fn update_config(&self, ctx: Ctx, arg: TxConfig) -> Self::Output;
}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_dispatcher(implements("ConfigUpdater"))]
#[service_factory(artifact_name = "config_updater", artifact_version = "0.1.0")]
pub struct ConfigUpdaterService;

impl ConfigUpdater<ExecutionContext<'_>> for ConfigUpdaterService {
    type Output = Result<(), ExecutionError>;

    fn update_config(&self, mut ctx: ExecutionContext<'_>, arg: TxConfig) -> Self::Output {
        ctx.supervisor_extensions()
            .writeable_core_schema()
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
        let msg = TxConfig {
            from,
            config: config.to_owned(),
            actual_from,
        };

        TxStub
            .update_config(ConfigUpdaterService::ID, msg)
            .sign(from, signer)
    }
}
