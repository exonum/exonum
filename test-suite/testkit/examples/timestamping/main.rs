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

#[macro_use]
extern crate exonum_testkit;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate exonum_derive;

use exonum::{
    api::node::public::explorer::{BlocksQuery, BlocksRange, TransactionQuery},
    blockchain::{ExecutionError, Schema},
    crypto::{gen_keypair, Hash},
    runtime::{
        rust::{CallContext, Service, Transaction},
        InstanceDescriptor,
    },
};
use exonum_merkledb::{ObjectHash, Snapshot};
use exonum_proto::ProtobufConvert;
use exonum_testkit::{ApiKind, InstanceCollection, TestKitBuilder};

mod proto;

// Simple service implementation.

#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::TxTimestamp")]
struct TxTimestamp {
    message: String,
}

impl TxTimestamp {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[exonum_service]
trait TimestampingInterface {
    fn timestamp(&self, context: CallContext<'_>, arg: TxTimestamp) -> Result<(), ExecutionError>;
}

#[derive(Debug, ServiceFactory)]
#[exonum(
    artifact_name = "timestamping",
    artifact_version = "1.0.0",
    proto_sources = "crate::proto",
    implements("TimestampingInterface")
)]
struct TimestampingService;

impl TimestampingInterface for TimestampingService {
    fn timestamp(
        &self,
        _context: CallContext<'_>,
        _arg: TxTimestamp,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }
}

impl Service for TimestampingService {
    fn state_hash(&self, _instance: InstanceDescriptor<'_>, _snapshot: &dyn Snapshot) -> Vec<Hash> {
        vec![]
    }
}

fn main() {
    let instance_id = 512;
    // Create a testkit for a network with four validators.
    let mut testkit = TestKitBuilder::validator()
        .with_validators(4)
        .with_rust_service(InstanceCollection::new(TimestampingService).with_instance(
            instance_id,
            "timestamping",
            (),
        ))
        .create();
    // Create few transactions.
    let keypair = gen_keypair();
    let tx1 = TxTimestamp::new("Down To Earth").sign(instance_id, keypair.0, &keypair.1);
    let tx2 = TxTimestamp::new("Cry Over Spilt Milk").sign(instance_id, keypair.0, &keypair.1);
    let tx3 = TxTimestamp::new("Dropping Like Flies").sign(instance_id, keypair.0, &keypair.1);

    // Commit them into blockchain.
    let block =
        testkit.create_block_with_transactions(txvec![tx1.clone(), tx2.clone(), tx3.clone(),]);
    assert_eq!(block.len(), 3);
    assert!(block.iter().all(|transaction| transaction.status().is_ok()));

    // Check results with schema.
    let snapshot = testkit.snapshot();
    let schema = Schema::new(&snapshot);
    assert!(schema.transactions().contains(&tx1.object_hash()));
    assert!(schema.transactions().contains(&tx2.object_hash()));
    assert!(schema.transactions().contains(&tx3.object_hash()));

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
        .query(&TransactionQuery {
            hash: tx1.object_hash(),
        })
        .get::<serde_json::Value>("v1/transactions")
        .unwrap();
}
