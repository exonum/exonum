// Copyright 2020 The Exonum Team
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

//! Simple timestamping service implementation.

use exonum::{
    crypto::KeyPair,
    merkledb::ObjectHash,
    runtime::{ExecutionContext, ExecutionError, SnapshotExt},
};
use exonum_derive::{exonum_interface, ServiceDispatcher, ServiceFactory};
use exonum_rust_runtime::{spec::Spec, Service};
use exonum_testkit::TestKitBuilder;

#[exonum_interface(auto_ids)]
trait TimestampingInterface<Ctx> {
    type Output;
    fn timestamp(&self, ctx: Ctx, arg: String) -> Self::Output;
}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_factory(artifact_name = "timestamping", artifact_version = "1.0.0")]
#[service_dispatcher(implements("TimestampingInterface"))]
struct TimestampingService;

impl TimestampingInterface<ExecutionContext<'_>> for TimestampingService {
    type Output = Result<(), ExecutionError>;

    fn timestamp(&self, _ctx: ExecutionContext<'_>, _arg: String) -> Self::Output {
        Ok(())
    }
}

impl Service for TimestampingService {}

fn main() {
    let instance_id = 512;
    // Create a testkit for a network with four validators.
    let service = TimestampingService;
    let mut testkit = TestKitBuilder::validator()
        .with_validators(4)
        .with(Spec::new(service).with_instance(instance_id, "timestamping", ()))
        .build();
    // Create few transactions.
    let keypair = KeyPair::random();
    let tx1 = keypair.timestamp(instance_id, "Down To Earth".to_owned());
    let tx2 = keypair.timestamp(instance_id, "Cry Over Spilt Milk".to_owned());
    let tx3 = keypair.timestamp(instance_id, "Dropping Like Flies".to_owned());

    // Commit them into blockchain.
    let block = testkit.create_block_with_transactions(vec![tx1.clone(), tx2.clone(), tx3.clone()]);
    assert_eq!(block.len(), 3);
    assert!(block.iter().all(|transaction| transaction.status().is_ok()));

    // Check results with the core schema.
    let snapshot = testkit.snapshot();
    let schema = snapshot.for_core();
    assert!(schema.transactions().contains(&tx1.object_hash()));
    assert!(schema.transactions().contains(&tx2.object_hash()));
    assert!(schema.transactions().contains(&tx3.object_hash()));
}
