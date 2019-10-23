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

use futures::{sync::mpsc, Future};

use std::{collections::BTreeMap, panic, sync::Mutex};

use exonum_proto::ProtobufConvert;

use crate::{
    blockchain::{
        Blockchain, BlockchainBuilder, ExecutionErrorKind, ExecutionStatus, FatalError,
        InstanceCollection, Schema,
    },
    crypto::{self, Hash},
    helpers::{generate_testnet_config, Height, ValidatorId},
    merkledb::{
        BinaryValue, Database, Entry, Error as StorageError, Fork, IndexAccess, ListIndex,
        ObjectHash, Snapshot, TemporaryDB,
    },
    messages::Verified,
    node::ApiSender,
    proto::schema::tests::*,
    runtime::{
        dispatcher,
        error::ErrorKind,
        rust::{BeforeCommitContext, Service, ServiceFactory, Transaction, TransactionContext},
        AnyTx, ArtifactId, ExecutionError, InstanceDescriptor, InstanceId, SUPERVISOR_INSTANCE_ID,
    },
};

const IDX_NAME: &str = "idx_name";
const TEST_SERVICE_ID: InstanceId = SUPERVISOR_INSTANCE_ID;

#[derive(Serialize, Deserialize, Debug, Clone, ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "TestServiceTx")]
struct TestExecute {
    value: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "TestServiceTx")]
struct TestDeploy {
    value: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "TestServiceTx")]
struct TestAdd {
    value: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "TestServiceTx")]
struct TestCallInitialize {
    value: u64,
}

#[exonum_service(crate = "crate")]
trait TestDispatcherInterface {
    fn test_execute(
        &self,
        context: TransactionContext,
        arg: TestExecute,
    ) -> Result<(), ExecutionError>;

    fn test_deploy(
        &self,
        context: TransactionContext,
        arg: TestDeploy,
    ) -> Result<(), ExecutionError>;

    fn test_add(&self, context: TransactionContext, arg: TestAdd) -> Result<(), ExecutionError>;
}

#[derive(Debug, ServiceFactory)]
#[exonum(
    crate = "crate",
    artifact_name = "test_dispatcher",
    proto_sources = "crate::proto::schema",
    implements("TestDispatcherInterface")
)]
struct TestDispatcherService;

impl Service for TestDispatcherService {
    fn initialize(
        &self,
        _instance: InstanceDescriptor,
        _fork: &Fork,
        params: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        if !params.is_empty() {
            let v = TestExecute::from_bytes(params.into()).unwrap();
            if v.value == 42 {
                panic!("42!");
            } else {
                return Err(ExecutionError::new(
                    ExecutionErrorKind::service(0),
                    "value is not a great answer",
                ));
            }
        }
        Ok(())
    }

    fn state_hash(&self, _instance: InstanceDescriptor, _snapshot: &dyn Snapshot) -> Vec<Hash> {
        vec![]
    }
}

impl TestDispatcherInterface for TestDispatcherService {
    fn test_deploy(
        &self,
        context: TransactionContext,
        arg: TestDeploy,
    ) -> Result<(), ExecutionError> {
        let mut index = Entry::new(context.instance.name, context.fork());
        index.set(arg.value);
        drop(index);

        let artifact = if arg.value == 24 {
            ServicePanicImpl.artifact_id().into()
        } else {
            ServiceGoodImpl.artifact_id().into()
        };

        context.dispatch_action(dispatcher::Action::RegisterArtifact {
            artifact,
            spec: Vec::new(),
        });

        if arg.value == 42 {
            return Err(dispatcher::Error::UnknownArtifactId.into());
        }

        Ok(())
    }

    fn test_add(&self, context: TransactionContext, arg: TestAdd) -> Result<(), ExecutionError> {
        let mut index = Entry::new(context.instance.name, context.fork());
        index.set(arg.value);
        drop(index);

        let config = match arg.value {
            42 => TestExecute { value: 42 }.into_bytes(),
            18 => TestExecute { value: 18 }.into_bytes(),
            _ => Vec::new(),
        };

        let artifact = if arg.value == 24 {
            ServicePanicImpl.artifact_id().into()
        } else {
            TestDispatcherService.artifact_id().into()
        };

        context.dispatch_action(dispatcher::Action::AddService {
            artifact,
            instance_name: format!("good-service-{}", arg.value),
            config,
        });

        Ok(())
    }

