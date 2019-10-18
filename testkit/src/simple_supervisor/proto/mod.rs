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

pub mod schema;

use exonum::{
    blockchain::ConsensusConfig,
    crypto,
    helpers::Height,
    merkledb::BinaryValue,
    messages::{AnyTx, Verified},
    runtime::{rust::Transaction, ConfigChange, InstanceId, ServiceConfig},
};
use exonum_proto_derive::ProtobufConvert;

use super::SimpleSupervisor;

#[derive(Debug, Clone, PartialEq, ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "schema::service::ConfigPropose")]
pub struct ConfigPropose {
    pub changes: Vec<ConfigChange>,
    pub actual_from: Height,
}

impl ConfigPropose {
    pub fn actual_from(height: Height) -> Self {
        Self {
            actual_from: height,
            changes: Vec::default(),
        }
    }

    pub fn consensus_config(mut self, config: ConsensusConfig) -> Self {
        self.changes.push(ConfigChange::Consensus(config));
        self
    }

    pub fn service_config(mut self, instance_id: InstanceId, config: impl BinaryValue) -> Self {
        self.changes.push(ConfigChange::Service(ServiceConfig {
            instance_id,
            params: config.into_bytes(),
        }));
        self
    }

    pub fn into_tx(self) -> Verified<AnyTx> {
        let keypair = crypto::gen_keypair();
        self.sign(SimpleSupervisor::BUILTIN_ID, keypair.0, &keypair.1)
    }
}
