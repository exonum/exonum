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

#![allow(dead_code, unsafe_code)]

use exonum_merkledb::{
    Database, DbOptions, Error as StorageError, Fork, ListIndex, ObjectHash, RocksDB, Snapshot,
    TemporaryDB,
};
use futures::sync::mpsc;
use rand::{distributions::Alphanumeric, thread_rng, Rng};

use crate::{
    blockchain::{Blockchain, ExecutionError, ExecutionResult, Schema},
    crypto::{gen_keypair, Hash},
    helpers::{Height, ValidatorId},
    impl_service_dispatcher,
    messages::{AnyTx, BinaryValue, Message, ServiceInstanceId, ServiceTransaction},
    node::ApiSender,
    proto::schema::tests::*,
    runtime::{
        dispatcher::DispatcherBuilder,
        rust::{RustArtifactSpec, Service, ServiceFactory, TransactionContext},
    },
};

const IDX_NAME: &str = "idx_name";
const TEST_SERVICE_ID: ServiceInstanceId = 255;

#[service_interface(exonum(crate = "crate"))]
trait TestService {
    fn tx(&self, context: TransactionContext, arg: Tx) -> ExecutionResult;
}

#[derive(Debug)]
struct TestServiceImpl;

impl TestService for TestServiceImpl {
    fn tx(&self, context: TransactionContext, arg: Tx) -> ExecutionResult {
        if arg.value == 42 {
            panic!(StorageError::new("42"))
        }
        let mut index = ListIndex::new(IDX_NAME, context.fork());
        index.push(arg.value);
        index.push(42 / arg.value);
        Ok(())
    }
}

impl Service for TestServiceImpl {}

impl_service_dispatcher!(TestServiceImpl, TestService);

impl ServiceFactory for TestServiceImpl {
    fn artifact(&self) -> RustArtifactSpec {
        RustArtifactSpec::new("test_service", 1, 0, 0)
    }

    fn new_instance(&self) -> Box<dyn Service> {
        Box::new(TestServiceImpl)
    }
}

#[derive(Serialize, Deserialize, ProtobufConvert, Debug, Clone)]
#[exonum(pb = "TestServiceTx", crate = "crate")]
struct Tx {
    value: u64,
}

impl Tx {
    fn new(value: u64) -> Self {
        Self { value }
    }
}

#[service_interface(exonum(crate = "crate"))]
trait ServiceGood {}

#[derive(Debug)]
struct ServiceGoodImpl;

impl ServiceGood for ServiceGoodImpl {}

impl Service for ServiceGoodImpl {
    fn before_commit(&self, fork: &Fork) {
        debug!("ServiceGood: before commit");
        let mut index = ListIndex::new(IDX_NAME, fork);
        index.push(1);
    }
}

impl_service_dispatcher!(ServiceGoodImpl, ServiceGood);

impl ServiceFactory for ServiceGoodImpl {
    fn artifact(&self) -> RustArtifactSpec {
        RustArtifactSpec::new("good_service", 1, 0, 0)
    }

    fn new_instance(&self) -> Box<dyn Service> {
        Box::new(Self)
    }
}

#[service_interface(exonum(crate = "crate"))]
trait ServicePanic {}

#[derive(Debug)]
struct ServicePanicImpl;

impl ServicePanic for ServicePanicImpl {}

impl Service for ServicePanicImpl {
    fn before_commit(&self, fork: &Fork) {
        panic!("42");
    }
}

impl_service_dispatcher!(ServicePanicImpl, ServicePanic);

impl ServiceFactory for ServicePanicImpl {
    fn artifact(&self) -> RustArtifactSpec {
        RustArtifactSpec::new("panic_service", 1, 0, 0)
    }

    fn new_instance(&self) -> Box<dyn Service> {
        Box::new(Self)
    }
}

#[service_interface(exonum(crate = "crate"))]
trait ServicePanicStorageError {}

#[derive(Debug)]
struct ServicePanicStorageErrorImpl;

impl ServicePanicStorageError for ServicePanicStorageErrorImpl {}

impl Service for ServicePanicStorageErrorImpl {
    fn before_commit(&self, _fork: &Fork) {
        panic!(StorageError::new("42"));
    }
}

impl_service_dispatcher!(ServicePanicStorageErrorImpl, ServicePanicStorageError);

impl ServiceFactory for ServicePanicStorageErrorImpl {
    fn artifact(&self) -> RustArtifactSpec {
        RustArtifactSpec::new("storage_error_service", 1, 0, 0)
    }

    fn new_instance(&self) -> Box<dyn Service> {
        Box::new(Self)
    }
}

fn assert_service_execute(blockchain: &Blockchain, db: &mut dyn Database) {
    let (_, patch) = blockchain.create_patch(ValidatorId::zero(), Height(1), &[]);
    db.merge(patch).unwrap();
    let snapshot = db.snapshot();
    let index = ListIndex::new(IDX_NAME, &snapshot);
    assert_eq!(index.len(), 1);
    assert_eq!(index.get(0), Some(1));
}

fn assert_service_execute_panic(blockchain: &Blockchain, db: &mut dyn Database) {
    let (_, patch) = blockchain.create_patch(ValidatorId::zero(), Height(1), &[]);
    db.merge(patch).unwrap();
    let snapshot = db.snapshot();
    let index: ListIndex<_, u32> = ListIndex::new(IDX_NAME, &snapshot);
    assert!(index.is_empty());
}