    fn test_execute(
        &self,
        context: TransactionContext,
        arg: TestExecute,
    ) -> Result<(), ExecutionError> {
        if arg.value == 42 {
            panic!(StorageError::new("42"))
        }
        let mut index = ListIndex::new(context.instance.name, context.fork());
        index.push(arg.value);
        index.push(42 / arg.value);
        Ok(())
    }
}

#[exonum_service(crate = "crate")]
trait ServiceGood {}

#[derive(Debug, ServiceFactory)]
#[exonum(
    crate = "crate",
    artifact_name = "good_service",
    artifact_version = "1.0.0",
    proto_sources = "crate::proto::schema",
    implements("ServiceGood")
)]
pub struct ServiceGoodImpl;

impl ServiceGood for ServiceGoodImpl {}

impl Service for ServiceGoodImpl {
    fn state_hash(&self, _instance: InstanceDescriptor, _snapshot: &dyn Snapshot) -> Vec<Hash> {
        vec![]
    }

    fn before_commit(&self, context: BeforeCommitContext) {
        let mut index = ListIndex::new(IDX_NAME, context.fork);
        index.push(1);
    }
}

#[exonum_service(crate = "crate")]
trait ServicePanic {}

#[derive(Debug, ServiceFactory)]
#[exonum(
    crate = "crate",
    artifact_name = "panic_service",
    artifact_version = "1.0.0",
    proto_sources = "crate::proto::schema",
    implements("ServicePanic")
)]
struct ServicePanicImpl;

impl ServicePanic for ServicePanicImpl {}

impl Service for ServicePanicImpl {
    fn before_commit(&self, _context: BeforeCommitContext) {
        panic!("42");
    }

    fn state_hash(&self, _instance: InstanceDescriptor, _snapshot: &dyn Snapshot) -> Vec<Hash> {
        vec![]
    }
}

#[exonum_service(crate = "crate")]
trait ServicePanicStorageError {}

#[derive(Debug, ServiceFactory)]
#[exonum(
    crate = "crate",
    artifact_name = "storage_error_service",
    artifact_version = "1.0.0",
    proto_sources = "crate::proto::schema",
    implements("ServicePanicStorageError")
)]
struct ServicePanicStorageErrorImpl;

impl ServicePanicStorageError for ServicePanicStorageErrorImpl {}

impl Service for ServicePanicStorageErrorImpl {
    fn before_commit(&self, _context: BeforeCommitContext) {
        panic!(StorageError::new("42"));
    }

    fn state_hash(&self, _instance: InstanceDescriptor, _snapshot: &dyn Snapshot) -> Vec<Hash> {
        vec![]
    }
}

const TX_CHECK_RESULT_SERVICE_ID: InstanceId = 255;

lazy_static! {
    static ref EXECUTION_STATUS: Mutex<Result<(), ExecutionError>> = Mutex::new(Ok(()));
}

#[protobuf_convert(source = "TestServiceTx")]
#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert, BinaryValue, ObjectHash)]
struct TxResult {
    value: u64,
}

#[exonum_service(crate = "crate")]
trait TxResultCheckInterface {
    fn tx_result(&self, context: TransactionContext, arg: TxResult) -> Result<(), ExecutionError>;
}

#[derive(Debug, ServiceFactory)]
#[exonum(
    crate = "crate",
    artifact_name = "tx_result_check",
    artifact_version = "1.0.0",
    proto_sources = "crate::proto::schema",
    implements("TxResultCheckInterface")
)]
struct TxResultCheckService;

impl TxResultCheckInterface for TxResultCheckService {
    fn tx_result(&self, context: TransactionContext, arg: TxResult) -> Result<(), ExecutionError> {
        let mut entry = create_entry(context.fork());
        entry.set(arg.value);
        EXECUTION_STATUS.lock().unwrap().clone()
    }
}

impl Service for TxResultCheckService {
    fn state_hash(&self, _instance: InstanceDescriptor, _snapshot: &dyn Snapshot) -> Vec<Hash> {
        vec![]
    }
}

fn create_entry<T: IndexAccess>(fork: T) -> Entry<T, u64> {
    Entry::new("transaction_status_test", fork)
}

