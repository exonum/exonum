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
    Database, Entry, Error as StorageError, Fork, ListIndex, ObjectHash, TemporaryDB,
};
use futures::sync::mpsc;

use std::sync::Mutex;

use crate::{
    blockchain::{Blockchain, InstanceCollection, Schema},
    crypto,
    helpers::{generate_testnet_config, Height, ValidatorId},
    node::ApiSender,
    proto::schema::tests::*,
    runtime::{
        ServiceInstanceId,
        error::ErrorKind,
        rust::{RustArtifactId, Service, ServiceFactory, Transaction, TransactionContext},
        ArtifactInfo, ExecutionError,
    },
};

const IDX_NAME: &str = "idx_name";
const TEST_SERVICE_ID: ServiceInstanceId = 255;

#[exonum_service(crate = "crate", dispatcher = "TestServiceImpl")]
trait TestService {
    fn tx(&self, context: TransactionContext, arg: Tx) -> Result<(), ExecutionError>;
}

#[derive(Debug)]
struct TestServiceImpl;

impl TestService for TestServiceImpl {
    fn tx(&self, context: TransactionContext, arg: Tx) -> Result<(), ExecutionError> {
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

impl ServiceFactory for TestServiceImpl {
    fn artifact_id(&self) -> RustArtifactId {
        RustArtifactId::new("test_service", 1, 0, 0)
    }

    fn artifact_info(&self) -> ArtifactInfo {
        ArtifactInfo::default()
    }

    fn create_instance(&self) -> Box<dyn Service> {
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

#[exonum_service(crate = "crate", dispatcher = "ServiceGoodImpl")]
trait ServiceGood {}

#[derive(Debug)]
struct ServiceGoodImpl;

impl ServiceGood for ServiceGoodImpl {}

impl Service for ServiceGoodImpl {
    fn before_commit(&self, context: TransactionContext) {
        let mut index = ListIndex::new(IDX_NAME, context.fork());
        index.push(1);
    }
}

impl ServiceFactory for ServiceGoodImpl {
    fn artifact_id(&self) -> RustArtifactId {
        RustArtifactId::new("good_service", 1, 0, 0)
    }

    fn artifact_info(&self) -> ArtifactInfo {
        ArtifactInfo::default()
    }

    fn create_instance(&self) -> Box<dyn Service> {
        Box::new(Self)
    }
}

#[exonum_service(crate = "crate", dispatcher = "ServicePanicImpl")]
trait ServicePanic {}

#[derive(Debug)]
struct ServicePanicImpl;

impl ServicePanic for ServicePanicImpl {}

impl Service for ServicePanicImpl {
    fn before_commit(&self, _context: TransactionContext) {
        panic!("42");
    }
}

impl ServiceFactory for ServicePanicImpl {
    fn artifact_id(&self) -> RustArtifactId {
        RustArtifactId::new("panic_service", 1, 0, 0)
    }

    fn artifact_info(&self) -> ArtifactInfo {
        ArtifactInfo::default()
    }

    fn create_instance(&self) -> Box<dyn Service> {
        Box::new(Self)
    }
}

#[exonum_service(crate = "crate", dispatcher = "ServicePanicStorageErrorImpl")]
trait ServicePanicStorageError {}

#[derive(Debug)]
struct ServicePanicStorageErrorImpl;

impl ServicePanicStorageError for ServicePanicStorageErrorImpl {}

impl Service for ServicePanicStorageErrorImpl {
    fn before_commit(&self, _context: TransactionContext) {
        panic!(StorageError::new("42"));
    }
}

impl ServiceFactory for ServicePanicStorageErrorImpl {
    fn artifact_id(&self) -> RustArtifactId {
        RustArtifactId::new("storage_error_service", 1, 0, 0)
    }

    fn artifact_info(&self) -> ArtifactInfo {
        ArtifactInfo::default()
    }

    fn create_instance(&self) -> Box<dyn Service> {
        Box::new(Self)
    }
}

const TX_CHECK_RESULT_SERVICE_ID: ServiceInstanceId = 255;

lazy_static! {
    static ref EXECUTION_STATUS: Mutex<Result<(), ExecutionError>> = Mutex::new(Ok(()));
}

#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert)]
#[exonum(pb = "TestServiceTx", crate = "crate")]
struct TxResult {
    value: u64,
}

#[derive(Debug)]
struct TxResultCheckService;

#[exonum_service(crate = "crate", dispatcher = "TxResultCheckService")]
trait TxResultCheckInterface {
    fn tx_result(&self, context: TransactionContext, arg: TxResult) -> Result<(), ExecutionError>;
}

impl TxResultCheckInterface for TxResultCheckService {
    fn tx_result(&self, context: TransactionContext, arg: TxResult) -> Result<(), ExecutionError> {
        let mut entry = create_entry(context.fork());
        entry.set(arg.value);
        EXECUTION_STATUS.lock().unwrap().clone()
    }
}

impl Service for TxResultCheckService {}

impl ServiceFactory for TxResultCheckService {
    fn artifact_id(&self) -> RustArtifactId {
        RustArtifactId::new("good_service", 1, 0, 0)
    }

    fn artifact_info(&self) -> ArtifactInfo {
        ArtifactInfo::default()
    }

    fn create_instance(&self) -> Box<dyn Service> {
        Box::new(Self)
    }
}

fn create_entry(fork: &Fork) -> Entry<&Fork, u64> {
    Entry::new("transaction_status_test", fork)
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
    let config = generate_testnet_config(1, 0)[0].clone();
    let service_keypair = config.service_keypair();
    let api_channel = mpsc::unbounded();
    let internal_sender = mpsc::channel(1).0;

    Blockchain::new(
        TemporaryDB::new(),
        vec![InstanceCollection::new(factory).with_instance(id, name, ())],
        config.genesis,
        service_keypair,
        ApiSender::new(api_channel.0),
        internal_sender,
    )
}

#[test]
fn handling_tx_panic_error() {
    let mut blockchain = create_blockchain();

    let (pk, sec_key) = crypto::gen_keypair();
    let tx_ok1 = Tx::new(3).sign(TEST_SERVICE_ID, pk, &sec_key);
    let tx_ok2 = Tx::new(4).sign(TEST_SERVICE_ID, pk, &sec_key);
    let tx_failed = Tx::new(0).sign(TEST_SERVICE_ID, pk, &sec_key);
    let tx_storage_error = Tx::new(42).sign(TEST_SERVICE_ID, pk, &sec_key);

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

    let (pk, sec_key) = crypto::gen_keypair();
    let tx_ok1 = Tx::new(3).sign(TEST_SERVICE_ID, pk, &sec_key);
    let tx_ok2 = Tx::new(4).sign(TEST_SERVICE_ID, pk, &sec_key);
    let tx_failed = Tx::new(0).sign(TEST_SERVICE_ID, pk, &sec_key);
    let tx_storage_error = Tx::new(42).sign(TEST_SERVICE_ID, pk, &sec_key);

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
    let blockchain = create_blockchain_with_service(ServiceGoodImpl, 3, "service_good");
    let mut db = TemporaryDB::new();
    assert_service_execute(&blockchain, &mut db);
}

#[test]
fn service_execute_panic() {
    let blockchain = create_blockchain_with_service(ServicePanicImpl, 4, "service_panic");
    let mut db = TemporaryDB::new();
    assert_service_execute_panic(&blockchain, &mut db);
}

#[test]
#[should_panic]
fn service_execute_panic_storage_error() {
    let blockchain =
        create_blockchain_with_service(ServicePanicStorageErrorImpl, 5, "service_execute_error");
    let mut db = TemporaryDB::new();
    assert_service_execute_panic(&blockchain, &mut db);
}

#[test]
fn error_discards_transaction_changes() {
    let statuses = [
        Err(ExecutionError::new(ErrorKind::service(0), "")),
        Err(ExecutionError::new(ErrorKind::dispatcher(5), "Foo")),
        Err(ExecutionError::new(ErrorKind::runtime(0), "Strange bar")),
        Err(ExecutionError::new(ErrorKind::Panic, "PANIC")),
        Ok(()),
    ];

    let (pk, sec_key) = crypto::gen_keypair();
    let mut blockchain = create_blockchain_with_service(
        TxResultCheckService,
        TX_CHECK_RESULT_SERVICE_ID,
        "check_result",
    );
    let db = TemporaryDB::new();

    for (index, status) in statuses.iter().enumerate() {
        let index = index as u64;

        *EXECUTION_STATUS.lock().unwrap() = status.clone();

        let transaction = TxResult { value: index }.sign(TX_CHECK_RESULT_SERVICE_ID, pk, &sec_key);
        let hash = transaction.object_hash();
        {
            let fork = blockchain.fork();
            {
                let mut schema = Schema::new(&fork);
                schema.add_transaction_into_pool(transaction.clone());
            }
            blockchain.merge(fork.into_patch()).unwrap();
        }

        let (_, patch) = blockchain.create_patch(ValidatorId::zero(), Height(index), &[hash]);

        db.merge(patch).unwrap();

        let fork = db.fork();
        let entry = create_entry(&fork);
        if status.is_err() {
            assert_eq!(None, entry.get());
        } else {
            assert_eq!(Some(index), entry.get());
        }
    }
}
