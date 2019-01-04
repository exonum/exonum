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

extern crate exonum;
#[macro_use]
extern crate exonum_testkit;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate exonum_derive;
extern crate failure;
extern crate protobuf;

use exonum::{
    api::node::public::explorer::{BlocksQuery, BlocksRange, TransactionQuery},
    blockchain::{
        ExecutionResult, Schema, Service, Transaction, TransactionContext, TransactionSet,
    },
    crypto::{gen_keypair, Hash, PublicKey, SecretKey},
    messages::{Message, RawTransaction, Signed},
    storage::Snapshot,
};
use exonum_testkit::{ApiKind, TestKitBuilder};

mod proto;

// Simple service implementation.

const SERVICE_ID: u16 = 512;

#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert)]
#[exonum(pb = "proto::TxTimestamp")]
struct TxTimestamp {
    message: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, TransactionSet)]
enum TimestampingServiceTransactions {
    TxTimestamp(TxTimestamp),
}

impl TxTimestamp {
    fn sign(author: &PublicKey, message: &str, key: &SecretKey) -> Signed<RawTransaction> {
        Message::sign_transaction(
            Self {
                message: message.to_owned(),
            },
            SERVICE_ID,
            *author,
            key,
        )
    }
}

struct TimestampingService;

impl Transaction for TxTimestamp {
    fn execute(&self, _context: TransactionContext) -> ExecutionResult {
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

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, failure::Error> {
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
    let tx1 = TxTimestamp::sign(&keypair.0, "Down To Earth", &keypair.1);
    let tx2 = TxTimestamp::sign(&keypair.0, "Cry Over Spilt Milk", &keypair.1);
    let tx3 = TxTimestamp::sign(&keypair.0, "Dropping Like Flies", &keypair.1);

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
    let blocks_range: BlocksRange = api
        .public(ApiKind::Explorer)
        .query(&BlocksQuery {
            count: 10,
            ..Default::default()
        })
        .get("v1/blocks")
        .unwrap();
    assert_eq!(blocks_range.blocks.len(), 2);

    api.public(ApiKind::Explorer)
        .query(&TransactionQuery { hash: tx1.hash() })
        .get::<serde_json::Value>("v1/transactions")
        .unwrap();
}
