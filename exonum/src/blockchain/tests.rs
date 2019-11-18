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

use exonum_crypto as crypto;
use exonum_merkledb::{
    access::AccessExt, BinaryValue, Database, Error as StorageError, ObjectHash, TemporaryDB,
};
use exonum_proto::ProtobufConvert;
use futures::{sync::mpsc, Future};

use std::{collections::BTreeMap, panic, sync::Mutex};

use crate::{
    blockchain::{
        Blockchain, BlockchainMut, ExecutionErrorKind, ExecutionStatus, InstanceCollection, Schema,
    },
    helpers::{generate_testnet_config, Height, ValidatorId},
    messages::Verified,
    node::ApiSender,
    proto::schema::tests::*,
    runtime::{
        error::ErrorKind,
        rust::{CallContext, Service, ServiceFactory, Transaction},
        AnyTx, ArtifactId, DispatcherError, DispatcherSchema, ExecutionError, InstanceId,
        SUPERVISOR_INSTANCE_ID,
    },
};

const TEST_SERVICE_ID: InstanceId = SUPERVISOR_INSTANCE_ID;
const TEST_SERVICE_NAME: &str = "test_service";
const IDX_NAME: &str = "test_service.val";

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

#[exonum_interface(crate = "crate")]
trait TestDispatcherInterface {
    fn test_execute(
        &self,
        context: CallContext<'_>,
        arg: TestExecute,
    ) -> Result<(), ExecutionError>;

    fn test_deploy(&self, context: CallContext<'_>, arg: TestDeploy) -> Result<(), ExecutionError>;

    fn test_add(&self, context: CallContext<'_>, arg: TestAdd) -> Result<(), ExecutionError>;
}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_dispatcher(crate = "crate", implements("TestDispatcherInterface"))]
#[service_factory(
    crate = "crate",
    artifact_name = "test_dispatcher",
    proto_sources = "crate::proto::schema"
)]
struct TestDispatcherService;

impl Service for TestDispatcherService {
    fn initialize(&self, _context: CallContext<'_>, params: Vec<u8>) -> Result<(), ExecutionError> {
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
}

impl TestDispatcherInterface for TestDispatcherService {
    fn test_execute(
        &self,
        context: CallContext<'_>,
        arg: TestExecute,
    ) -> Result<(), ExecutionError> {
        if arg.value == 42 {
            panic!(StorageError::new("42"))
        }
        let mut index = context.service_data().get_list("val");
        index.push(arg.value);
        index.push(42 / arg.value);
        Ok(())
    }

    fn test_deploy(&self, context: CallContext<'_>, arg: TestDeploy) -> Result<(), ExecutionError> {
        context.service_data().get_entry("val").set(arg.value);

        let artifact = if arg.value == 24 {
            ServicePanicImpl.artifact_id().into()
        } else {
            ServiceGoodImpl.artifact_id().into()
        };
        context.start_artifact_registration(artifact, vec![])?;
        if arg.value == 42 {
            return Err(DispatcherError::UnknownArtifactId.into());
        }

        Ok(())
    }

    fn test_add(&self, mut context: CallContext<'_>, arg: TestAdd) -> Result<(), ExecutionError> {
        let mut index = context.service_data().get_entry("val");
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
        context.start_adding_service(artifact, format!("good-service-{}", arg.value), config)
    }
}

#[exonum_interface(crate = "crate")]
trait ServiceGood {}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_dispatcher(crate = "crate", implements("ServiceGood"))]
#[service_factory(
    crate = "crate",
    artifact_name = "good_service",
    artifact_version = "1.0.0",
    proto_sources = "crate::proto::schema"
)]
pub struct ServiceGoodImpl;

impl ServiceGood for ServiceGoodImpl {}

impl Service for ServiceGoodImpl {
    fn before_commit(&self, context: CallContext<'_>) {
        let mut index = context.service_data().get_list("val");
        index.push(1);
    }
}

#[exonum_interface(crate = "crate")]
trait ServicePanic {}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_dispatcher(crate = "crate", implements("ServicePanic"))]
#[service_factory(
    crate = "crate",
    artifact_name = "panic_service",
    artifact_version = "1.0.0",
    proto_sources = "crate::proto::schema"
)]
struct ServicePanicImpl;

impl ServicePanic for ServicePanicImpl {}

impl Service for ServicePanicImpl {
    fn before_commit(&self, _context: CallContext<'_>) {
        panic!("42");
    }
}

