// Copyright 2017 The Exonum Team
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

use exonum::crypto::{PublicKey, Hash};
use exonum::blockchain::{Service, Transaction, Schema};
use exonum::messages::{RawTransaction, Message, FromRaw};
use exonum::storage::{Snapshot, Fork};
use exonum::encoding::Error as MessageError;
use exonum::blockchain::StoredConfiguration;
use exonum::helpers::Height;

pub const CONFIG_SERVICE: u16 = 1;
pub const CONFIG_PROPOSE_MESSAGE_ID: u16 = 0;

message! {
    struct TxConfig {
        const TYPE = CONFIG_SERVICE;
        const ID = CONFIG_PROPOSE_MESSAGE_ID;
        const SIZE = 48;

        field from:               &PublicKey  [00 => 32]
        field config:             &[u8]       [32 => 40]
        field actual_from:        Height      [40 => 48]
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
    fn verify(&self) -> bool {
        self.verify_signature(self.from())
    }

    fn execute(&self, fork: &mut Fork) {
        let mut schema = Schema::new(fork);
        schema.commit_configuration(StoredConfiguration::try_deserialize(self.config()).unwrap())
    }
}

impl Service for ConfigUpdateService {
    fn service_name(&self) -> &'static str {
        "sandbox_config_updater"
    }

    fn service_id(&self) -> u16 {
        CONFIG_SERVICE
    }

    fn state_hash(&self, _: &Snapshot) -> Vec<Hash> {
        vec![]
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, MessageError> {
        if raw.message_type() != CONFIG_PROPOSE_MESSAGE_ID {
            return Err(MessageError::IncorrectMessageType {
                message_type: raw.message_type(),
            });
        }
        TxConfig::from_raw(raw).map(|tx| Box::new(tx) as Box<Transaction>)
    }
}