fn assert_service_execute(blockchain: &Blockchain, db: &mut dyn Database) {
    let (_, patch) =
        blockchain.create_patch(ValidatorId::zero(), Height(1), &[], &mut BTreeMap::new());
    db.merge(patch).unwrap();
    let snapshot = db.snapshot();
    let index = ListIndex::new(IDX_NAME, &snapshot);
    assert_eq!(index.len(), 1);
    assert_eq!(index.get(0), Some(1));
}

fn assert_service_execute_panic(blockchain: &Blockchain, db: &mut dyn Database) {
    let (_, patch) =
        blockchain.create_patch(ValidatorId::zero(), Height(1), &[], &mut BTreeMap::new());
    db.merge(patch).unwrap();
    let snapshot = db.snapshot();
    let index: ListIndex<_, u32> = ListIndex::new(IDX_NAME, &snapshot);
    assert!(index.is_empty());
}

fn execute_transaction(blockchain: &mut Blockchain, tx: Verified<AnyTx>) -> ExecutionStatus {
    let tx_hash = tx.object_hash();
    blockchain
        .merge({
            let fork = blockchain.fork();
            {
                let mut schema = Schema::new(&fork);
                schema.add_transaction_into_pool(tx.clone());
            }
            fork.into_patch()
        })
        .unwrap();

    blockchain
        .merge(
            blockchain
                .create_patch(
                    ValidatorId::zero(),
                    Height::zero(),
                    &[tx.object_hash()],
                    &mut BTreeMap::new(),
                )
                .1,
        )
        .unwrap();

    let snapshot = blockchain.snapshot();
    Schema::new(snapshot.as_ref())
        .transaction_results()
        .get(&tx_hash)
        .unwrap()
}

fn create_blockchain(instances: impl IntoIterator<Item = InstanceCollection>) -> Blockchain {
    let config = generate_testnet_config(1, 0)[0].clone();
    let service_keypair = config.service_keypair();
    let api_channel = mpsc::channel(1);
    let internal_sender = mpsc::channel(1).0;

    BlockchainBuilder::new(TemporaryDB::new(), config.consensus, service_keypair)
        .with_rust_runtime(instances)
        .finalize(ApiSender::new(api_channel.0), internal_sender)
        .unwrap()
}

#[test]
fn handling_tx_panic_error() {
    let mut blockchain =
        create_blockchain(vec![InstanceCollection::new(TestDispatcherService)
            .with_instance(TEST_SERVICE_ID, IDX_NAME, ())]);

    let (pk, sec_key) = crypto::gen_keypair();
    let tx_ok1 = TestExecute { value: 3 }.sign(TEST_SERVICE_ID, pk, &sec_key);
    let tx_ok2 = TestExecute { value: 4 }.sign(TEST_SERVICE_ID, pk, &sec_key);
    let tx_failed = TestExecute { value: 0 }.sign(TEST_SERVICE_ID, pk, &sec_key);
    let tx_storage_error = TestExecute { value: 42 }.sign(TEST_SERVICE_ID, pk, &sec_key);

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
        &mut BTreeMap::new(),
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
    let mut blockchain =
        create_blockchain(vec![InstanceCollection::new(TestDispatcherService)
            .with_instance(TEST_SERVICE_ID, IDX_NAME, ())]);

    let (pk, sec_key) = crypto::gen_keypair();
    let tx_ok1 = TestExecute { value: 3 }.sign(TEST_SERVICE_ID, pk, &sec_key);
    let tx_ok2 = TestExecute { value: 4 }.sign(TEST_SERVICE_ID, pk, &sec_key);
    let tx_failed = TestExecute { value: 0 }.sign(TEST_SERVICE_ID, pk, &sec_key);
    let tx_storage_error = TestExecute { value: 42 }.sign(TEST_SERVICE_ID, pk, &sec_key);

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
        &mut BTreeMap::new(),
    );
}

#[test]
fn service_execute_good() {
    let blockchain = create_blockchain(vec![
        InstanceCollection::new(ServiceGoodImpl).with_instance(3, "service_good", ())
    ]);
    let mut db = TemporaryDB::new();
    assert_service_execute(&blockchain, &mut db);
}

#[test]
fn service_execute_panic() {
    let blockchain = create_blockchain(vec![
        InstanceCollection::new(ServicePanicImpl).with_instance(4, "service_panic", ())
    ]);
    let mut db = TemporaryDB::new();
    assert_service_execute_panic(&blockchain, &mut db);
}

