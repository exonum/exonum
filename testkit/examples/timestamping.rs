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

#[macro_use]
extern crate exonum;
#[macro_use]
extern crate exonum_testkit;
extern crate serde_json;

use exonum::api::public::BlocksRange;
use exonum::blockchain::{ExecutionResult, Schema, Service, Transaction, TransactionSet};
use exonum::crypto::{gen_keypair, CryptoHash, Hash, PublicKey};
use exonum::encoding;
use exonum::messages::{Message, RawTransaction};
use exonum::storage::{Fork, Snapshot};
use exonum_testkit::{ApiKind, TestKitBuilder};

// Simple service implementation.

const SERVICE_ID: u16 = 512;

transactions! {
    TimestampingServiceTransactions {
        const SERVICE_ID = SERVICE_ID;

        struct TxTimestamp {
            from: &PublicKey,
            msg: &str,
        }
    }
}

struct TimestampingService;

impl Transaction for TxTimestamp {
    fn verify(&self) -> bool {
        self.verify_signature(self.from())
    }

    fn execute(&self, _fork: &mut Fork) -> ExecutionResult {
        Ok(())
    }
}

impl Service for TimestampingService {
    fn service_name(&self) -> &str {
        "timestamping"
    }

    fn state_hash(&self, _: &Snapshot) -> Vec<Hash> {
        Vec::new()
    }

    fn service_id(&self) -> u16 {
        SERVICE_ID
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, encoding::Error> {
        let tx = TimestampingServiceTransactions::tx_from_raw(raw)?;
        Ok(tx.into())
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
    let block =
        testkit.create_block_with_transactions(txvec![tx1.clone(), tx2.clone(), tx3.clone(),]);
    assert_eq!(block.len(), 3);
    assert!(block.iter().all(|transaction| transaction.status().is_ok()));

    // Check results with schema.
    let snapshot = testkit.snapshot();
    let schema = Schema::new(&snapshot);
    assert!(schema.transactions().contains(&tx1.hash()));
    assert!(schema.transactions().contains(&tx2.hash()));
    assert!(schema.transactions().contains(&tx3.hash()));

    // Check results with api.
    let api = testkit.api();
    let blocks_range: BlocksRange = api.get(ApiKind::Explorer, "v1/blocks?count=10");
    assert_eq!(blocks_range.blocks.len(), 2);
    api.get::<serde_json::Value>(
        ApiKind::Explorer,
        &format!("v1/transactions/{}", tx1.hash().to_string()),
    );
}
