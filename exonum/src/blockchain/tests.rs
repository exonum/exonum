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

use exonum_merkledb::{
    Database, Entry, Error as StorageError, Fork, ListIndex, ObjectHash, TemporaryDB,
};
use futures::{sync::mpsc, Future};

use std::sync::Mutex;

use crate::{
    blockchain::{Blockchain, ExecutionErrorKind, InstanceCollection, Schema},
    crypto,
    helpers::{generate_testnet_config, Height, ValidatorId},
    messages::Verified,
    node::ApiSender,
    proto::{schema::tests::*, Any},
    runtime::{
        dispatcher,
        error::ErrorKind,
        rust::{
            RustArtifactId, Service, ServiceDescriptor, ServiceFactory, Transaction,
            TransactionContext,
        },
        AnyTx, ArtifactId, ArtifactInfo, ExecutionError, ServiceInstanceId,
    },
};

const IDX_NAME: &str = "idx_name";
const TEST_SERVICE_ID: ServiceInstanceId = 255;

#[derive(Serialize, Deserialize, ProtobufConvert, Debug, Clone)]
#[exonum(pb = "TestServiceTx", crate = "crate")]
struct TestExecute {
    value: u64,
}

#[derive(Serialize, Deserialize, ProtobufConvert, Debug, Clone)]
#[exonum(pb = "TestServiceTx", crate = "crate")]
struct TestDeploy {
    value: u64,
}

#[derive(Serialize, Deserialize, ProtobufConvert, Debug, Clone)]
#[exonum(pb = "TestServiceTx", crate = "crate")]
struct TestStart {
    value: u64,
}

#[exonum_service(crate = "crate", dispatcher = "TestDispatcherService")]
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
    fn test_start(&self, context: TransactionContext, arg: TestStart)
        -> Result<(), ExecutionError>;
}

#[derive(Debug, ServiceFactory)]
#[exonum(
    crate = "crate",
    artifact_name = "test_dispatcher",
    proto_sources = "crate::proto::schema"
)]
struct TestDispatcherService;

impl Service for TestDispatcherService {
    fn configure(
        &self,
        _descriptor: ServiceDescriptor,
        _fork: &Fork,
        params: Any,
    ) -> Result<(), ExecutionError> {
        if params.clone().try_into::<()>().is_err() {
            let v: TestExecute = params.try_into().expect("Expected `TestExecute`");
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
}

impl TestDispatcherInterface for TestDispatcherService {
    fn test_deploy(
        &self,
        mut context: TransactionContext,
        arg: TestDeploy,
    ) -> Result<(), ExecutionError> {
        let mut index = Entry::new(context.service_name(), context.fork());
        index.set(arg.value);
        drop(index);

        let artifact = if arg.value == 24 {
            ServicePanicImpl.artifact_id().into()
        } else {
            ServiceGoodImpl.artifact_id().into()
        };

        context.dispatch_action(dispatcher::Action::RegisterArtifact {
            artifact,
            spec: ().into(),
        });

        if arg.value == 42 {
            Err(dispatcher::Error::UnknownArtifactId)?;
        }

        Ok(())
    }

    fn test_start(
        &self,
        mut context: TransactionContext,
        arg: TestStart,
    ) -> Result<(), ExecutionError> {
        let mut index = Entry::new(context.service_name(), context.fork());
        index.set(arg.value);
        drop(index);

        let config = match arg.value {
            42 => TestExecute { value: 42 }.into(),
            18 => TestExecute { value: 18 }.into(),
            _ => ().into(),
        };

        let artifact = if arg.value == 24 {
            ServicePanicImpl.artifact_id().into()
        } else {
            TestDispatcherService.artifact_id().into()
        };

        context.dispatch_action(dispatcher::Action::StartService {
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
        let mut index = ListIndex::new(context.service_name(), context.fork());
        index.push(arg.value);
        index.push(42 / arg.value);
        Ok(())
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

fn execute_transaction(blockchain: &mut Blockchain, tx: Verified<AnyTx>) {
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
                .create_patch(ValidatorId::zero(), Height::zero(), &[tx.object_hash()])
                .1,
        )
        .unwrap();
}

fn create_blockchain(instances: impl IntoIterator<Item = InstanceCollection>) -> Blockchain {
    let config = generate_testnet_config(1, 0)[0].clone();
    let service_keypair = config.service_keypair();
    let api_channel = mpsc::unbounded();
    let internal_sender = mpsc::channel(1).0;

    Blockchain::new(
        TemporaryDB::new(),
        instances,
        config.genesis,
        service_keypair,
        ApiSender::new(api_channel.0),
        internal_sender,
    )
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
    assert!(!blockchain.dispatcher().is_deployed(&artifact_id));
    blockchain
        .dispatcher()
        .deploy_artifact(artifact_id.clone(), ())
        .wait()
        .unwrap();
    assert!(blockchain.dispatcher().is_deployed(&artifact_id));

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
fn test_dispatcher_register_rollback() {
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
    let artifact_id: ArtifactId = ServicePanicImpl.artifact_id().into();
    execute_transaction(
        &mut blockchain,
        TestDeploy { value: 24 }.sign(TEST_SERVICE_ID, keypair.0, &keypair.1),
    );
    let snapshot = blockchain.snapshot();
    assert!(!dispatcher::Schema::new(snapshot.as_ref())
        .artifacts()
        .contains(&artifact_id.name));
    assert_eq!(
        Entry::<_, u64>::new(IDX_NAME, snapshot.as_ref()).get(),
        None
    );
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
        TestStart { value: 1 }.sign(TEST_SERVICE_ID, keypair.0, &keypair.1),
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
        TestStart { value: 24 }.sign(TEST_SERVICE_ID, keypair.0, &keypair.1),
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
        TestStart { value: 42 }.sign(TEST_SERVICE_ID, keypair.0, &keypair.1),
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
        TestStart { value: 18 }.sign(TEST_SERVICE_ID, keypair.0, &keypair.1),
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
