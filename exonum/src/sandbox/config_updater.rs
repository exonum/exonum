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

use exonum_merkledb::{impl_binary_value_for_message, BinaryValue};
use protobuf::Message as PbMessage;
use semver::Version;

use std::borrow::Cow;

use crate::{
    blockchain::{ExecutionResult, Schema, StoredConfiguration},
    crypto::{PublicKey, SecretKey},
    helpers::Height,
    messages::{AnyTx, ServiceInstanceId, Signed},
    proto::ProtobufConvert,
    runtime::rust::{RustArtifactSpec, Service, ServiceFactory, Transaction, TransactionContext},
};

#[service_interface(exonum(crate = "crate"))]
pub trait ConfigUpdaterInterface {
    fn update_config(&self, context: TransactionContext, arg: TxConfig) -> ExecutionResult;
}

#[derive(Debug)]
pub struct ConfigUpdaterService;

impl_service_dispatcher!(ConfigUpdaterService, ConfigUpdaterInterface);

impl ConfigUpdaterInterface for ConfigUpdaterService {
    fn update_config(&self, context: TransactionContext, arg: TxConfig) -> ExecutionResult {
        let mut schema = Schema::new(context.fork());
        schema
            .commit_configuration(StoredConfiguration::try_deserialize(arg.get_config()).unwrap());
        Ok(())
    }
}

impl Service for ConfigUpdaterService {}

impl ServiceFactory for ConfigUpdaterService {
    fn artifact(&self) -> RustArtifactSpec {
        RustArtifactSpec {
            name: "config_updater".into(),
            version: Version::new(0, 1, 0),
        }
    }

    fn new_instance(&self) -> Box<dyn Service> {
        Box::new(Self)
    }
}

impl ConfigUpdaterService {
    pub const ID: ServiceInstanceId = 3;
}

impl TxConfig {
    pub fn create_signed(
        from: &PublicKey,
        config: &[u8],
        actual_from: Height,
        signer: &SecretKey,
    ) -> Signed<AnyTx> {
        let mut msg = TxConfig::new();
        msg.set_from(from.to_pb());
        msg.set_config(config.to_vec());
        msg.set_actual_from(actual_from.0);
        msg.sign(ConfigUpdaterService::ID, *from, signer)
    }
}

impl_binary_value_for_message! { TxConfig }
