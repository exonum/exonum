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

//! Tests that different error descriptions in the service code do not cause consensus
//! divergence.

use exonum::{
    crypto::KeyPair,
    merkledb::ObjectHash,
    runtime::{ExecutionContext, ExecutionError, InstanceId},
};
use exonum_derive::*;
use exonum_rust_runtime::{DefaultInstance, Service};

use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

use exonum_testkit::{Spec, TestKit, TestKitBuilder};

#[exonum_interface(auto_ids)]
trait ErroneousInterface<Ctx> {
    type Output;
    fn generate_error(&self, context: Ctx, code: u8) -> Self::Output;
}

#[derive(Debug, Clone, Default, ServiceDispatcher, ServiceFactory)]
#[service_dispatcher(implements("ErroneousInterface"))]
#[service_factory(
    artifact_name = "erroneous-service",
    service_constructor = "Self::new_instance"
)]
struct ErroneousService(Arc<AtomicU64>);

impl ErroneousService {
    fn new_instance(&self) -> Box<dyn Service> {
        Box::new(self.clone())
    }
}

impl ErroneousInterface<ExecutionContext<'_>> for ErroneousService {
    type Output = Result<(), ExecutionError>;

    fn generate_error(&self, _context: ExecutionContext<'_>, code: u8) -> Self::Output {
        let count = self.0.fetch_add(1, Ordering::SeqCst);
        let description = format!("Error was generated {} times", count + 1);
        Err(ExecutionError::service(code, description))
    }
}

impl Service for ErroneousService {}

impl DefaultInstance for ErroneousService {
    const INSTANCE_ID: InstanceId = 100;
    const INSTANCE_NAME: &'static str = "erroneous";
}

fn init_testkit() -> TestKit {
    let service = ErroneousService::default();
    TestKitBuilder::validator()
        .with(Spec::new(service).with_default_instance())
        .build()
}

#[test]
fn diverging_error_descriptions() {
    let mut testkit = init_testkit();
    let keypair = KeyPair::random();
    let tx = keypair.generate_error(ErroneousService::INSTANCE_ID, 1);

    testkit.checkpoint();
    let block = testkit.create_block_with_transaction(tx.clone());
    assert_eq!(block.errors.len(), 1);
    assert_eq!(
        block.errors[0].error.description(),
        "Error was generated 1 times"
    );
    let block = block.header;

    testkit.rollback();
    let new_block = testkit.create_block_with_transaction(tx);
    assert_eq!(new_block.errors.len(), 1);
    assert_eq!(
        new_block.errors[0].error.description(),
        "Error was generated 2 times"
    );
    let new_block = new_block.header;

    assert_eq!(block.error_hash, new_block.error_hash);
    assert_eq!(block.object_hash(), new_block.object_hash());
}

#[test]
fn diverging_error_codes() {
    let mut testkit = init_testkit();
    let keypair = KeyPair::random();

    testkit.checkpoint();
    let tx = keypair.generate_error(ErroneousService::INSTANCE_ID, 1);
    let block = testkit.create_block_with_transaction(tx).header;

    testkit.rollback();
    let other_tx = keypair.generate_error(ErroneousService::INSTANCE_ID, 2);
    let new_block = testkit.create_block_with_transaction(other_tx).header;

    assert_ne!(block.error_hash, new_block.error_hash);
    assert_ne!(block.object_hash(), new_block.object_hash());
}
