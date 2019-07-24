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
use semver::Version;

use crate::{
    blockchain::{ExecutionError, Schema, StoredConfiguration},
    crypto::{PublicKey, SecretKey},
    helpers::Height,
    messages::{AnyTx, Verified},
    proto::{schema::PROTO_SOURCES, ProtobufConvert},
    runtime::{
        rust::{RustArtifactId, Service, ServiceFactory, Transaction, TransactionContext},
        ArtifactInfo, ServiceInstanceId,
    },
};

#[exonum_service(crate = "crate", dispatcher = "ConfigUpdaterService")]
pub trait ConfigUpdaterInterface {
    fn update_config(
        &self,
        context: TransactionContext,
        arg: TxConfig,
    ) -> Result<(), ExecutionError>;
}

#[derive(Debug)]
pub struct ConfigUpdaterService;

impl ConfigUpdaterInterface for ConfigUpdaterService {
    fn update_config(
        &self,
        context: TransactionContext,
        arg: TxConfig,
    ) -> Result<(), ExecutionError> {
        let mut schema = Schema::new(context.fork());
        schema
            .commit_configuration(StoredConfiguration::try_deserialize(arg.get_config()).unwrap());
        Ok(())
    }
}

impl Service for ConfigUpdaterService {}

impl ServiceFactory for ConfigUpdaterService {
    fn artifact_id(&self) -> RustArtifactId {
        RustArtifactId {
            name: "config_updater".into(),
            version: Version::new(0, 1, 0),
        }
    }

    fn artifact_info(&self) -> ArtifactInfo {
        ArtifactInfo {
            proto_sources: PROTO_SOURCES.as_ref(),
        }
    }

    fn create_instance(&self) -> Box<dyn Service> {
        Box::new(Self)
    }
}

impl ConfigUpdaterService {
    pub const ID: ServiceInstanceId = 2;
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