#[test]
#[should_panic]
fn service_execute_panic_storage_error() {
    let blockchain =
        create_blockchain(vec![InstanceCollection::new(ServicePanicStorageErrorImpl)
            .with_instance(5, "service_good", ())]);
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
    let mut blockchain = create_blockchain(vec![InstanceCollection::new(TxResultCheckService)
        .with_instance(TX_CHECK_RESULT_SERVICE_ID, "check_result", ())]);
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

        let (_, patch) = blockchain.create_patch(
            ValidatorId::zero(),
            Height(index),
            &[hash],
            &mut BTreeMap::new(),
        );

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

#[test]
#[should_panic(expected = "Instance identifier for builtin service should be lesser than")]
fn test_dispatcher_incorrect_builtin_service_id() {
    create_blockchain(vec![
        InstanceCollection::new(TestDispatcherService).with_instance(1024, IDX_NAME, ())
    ]);
}

#[test]
fn test_dispatcher_deploy_good() {
    let keypair = crypto::gen_keypair();
    let mut blockchain = create_blockchain(vec![
        InstanceCollection::new(TestDispatcherService).with_instance(TEST_SERVICE_ID, IDX_NAME, ()),
        InstanceCollection::new(ServiceGoodImpl),
    ]);

    let artifact_id = ServiceGoodImpl.artifact_id().into();

    // Tests deployment procedure for the available artifact.
    assert!(!blockchain.dispatcher().is_artifact_deployed(&artifact_id));
    blockchain
        .dispatcher()
        .deploy_artifact(artifact_id.clone(), ())
        .wait()
        .unwrap();
    assert!(blockchain.dispatcher().is_artifact_deployed(&artifact_id));

    // Tests the register artifact action for the deployed artifact.
    let snapshot = blockchain.snapshot();
    assert!(!dispatcher::Schema::new(snapshot.as_ref())
        .artifacts()
        .contains(&artifact_id.name));
    execute_transaction(
        &mut blockchain,
        TestDeploy { value: 1 }.sign(TEST_SERVICE_ID, keypair.0, &keypair.1),
    );
    let snapshot = blockchain.snapshot();
    assert!(dispatcher::Schema::new(snapshot.as_ref())
        .artifacts()
        .contains(&artifact_id.name));
    assert_eq!(Entry::new(IDX_NAME, snapshot.as_ref()).get(), Some(1_u64));
}

#[test]
fn test_dispatcher_already_deployed() {
    let keypair = crypto::gen_keypair();
    let mut blockchain = create_blockchain(vec![
        InstanceCollection::new(TestDispatcherService).with_instance(TEST_SERVICE_ID, IDX_NAME, ()),
        InstanceCollection::new(ServiceGoodImpl).with_instance(11, "good", ()),
    ]);

    let artifact_id = ServiceGoodImpl.artifact_id().into();

    // Tests that we get an error if we try to deploy already deployed artifact.
    assert!(blockchain.dispatcher().is_artifact_deployed(&artifact_id));
    let err = blockchain
        .dispatcher()
        .deploy_artifact(artifact_id.clone(), ())
        .wait()
        .unwrap_err();
    assert_eq!(err, dispatcher::Error::ArtifactAlreadyDeployed.into());
    // Tests that we cannot register artifact twice.
    let result = execute_transaction(
        &mut blockchain,
        TestDeploy { value: 1 }.sign(TEST_SERVICE_ID, keypair.0, &keypair.1),
    );
    assert_eq!(
        result.0,
        Err(dispatcher::Error::ArtifactAlreadyDeployed.into())
    );
}

#[test]
fn test_dispatcher_register_unavailable() {
    let keypair = crypto::gen_keypair();
    let mut blockchain = create_blockchain(vec![
        InstanceCollection::new(TestDispatcherService).with_instance(TEST_SERVICE_ID, IDX_NAME, ()),
        InstanceCollection::new(ServiceGoodImpl),
    ]);

    let artifact_id: ArtifactId = ServiceGoodImpl.artifact_id().into();
    blockchain
        .dispatcher()
        .deploy_artifact(artifact_id.clone(), ())
        .wait()
        .unwrap();
    // Tests ExecutionError during the register artifact execution.
    execute_transaction(
        &mut blockchain,
        TestDeploy { value: 42 }.sign(TEST_SERVICE_ID, keypair.0, &keypair.1),
    );
    let snapshot = blockchain.snapshot();
    assert!(!dispatcher::Schema::new(snapshot.as_ref())
        .artifacts()
        .contains(&artifact_id.name));
    assert_eq!(
        Entry::<_, u64>::new(IDX_NAME, snapshot.as_ref()).get(),
        None
    );
    // Tests that an unavailable artifact will not be registered.
    let error_string = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        execute_transaction(
            &mut blockchain,
            TestDeploy { value: 24 }.sign(TEST_SERVICE_ID, keypair.0, &keypair.1),
        )
        .0
    }))
    .unwrap_err()
    .downcast_ref::<FatalError>()
    .unwrap()
    .to_string();

    assert!(error_string.contains("Unable to deploy registered artifact"));
}