#[exonum_interface(crate = "crate")]
trait ServicePanicStorageError {}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_dispatcher(crate = "crate", implements("ServicePanicStorageError"))]
#[service_factory(
    crate = "crate",
    artifact_name = "storage_error_service",
    artifact_version = "1.0.0",
    proto_sources = "crate::proto::schema"
)]
struct ServicePanicStorageErrorImpl;

impl ServicePanicStorageError for ServicePanicStorageErrorImpl {}

impl Service for ServicePanicStorageErrorImpl {
    fn before_commit(&self, _context: CallContext<'_>) {
        panic!(StorageError::new("42"));
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

#[exonum_interface(crate = "crate")]
trait TxResultCheckInterface {
    fn tx_result(&self, context: CallContext<'_>, arg: TxResult) -> Result<(), ExecutionError>;
}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_dispatcher(crate = "crate", implements("TxResultCheckInterface"))]
#[service_factory(
    crate = "crate",
    artifact_name = "tx_result_check",
    artifact_version = "1.0.0",
    proto_sources = "crate::proto::schema"
)]
struct TxResultCheckService;

impl TxResultCheckInterface for TxResultCheckService {
    fn tx_result(&self, context: CallContext<'_>, arg: TxResult) -> Result<(), ExecutionError> {
        let mut entry = context.service_data().get_entry("status");
        entry.set(arg.value);
        EXECUTION_STATUS.lock().unwrap().clone()
    }
}

impl Service for TxResultCheckService {}

fn assert_service_execute(blockchain: &mut BlockchainMut) {
    let (_, patch) =
        blockchain.create_patch(ValidatorId::zero(), Height(1), &[], &mut BTreeMap::new());
    blockchain.merge(patch).unwrap();
    let snapshot = blockchain.snapshot();
    let index = snapshot.get_list("service_good.val");
    assert_eq!(index.len(), 1);
    assert_eq!(index.get(0), Some(1));
}

fn assert_service_execute_panic(blockchain: &mut BlockchainMut) {
    let (_, patch) =
        blockchain.create_patch(ValidatorId::zero(), Height(1), &[], &mut BTreeMap::new());
    blockchain.merge(patch).unwrap();
    let snapshot = blockchain.snapshot();
    assert!(snapshot
        .as_ref()
        .get_list::<_, u32>("service_panic.val")
        .is_empty());
}

fn execute_transaction(blockchain: &mut BlockchainMut, tx: Verified<AnyTx>) -> ExecutionStatus {
    let tx_hash = tx.object_hash();
    blockchain
        .merge({
            let fork = blockchain.fork();
            let mut schema = Schema::new(&fork);
            schema.add_transaction_into_pool(tx.clone());
            fork.into_patch()
        })
        .unwrap();

    let (block_hash, patch) = blockchain.create_patch(
        ValidatorId::zero(),
        Height::zero(),
        &[tx.object_hash()],
        &mut BTreeMap::new(),
    );

    blockchain
        .commit(patch, block_hash, vec![], &mut BTreeMap::new())
        .unwrap();
    let snapshot = blockchain.snapshot();
    Schema::new(&snapshot)
        .transaction_results()
        .get(&tx_hash)
        .unwrap()
}

fn create_blockchain(instances: impl IntoIterator<Item = InstanceCollection>) -> BlockchainMut {
    let config = generate_testnet_config(1, 0)[0].clone();
    let service_keypair = config.service_keypair();
    let api_notifier = mpsc::channel(0).0;

    Blockchain::new(TemporaryDB::new(), service_keypair, ApiSender::closed())
        .into_mut(config.consensus)
        .with_rust_runtime(api_notifier, instances)
        .build()
        .unwrap()
}

#[test]
fn handling_tx_panic_error() {
    let mut blockchain = create_blockchain(vec![InstanceCollection::new(TestDispatcherService)
        .with_instance(TEST_SERVICE_ID, TEST_SERVICE_NAME, ())]);

    let (pk, sec_key) = crypto::gen_keypair();
    let tx_ok1 = TestExecute { value: 3 }.sign(TEST_SERVICE_ID, pk, &sec_key);
    let tx_ok2 = TestExecute { value: 4 }.sign(TEST_SERVICE_ID, pk, &sec_key);
    let tx_failed = TestExecute { value: 0 }.sign(TEST_SERVICE_ID, pk, &sec_key);
    let tx_storage_error = TestExecute { value: 42 }.sign(TEST_SERVICE_ID, pk, &sec_key);

    let fork = blockchain.fork();
    let mut schema = Schema::new(&fork);
    schema.add_transaction_into_pool(tx_ok1.clone());
    schema.add_transaction_into_pool(tx_ok2.clone());
    schema.add_transaction_into_pool(tx_failed.clone());
    schema.add_transaction_into_pool(tx_storage_error.clone());
    blockchain.merge(fork.into_patch()).unwrap();

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

    let index = snapshot.get_list(IDX_NAME);
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

    let fork = blockchain.fork();
    let mut schema = Schema::new(&fork);
    schema.add_transaction_into_pool(tx_ok1.clone());
    schema.add_transaction_into_pool(tx_ok2.clone());
    schema.add_transaction_into_pool(tx_failed.clone());
    schema.add_transaction_into_pool(tx_storage_error.clone());
    blockchain.merge(fork.into_patch()).unwrap();
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
    let mut blockchain = create_blockchain(vec![
        InstanceCollection::new(ServiceGoodImpl).with_instance(3, "service_good", ())
    ]);
    assert_service_execute(&mut blockchain);
}

