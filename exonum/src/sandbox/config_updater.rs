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

pub use crate::proto::schema::tests::TxConfig;

use exonum_merkledb::BinaryValue;
use exonum_proto::ProtobufConvert;

use crate::{
    blockchain::{ConsensusConfig, ExecutionError, Schema},
    crypto::{Hash, PublicKey, SecretKey},
    helpers::Height,
    merkledb::Snapshot,
    messages::{AnyTx, Verified},
    runtime::{
        rust::{Service, Transaction, TransactionContext},
        InstanceDescriptor, InstanceId,
    },
};

#[exonum_service(crate = "crate")]
pub trait ConfigUpdaterInterface {
    fn update_config(
        &self,
        context: TransactionContext,
        arg: TxConfig,
    ) -> Result<(), ExecutionError>;
}

#[derive(Debug, ServiceFactory)]
#[exonum(
    crate = "crate",
    artifact_name = "config_updater",
    artifact_version = "0.1.0",
    proto_sources = "crate::proto::schema",
    implements("ConfigUpdaterInterface")
)]
pub struct ConfigUpdaterService;

impl ConfigUpdaterInterface for ConfigUpdaterService {
    fn update_config(
        &self,
        context: TransactionContext,
        arg: TxConfig,
    ) -> Result<(), ExecutionError> {
        Schema::new(context.fork())
            .consensus_config_entry()
            .set(ConsensusConfig::from_bytes(arg.config.into()).unwrap());
        Ok(())
    }
}

impl Service for ConfigUpdaterService {
    fn state_hash(&self, _instance: InstanceDescriptor, _snapshot: &dyn Snapshot) -> Vec<Hash> {
        vec![]
    }
}

impl ConfigUpdaterService {
    pub const ID: InstanceId = 2;
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
        msg.sign(ConfigUpdaterService::ID, from, signer)
    }
}

impl_binary_value_for_pb_message! { TxConfig }