#[test]
fn test_dispatcher_start_service_good() {
    let keypair = crypto::gen_keypair();
    let mut blockchain =
        create_blockchain(vec![InstanceCollection::new(TestDispatcherService)
            .with_instance(TEST_SERVICE_ID, IDX_NAME, ())]);
    // Tests start service for the good service.
    let snapshot = blockchain.snapshot();
    assert!(!dispatcher::Schema::new(snapshot.as_ref())
        .service_instances()
        .contains(&"good-service-1".to_owned()));
    execute_transaction(
        &mut blockchain,
        TestAdd { value: 1 }.sign(TEST_SERVICE_ID, keypair.0, &keypair.1),
    );
    let snapshot = blockchain.snapshot();
    assert!(dispatcher::Schema::new(snapshot.as_ref())
        .service_instances()
        .contains(&"good-service-1".to_owned()));
    assert_eq!(Entry::new(IDX_NAME, snapshot.as_ref()).get(), Some(1_u64));
}

#[test]
fn test_dispatcher_start_service_rollback() {
    let keypair = crypto::gen_keypair();
    let mut blockchain =
        create_blockchain(vec![InstanceCollection::new(TestDispatcherService)
            .with_instance(TEST_SERVICE_ID, IDX_NAME, ())]);

    // Tests that a service with an unregistered artifact will not be started.
    let snapshot = blockchain.snapshot();
    assert!(!dispatcher::Schema::new(snapshot.as_ref())
        .service_instances()
        .contains(&"good-service-24".to_owned()));
    execute_transaction(
        &mut blockchain,
        TestAdd { value: 24 }.sign(TEST_SERVICE_ID, keypair.0, &keypair.1),
    );
    let snapshot = blockchain.snapshot();
    assert!(!dispatcher::Schema::new(snapshot.as_ref())
        .service_instances()
        .contains(&"good-service-24".to_owned()));
    assert_eq!(
        Entry::<_, u64>::new(IDX_NAME, snapshot.as_ref()).get(),
        None
    );

    // Tests that a service with panic during the configure will not be started.
    assert!(!dispatcher::Schema::new(snapshot.as_ref())
        .service_instances()
        .contains(&"good-service-42".to_owned()));
    execute_transaction(
        &mut blockchain,
        TestAdd { value: 42 }.sign(TEST_SERVICE_ID, keypair.0, &keypair.1),
    );
    let snapshot = blockchain.snapshot();
    assert!(!dispatcher::Schema::new(snapshot.as_ref())
        .service_instances()
        .contains(&"good-service-42".to_owned()));
    assert_eq!(
        Entry::<_, u64>::new(IDX_NAME, snapshot.as_ref()).get(),
        None
    );

    // Tests that a service with execution error during the configure will not be started.
    assert!(!dispatcher::Schema::new(snapshot.as_ref())
        .service_instances()
        .contains(&"good-service-18".to_owned()));
    execute_transaction(
        &mut blockchain,
        TestAdd { value: 18 }.sign(TEST_SERVICE_ID, keypair.0, &keypair.1),
    );
    let snapshot = blockchain.snapshot();
    assert!(!dispatcher::Schema::new(snapshot.as_ref())
        .service_instances()
        .contains(&"good-service-18".to_owned()));
    assert_eq!(
        Entry::<_, u64>::new(IDX_NAME, snapshot.as_ref()).get(),
        None
    );
}
