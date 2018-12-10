// Copyright 2018 The Exonum Team
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

pub use proto::schema::tests::TxConfig;

use blockchain::{
    ExecutionResult, Schema, Service, StoredConfiguration, Transaction, TransactionContext,
    TransactionSet,
};
use crypto::{Hash, PublicKey, SecretKey};
use helpers::Height;
use messages::{Message, RawTransaction, Signed};
use proto::ProtobufConvert;
use storage::Snapshot;

pub const CONFIG_SERVICE: u16 = 1;

#[derive(Serialize, Deserialize, Clone, Debug, TransactionSet)]
#[exonum(crate = "crate")]
enum ConfigUpdaterTransactions {
    TxConfig(TxConfig),
}

impl TxConfig {
    pub fn create_signed(
        from: &PublicKey,
        config: &[u8],
        actual_from: Height,
        signer: &SecretKey,
    ) -> Signed<RawTransaction> {
        let mut msg = TxConfig::new();
        msg.set_from(from.to_pb());
        msg.set_config(config.to_vec());
        msg.set_actual_from(actual_from.0);

        Message::sign_transaction(msg, CONFIG_SERVICE, *from, signer)
    }
}
#[derive(Default)]
pub struct ConfigUpdateService {}

impl ConfigUpdateService {
    pub fn new() -> Self {
        ConfigUpdateService::default()
    }
}

impl Transaction for TxConfig {
    fn execute(&self, mut tc: TransactionContext) -> ExecutionResult {
        let mut schema = Schema::new(tc.fork());
        schema
            .commit_configuration(StoredConfiguration::try_deserialize(self.get_config()).unwrap());
        Ok(())
    }
}

impl Service for ConfigUpdateService {
    fn service_name(&self) -> &str {
        "sandbox_config_updater"
    }

    fn service_id(&self) -> u16 {
        CONFIG_SERVICE
    }

    fn state_hash(&self, _: &dyn Snapshot) -> Vec<Hash> {
        vec![]
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<dyn Transaction>, failure::Error> {
        let tx = ConfigUpdaterTransactions::tx_from_raw(raw)?;
        Ok(tx.into())
    }
}
