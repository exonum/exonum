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

#[macro_use]
extern crate exonum;
#[macro_use]
extern crate exonum_testkit;
extern crate serde_json;

use exonum::crypto::{gen_keypair, Hash, PublicKey, CryptoHash};
use exonum::blockchain::{Block, Schema, Service, Transaction, ExecutionStatus};
use exonum::messages::{Message, RawTransaction};
use exonum::storage::{Fork, Snapshot};
use exonum::encoding;
use exonum_testkit::{ApiKind, TestKitBuilder};

// Simple service implementation.

const SERVICE_ID: u16 = 512;
const TX_TIMESTAMP_ID: u16 = 0;

message! {
    struct TxTimestamp {
        const TYPE = SERVICE_ID;
        const ID = TX_TIMESTAMP_ID;

        from: &PublicKey,
        msg: &str,
    }
}

struct TimestampingService;

impl Transaction for TxTimestamp {
    fn verify(&self) -> bool {
        self.verify_signature(self.from())
    }

    fn execute(&self, _fork: &mut Fork) -> ExecutionStatus {
        Ok(())
    }
}

impl Service for TimestampingService {
    fn service_name(&self) -> &'static str {
        "timestamping"
    }

    fn state_hash(&self, _: &Snapshot) -> Vec<Hash> {
        Vec::new()
    }

    fn service_id(&self) -> u16 {
        SERVICE_ID
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, encoding::Error> {
        let trans: Box<Transaction> = match raw.message_type() {
            TX_TIMESTAMP_ID => Box::new(TxTimestamp::from_raw(raw)?),
            _ => {
                return Err(encoding::Error::IncorrectMessageType {
                    message_type: raw.message_type(),
                });
            }
        };
        Ok(trans)
    }
}

fn main() {
    // Create testkit for network with four validators.
    let mut testkit = TestKitBuilder::validator()
        .with_validators(4)
        .with_service(TimestampingService)
        .create();
    // Create few transactions.
    let keypair = gen_keypair();
    let tx1 = TxTimestamp::new(&keypair.0, "Down To Earth", &keypair.1);
    let tx2 = TxTimestamp::new(&keypair.0, "Cry Over Spilt Milk", &keypair.1);
    let tx3 = TxTimestamp::new(&keypair.0, "Dropping Like Flies", &keypair.1);
    // Commit them into blockchain.
    testkit.create_block_with_transactions(txvec![tx1.clone(), tx2.clone(), tx3.clone()]);
    // Check results with schema.
    let snapshot = testkit.snapshot();
    let schema = Schema::new(&snapshot);
    assert!(schema.transactions().contains(&tx1.hash()));
    assert!(schema.transactions().contains(&tx2.hash()));
    assert!(schema.transactions().contains(&tx3.hash()));
    // Check results with api.
    let api = testkit.api();
    let blocks: Vec<Block> = api.get(ApiKind::Explorer, "v1/blocks?count=10");
    assert_eq!(blocks.len(), 2);
    api.get::<serde_json::Value>(
        ApiKind::System,
        &format!("v1/transactions/{}", tx1.hash().to_string()),
    );
}