#[test]
fn service_execute_panic() {
    let mut blockchain = create_blockchain(vec![
        InstanceCollection::new(ServicePanicImpl).with_instance(4, "service_panic", ())
    ]);
    assert_service_execute_panic(&mut blockchain);
}

#[test]
#[should_panic]
fn service_execute_panic_storage_error() {
    let mut blockchain =
        create_blockchain(vec![InstanceCollection::new(ServicePanicStorageErrorImpl)
            .with_instance(5, "service_panic", ())]);
    assert_service_execute_panic(&mut blockchain);
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
        let fork = blockchain.fork();
        let mut schema = Schema::new(&fork);
        schema.add_transaction_into_pool(transaction.clone());
        blockchain.merge(fork.into_patch()).unwrap();

        let (_, patch) = blockchain.create_patch(
            ValidatorId::zero(),
            Height(index),
            &[hash],
            &mut BTreeMap::new(),
        );
        db.merge(patch).unwrap();

        let snapshot = db.snapshot();
        let entry = snapshot
            .as_ref()
            .get_entry::<_, u64>("check_result.status")
            .get();
        if status.is_err() {
            assert!(entry.is_none());
        } else {
            assert_eq!(Some(index), entry);
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
        InstanceCollection::new(TestDispatcherService).with_instance(
            TEST_SERVICE_ID,
            TEST_SERVICE_NAME,
            (),
        ),
        InstanceCollection::new(ServiceGoodImpl),
    ]);

    let artifact_id = ServiceGoodImpl.artifact_id().into();

    // Tests deployment procedure for the available artifact.
    assert!(!blockchain.dispatcher.is_artifact_deployed(&artifact_id));
    blockchain
        .dispatcher
        .deploy_artifact(artifact_id.clone(), vec![])
        .wait()
        .unwrap();
    assert!(blockchain.dispatcher.is_artifact_deployed(&artifact_id));

    // Tests the register artifact action for the deployed artifact.
    let snapshot = blockchain.snapshot();
    assert!(!DispatcherSchema::new(&snapshot)
        .artifacts()
        .contains(&artifact_id.name));
    execute_transaction(
        &mut blockchain,
        TestDeploy { value: 1 }.sign(TEST_SERVICE_ID, keypair.0, &keypair.1),
    );
    let snapshot = blockchain.snapshot();
    assert!(DispatcherSchema::new(&snapshot)
        .artifacts()
        .contains(&artifact_id.name));
    assert_eq!(snapshot.get_entry(IDX_NAME).get(), Some(1_u64));
}

#[test]
fn test_dispatcher_already_deployed() {
    let keypair = crypto::gen_keypair();
    let mut blockchain = create_blockchain(vec![
        InstanceCollection::new(TestDispatcherService).with_instance(
            TEST_SERVICE_ID,
            TEST_SERVICE_NAME,
            (),
        ),
        InstanceCollection::new(ServiceGoodImpl).with_instance(11, "good", ()),
    ]);
    let artifact_id = ServiceGoodImpl.artifact_id().into();

    // Tests that we get an error if we try to deploy already deployed artifact.
    assert!(blockchain.dispatcher.is_artifact_deployed(&artifact_id));
    let err = blockchain
        .dispatcher
        .deploy_artifact(artifact_id.clone(), vec![])
        .wait()
        .unwrap_err();
    assert_eq!(err, DispatcherError::ArtifactAlreadyDeployed.into());
    // Tests that we cannot register artifact twice.
    let result = execute_transaction(
        &mut blockchain,
        TestDeploy { value: 1 }.sign(TEST_SERVICE_ID, keypair.0, &keypair.1),
    );
    assert_eq!(
        result.0,
        Err(DispatcherError::ArtifactAlreadyDeployed.into())
    );
}

