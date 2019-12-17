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

use crate::{proto, TestKit, TestKitApi, TestKitBuilder};
use exonum::{
    crypto::gen_keypair,
    explorer::CommittedTransaction,
    messages::{AnyTx, Verified},
    runtime::{
        rust::{CallContext, Service, ServiceFactory, Transaction},
        ExecutionError, SnapshotExt,
    },
};
use exonum_merkledb::ObjectHash;
use exonum_proto::ProtobufConvert;

const SERVICE_ID: u32 = 2;
const SERVICE_NAME: &str = "sample";

#[derive(Clone, Debug)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::examples::TxTimestamp")]
struct TxTimestamp {
    message: String,
}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_factory(artifact_name = "sample-service", proto_sources = "crate::proto")]
#[service_dispatcher(implements("SampleServiceInterface"))]
struct SampleService;

#[exonum_interface]
trait SampleServiceInterface {
    fn timestamp(&self, context: CallContext<'_>, arg: TxTimestamp) -> Result<(), ExecutionError>;
}

impl SampleServiceInterface for SampleService {
    fn timestamp(
        &self,
        _context: CallContext<'_>,
        _arg: TxTimestamp,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }
}

impl Service for SampleService {}

fn init_testkit() -> (TestKit, TestKitApi) {
    let service = SampleService;
    let artifact = service.artifact_id();
    let mut testkit = TestKitBuilder::validator()
        .with_artifact(artifact.clone())
        .with_instance(artifact.into_default_instance(SERVICE_ID, SERVICE_NAME))
        .with_rust_service(service)
        .create();
    let api = testkit.api();
    (testkit, api)
}

fn gen_tx(message: String) -> Verified<AnyTx> {
    let (pubkey, key) = gen_keypair();
    TxTimestamp { message }.sign(SERVICE_ID, pubkey, &key)
}

fn gen_incorrect_tx(message: String) -> Verified<AnyTx> {
    let (pubkey, key) = gen_keypair();
    TxTimestamp { message }.sign(SERVICE_ID + 1, pubkey, &key)
}

#[test]
fn testkit_add_tx() {
    let (mut testkit, _) = init_testkit();
    let tx = gen_tx("tx".to_string());
    testkit.add_tx(tx.clone());
    assert!(testkit.is_tx_in_pool(&tx.object_hash()));
}

#[test]
#[should_panic(expected = "Attempt to add invalid tx in the pool")]
fn testkit_add_tx_incorrect_transaction() {
    let (mut testkit, _) = init_testkit();
    let incorrect_tx = gen_incorrect_tx("tx".to_string());
    testkit.add_tx(incorrect_tx);
}

#[test]
fn testkit_probe_all() {
    let (mut testkit, _) = init_testkit();
    let tx0 = gen_tx("tx0".to_string());
    let tx1 = gen_tx("tx1".to_string());
    let snapshot = testkit.probe_all(vec![tx0.clone(), tx1.clone()]);
    let schema = snapshot.for_core();
    assert!(
        schema.transactions().contains(&tx0.object_hash())
            && schema.transactions().contains(&tx1.object_hash())
    );
}

#[test]
#[should_panic(expected = "Attempt to add invalid tx in the pool")]
fn testkit_probe_all_incorrect_transaction() {
    let (mut testkit, _) = init_testkit();
    let incorrect_tx = gen_incorrect_tx("tx".to_string());
    testkit.probe_all(vec![incorrect_tx]);
}

#[test]
fn testkit_probe() {
    let (mut testkit, _) = init_testkit();
    let tx = gen_tx("tx".to_string());
    let snapshot = testkit.probe(tx.clone());
    let schema = snapshot.for_core();
    assert!(schema.transactions().contains(&tx.object_hash()));
}

#[test]
#[should_panic(expected = "Attempt to add invalid tx in the pool")]
fn testkit_probe_incorrect_transaction() {
    let (mut testkit, _) = init_testkit();
    let incorrect_tx = gen_incorrect_tx("tx".to_string());
    testkit.probe(incorrect_tx);
}

#[test]
fn testkit_create_block_with_transactions() {
    let (mut testkit, _) = init_testkit();
    let tx0 = gen_tx("tx0".to_string());
    let tx1 = gen_tx("tx1".to_string());
    let block = testkit.create_block_with_transactions(vec![tx0.clone(), tx1.clone()]);

    assert!(
        block
            .iter()
            .map(CommittedTransaction::content)
            .any(|tx| *tx == tx0),
        "Transaction tx0 should be in the block."
    );
    assert!(
        block
            .iter()
            .map(CommittedTransaction::content)
            .any(|tx| *tx == tx1),
        "Transaction tx1 should be in the block."
    );

    let snapshot = testkit.snapshot();
    let txs = snapshot.for_core().transactions();
    assert!(
        txs.contains(&tx0.object_hash()) && txs.contains(&tx1.object_hash()),
        "Transactions should be in scheme."
    );
}

#[test]
#[should_panic(expected = "Transaction is already committed")]
fn testkit_create_block_with_transactions_commit_duplicated_transaction() {
    let (mut testkit, _) = init_testkit();
    let tx = gen_tx("tx".to_string());
    testkit.create_block_with_transactions(vec![tx.clone()]);
    testkit.create_block_with_transactions(vec![tx.clone()]);
}

#[test]
#[should_panic(expected = "Attempt to add invalid tx in the pool")]
fn testkit_create_block_with_transactions_incorrect_transaction() {
    let (mut testkit, _) = init_testkit();
    let incorrect_tx = gen_incorrect_tx("tx".to_string());
    testkit.create_block_with_transactions(vec![incorrect_tx]);
}

#[test]
fn testkit_create_block_with_transaction() {
    let (mut testkit, _) = init_testkit();
    let tx = gen_tx("tx".to_string());
    let block = testkit.create_block_with_transaction(tx.clone());
    assert!(
        block
            .iter()
            .map(CommittedTransaction::content)
            .any(|in_tx| *in_tx == tx),
        "Transaction tx should be in the block."
    );

    let snapshot = testkit.snapshot();
    let txs = snapshot.for_core().transactions();
    assert!(
        txs.contains(&tx.object_hash()),
        "Transaction should be in scheme."
    );
}

#[test]
#[should_panic(expected = "Transaction is already committed")]
fn testkit_create_block_with_transaction_commit_duplicated_transaction() {
    let (mut testkit, _) = init_testkit();
    let tx = gen_tx("tx".to_string());
    testkit.create_block_with_transaction(tx.clone());
    testkit.create_block_with_transaction(tx.clone());
}

#[test]
#[should_panic(expected = "Attempt to add invalid tx in the pool")]
fn testkit_create_block_with_transaction_incorrect_transaction() {
    let (mut testkit, _) = init_testkit();
    let incorrect_tx = gen_incorrect_tx("tx".to_string());
    testkit.create_block_with_transaction(incorrect_tx);
}