fn create_blockchain() -> Blockchain {
    create_blockchain_with_service(TestServiceImpl, TEST_SERVICE_ID, IDX_NAME)
}

fn create_blockchain_with_service(
    factory: impl Into<Box<dyn ServiceFactory>>,
    id: ServiceInstanceId,
    name: &str,
) -> Blockchain {
    let service_keypair = gen_keypair();
    let api_channel = mpsc::unbounded();
    let internal_sender = mpsc::channel(1).0;

    Blockchain::with_dispatcher(
        TemporaryDB::new(),
        DispatcherBuilder::new(internal_sender)
            .with_builtin_service(factory, id, name)
            .finalize(),
        service_keypair.0,
        service_keypair.1,
        ApiSender::new(api_channel.0),
    )
}

#[test]
fn handling_tx_panic() {
    let mut blockchain = create_blockchain();

    let (pk, sec_key) = gen_keypair();
    let tx_ok1 = Message::sign_transaction(Tx::new(3), TEST_SERVICE_ID, pk, &sec_key);
    let tx_ok2 = Message::sign_transaction(Tx::new(4), TEST_SERVICE_ID, pk, &sec_key);
    let tx_failed = Message::sign_transaction(Tx::new(0), TEST_SERVICE_ID, pk, &sec_key);
    let tx_storage_error = Message::sign_transaction(Tx::new(42), TEST_SERVICE_ID, pk, &sec_key);

    let patch = {
        let fork = blockchain.fork();
        {
            let mut schema = Schema::new(&fork);

            schema.add_transaction_into_pool(tx_ok1.clone());
            schema.add_transaction_into_pool(tx_ok2.clone());
            schema.add_transaction_into_pool(tx_failed.clone());
            schema.add_transaction_into_pool(tx_storage_error.clone());
        }
        fork.into_patch()
    };
    blockchain.merge(patch).unwrap();

    let (_, patch) = blockchain.create_patch(
        ValidatorId::zero(),
        Height::zero(),
        &[
            tx_ok1.object_hash(),
            tx_failed.object_hash(),
            tx_ok2.object_hash(),
        ],
    );

    blockchain.merge(patch).unwrap();
    let snapshot = blockchain.snapshot();

    let schema = Schema::new(&snapshot);
    assert_eq!(
        schema.transactions().get(&tx_ok1.object_hash()),
        Some(tx_ok1.clone())
    );
    assert_eq!(
        schema.transactions().get(&tx_ok2.object_hash()),
        Some(tx_ok2.clone())
    );
    assert_eq!(
        schema.transactions().get(&tx_failed.object_hash()),
        Some(tx_failed.clone())
    );

    let index = ListIndex::new(IDX_NAME, &snapshot);

    assert_eq!(index.len(), 4);
    assert_eq!(index.get(0), Some(3));
    assert_eq!(index.get(1), Some(14));
    assert_eq!(index.get(2), Some(4));
    assert_eq!(index.get(3), Some(10));
}

#[test]
#[should_panic]
fn handling_tx_panic_storage_error() {
    let mut blockchain = create_blockchain();

    let (pk, sec_key) = gen_keypair();
    let tx_ok1 = Message::sign_transaction(Tx::new(3), TEST_SERVICE_ID, pk, &sec_key);
    let tx_ok2 = Message::sign_transaction(Tx::new(4), TEST_SERVICE_ID, pk, &sec_key);
    let tx_failed = Message::sign_transaction(Tx::new(0), TEST_SERVICE_ID, pk, &sec_key);
    let tx_storage_error = Message::sign_transaction(Tx::new(42), TEST_SERVICE_ID, pk, &sec_key);

    let patch = {
        let fork = blockchain.fork();
        {
            let mut schema = Schema::new(&fork);
            schema.add_transaction_into_pool(tx_ok1.clone());
            schema.add_transaction_into_pool(tx_ok2.clone());
            schema.add_transaction_into_pool(tx_failed.clone());
            schema.add_transaction_into_pool(tx_storage_error.clone());
        }
        fork.into_patch()
    };
    blockchain.merge(patch).unwrap();
    blockchain.create_patch(
        ValidatorId::zero(),
        Height::zero(),
        &[
            tx_ok1.object_hash(),
            tx_storage_error.object_hash(),
            tx_ok2.object_hash(),
        ],
    );
}

#[test]
fn service_execute_good() {
    let blockchain = create_blockchain_with_service(ServiceGoodImpl, 1, "service_good");
    let mut db = TemporaryDB::new();
    assert_service_execute(&blockchain, &mut db);
}

#[test]
fn service_execute_panic() {
    let blockchain = create_blockchain_with_service(ServicePanicImpl, 1, "service_panic");
    let mut db = TemporaryDB::new();
    assert_service_execute_panic(&blockchain, &mut db);
}

#[test]
#[should_panic]
fn service_execute_panic_storage_error() {
    let blockchain =
        create_blockchain_with_service(ServicePanicStorageErrorImpl, 1, "service_execute_error");
    let mut db = TemporaryDB::new();
    assert_service_execute_panic(&blockchain, &mut db);
}