#[test]
#[should_panic(expected = "Unable to deploy registered artifact")]
fn test_dispatcher_register_unavailable() {
    let keypair = crypto::gen_keypair();
    let mut blockchain = create_blockchain(vec![
        InstanceCollection::new(TestDispatcherService).with_instance(
            TEST_SERVICE_ID,
            TEST_SERVICE_NAME,
            (),
        ),
        InstanceCollection::new(ServiceGoodImpl),
    ]);

    let artifact_id: ArtifactId = ServiceGoodImpl.artifact_id().into();
    blockchain
        .dispatcher
        .deploy_artifact(artifact_id.clone(), vec![])
        .wait()
        .unwrap();
    // Tests ExecutionError during the register artifact execution.
    execute_transaction(
        &mut blockchain,
        TestDeploy { value: 42 }.sign(TEST_SERVICE_ID, keypair.0, &keypair.1),
    );
    let snapshot = blockchain.snapshot();
    assert!(!DispatcherSchema::new(&snapshot)
        .artifacts()
        .contains(&artifact_id.name));
    assert!(!snapshot.get_entry::<_, u64>(IDX_NAME).exists());
    // Tests that an unavailable artifact will not be registered.
    execute_transaction(
        &mut blockchain,
        TestDeploy { value: 24 }.sign(TEST_SERVICE_ID, keypair.0, &keypair.1),
    );
}

#[test]
fn test_dispatcher_start_service_good() {
    let keypair = crypto::gen_keypair();
    let mut blockchain = create_blockchain(vec![InstanceCollection::new(TestDispatcherService)
        .with_instance(TEST_SERVICE_ID, TEST_SERVICE_NAME, ())]);
    // Tests start service for the good service.
    let snapshot = blockchain.snapshot();
    assert!(!DispatcherSchema::new(&snapshot)
        .service_instances()
        .contains(&"good-service-1".to_owned()));
    execute_transaction(
        &mut blockchain,
        TestAdd { value: 1 }.sign(TEST_SERVICE_ID, keypair.0, &keypair.1),
    );
    let snapshot = blockchain.snapshot();
    assert!(DispatcherSchema::new(&snapshot)
        .service_instances()
        .contains(&"good-service-1".to_owned()));
    assert_eq!(snapshot.get_entry(IDX_NAME).get(), Some(1_u64));
}

#[test]
fn test_dispatcher_start_service_rollback() {
    let keypair = crypto::gen_keypair();
    let mut blockchain = create_blockchain(vec![InstanceCollection::new(TestDispatcherService)
        .with_instance(TEST_SERVICE_ID, TEST_SERVICE_NAME, ())]);

    // Tests that a service with an unregistered artifact will not be started.
    let snapshot = blockchain.snapshot();
    assert!(!DispatcherSchema::new(&snapshot)
        .service_instances()
        .contains(&"good-service-24".to_owned()));
    execute_transaction(
        &mut blockchain,
        TestAdd { value: 24 }.sign(TEST_SERVICE_ID, keypair.0, &keypair.1),
    );
    let snapshot = blockchain.snapshot();
    assert!(!DispatcherSchema::new(&snapshot)
        .service_instances()
        .contains(&"good-service-24".to_owned()));
    assert!(!snapshot.get_entry::<_, u64>(IDX_NAME).exists());

    // Tests that a service with panic during the configure will not be started.
    assert!(!DispatcherSchema::new(&snapshot)
        .service_instances()
        .contains(&"good-service-42".to_owned()));
    execute_transaction(
        &mut blockchain,
        TestAdd { value: 42 }.sign(TEST_SERVICE_ID, keypair.0, &keypair.1),
    );
    let snapshot = blockchain.snapshot();
    assert!(!DispatcherSchema::new(&snapshot)
        .service_instances()
        .contains(&"good-service-42".to_owned()));
    assert!(!snapshot.get_entry::<_, u64>(IDX_NAME).exists());

    // Tests that a service with execution error during the configure will not be started.
    assert!(!DispatcherSchema::new(&snapshot)
        .service_instances()
        .contains(&"good-service-18".to_owned()));
    execute_transaction(
        &mut blockchain,
        TestAdd { value: 18 }.sign(TEST_SERVICE_ID, keypair.0, &keypair.1),
    );
    let snapshot = blockchain.snapshot();
    assert!(!DispatcherSchema::new(&snapshot)
        .service_instances()
        .contains(&"good-service-18".to_owned()));
    assert!(!snapshot.get_entry::<_, u64>(IDX_NAME).exists());
}
