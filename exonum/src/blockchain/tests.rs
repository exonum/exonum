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

use exonum_crypto::gen_keypair;
use exonum_merkledb::{
    access::AccessExt, BinaryValue, Database, Error as StorageError, ObjectHash, TemporaryDB,
};
use futures::{sync::mpsc, Future};

use std::{collections::BTreeMap, panic, sync::Mutex};

use crate::{
    blockchain::{
        config::{GenesisConfigBuilder, InstanceInitParams},
        Blockchain, BlockchainMut, Schema,
    },
    helpers::{generate_testnet_config, Height, ValidatorId},
    messages::Verified,
    node::ApiSender,
    runtime::{
        rust::{CallContext, RustRuntime, Service, ServiceFactory},
        AnyTx, ArtifactId, DispatcherError, DispatcherSchema, ErrorKind, ErrorMatch,
        ExecutionError, InstanceId, InstanceSpec, InstanceStatus, SUPERVISOR_INSTANCE_ID,
    },
};

const TEST_SERVICE_ID: InstanceId = SUPERVISOR_INSTANCE_ID;
const TEST_SERVICE_NAME: &str = "test_service";
const IDX_NAME: &str = "test_service.val";

#[exonum_interface(crate = "crate")]
trait TestDispatcher<Ctx> {
    type Output;

    fn test_execute(&self, ctx: Ctx, arg: u64) -> Self::Output;
    fn test_deploy(&self, ctx: Ctx, arg: u64) -> Self::Output;
    fn test_add(&self, ctx: Ctx, arg: u64) -> Self::Output;
    fn test_stop(&self, ctx: Ctx, instance_id: InstanceId) -> Self::Output;
}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_dispatcher(crate = "crate", implements("TestDispatcher"))]
#[service_factory(
    crate = "crate",
    artifact_name = "test_dispatcher",
    proto_sources = "crate::proto::schema"
)]
struct TestDispatcherService;

impl Service for TestDispatcherService {
    fn initialize(&self, _context: CallContext<'_>, params: Vec<u8>) -> Result<(), ExecutionError> {
        if !params.is_empty() {
            let v = u64::from_bytes(params.into()).unwrap();
            if v == 42 {
                panic!("42!");
            } else {
                return Err(ExecutionError::service(0, "Not a great answer"));
            }
        }
        Ok(())
    }
}

impl TestDispatcher<CallContext<'_>> for TestDispatcherService {
    type Output = Result<(), ExecutionError>;

    fn test_execute(&self, ctx: CallContext<'_>, arg: u64) -> Self::Output {
        if arg == 42 {
            panic!(StorageError::new("42"))
        }
        let mut index = ctx.service_data().get_list("val");
        index.push(arg);
        index.push(42 / arg);
        Ok(())
    }

    fn test_deploy(&self, ctx: CallContext<'_>, arg: u64) -> Self::Output {
        ctx.service_data().get_entry("val").set(arg);

        let artifact = if arg == 24 {
            ServicePanicImpl.artifact_id()
        } else {
            ServiceGoodImpl.artifact_id()
        };
        ctx.start_artifact_registration(artifact, vec![])?;
        if arg == 42 {
            return Err(DispatcherError::UnknownArtifactId.into());
        }

        Ok(())
    }

    fn test_add(&self, mut ctx: CallContext<'_>, arg: u64) -> Self::Output {
        {
            let mut index = ctx.service_data().get_entry("val");
            index.set(arg);
        }

        let instance_id = {
            let mut instance_id_entry = ctx.service_data().get_entry("instance_ids");
            let instance_id = instance_id_entry.get().unwrap_or(TEST_SERVICE_ID + 1);
            instance_id_entry.set(instance_id + 1);
            instance_id
        };

        let config = match arg {
            42 => 42_u64.into_bytes(),
            18 => 18_u64.into_bytes(),
            _ => Vec::new(),
        };

        let artifact = if arg == 24 {
            ServicePanicImpl.artifact_id()
        } else {
            TestDispatcherService.artifact_id()
        };

        let instance_name = format!("good-service-{}", arg);
        let spec = InstanceSpec {
            id: instance_id,
            name: instance_name,
            artifact,
        };

        ctx.initiate_adding_service(spec, config)
    }

    fn test_stop(&self, ctx: CallContext<'_>, instance_id: InstanceId) -> Self::Output {
        ctx.initiate_stopping_service(instance_id)
    }
}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_dispatcher(crate = "crate")]
#[service_factory(
    crate = "crate",
    artifact_name = "good_service",
    artifact_version = "1.0.0",
    proto_sources = "crate::proto::schema"
)]
pub struct ServiceGoodImpl;

impl Service for ServiceGoodImpl {
    fn after_transactions(&self, context: CallContext<'_>) -> Result<(), ExecutionError> {
        let mut index = context.service_data().get_list("val");
        index.push(1);
        Ok(())
    }
}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_dispatcher(crate = "crate")]
#[service_factory(
    crate = "crate",
    artifact_name = "panic_service",
    artifact_version = "1.0.0",
    proto_sources = "crate::proto::schema"
)]
struct ServicePanicImpl;

impl Service for ServicePanicImpl {
    fn after_transactions(&self, context: CallContext<'_>) -> Result<(), ExecutionError> {
        if context.in_genesis_block() {
            return Ok(());
        }
        panic!("42");
    }
}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_dispatcher(crate = "crate")]
#[service_factory(
    crate = "crate",
    artifact_name = "panic_service",
    artifact_version = "1.0.0",
    proto_sources = "crate::proto::schema"
)]
struct ServiceGenesisPanicImpl;

impl Service for ServiceGenesisPanicImpl {
    fn after_transactions(&self, _context: CallContext<'_>) -> Result<(), ExecutionError> {
        // Panics even on the genesis block.
        panic!("42");
    }
}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_dispatcher(crate = "crate")]
#[service_factory(
    crate = "crate",
    artifact_name = "storage_error_service",
    artifact_version = "1.0.0",
    proto_sources = "crate::proto::schema"
)]
struct ServicePanicStorageErrorImpl;

impl Service for ServicePanicStorageErrorImpl {
    fn after_transactions(&self, _context: CallContext<'_>) -> Result<(), ExecutionError> {
        panic!(StorageError::new("42"));
    }
}

const CHECK_RESULT_SERVICE_ID: InstanceId = 255;

lazy_static! {
    static ref EXECUTION_STATUS: Mutex<Result<(), ExecutionError>> = Mutex::new(Ok(()));
}

#[exonum_interface(crate = "crate")]
trait ResultCheck<Ctx> {
    type Output;
    fn tx_result(&self, ctx: Ctx, arg: u64) -> Self::Output;
}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_dispatcher(crate = "crate", implements("ResultCheck"))]
#[service_factory(
    crate = "crate",
    artifact_name = "result_check",
    artifact_version = "1.0.0",
    proto_sources = "crate::proto::schema"
)]
struct ResultCheckService;

impl ResultCheck<CallContext<'_>> for ResultCheckService {
    type Output = Result<(), ExecutionError>;

    fn tx_result(&self, ctx: CallContext<'_>, arg: u64) -> Self::Output {
        let mut entry = ctx.service_data().get_entry("status");
        entry.set(arg);
        EXECUTION_STATUS.lock().unwrap().clone()
    }
}

impl Service for ResultCheckService {}

fn assert_service_execute(blockchain: &mut BlockchainMut) {
    let (_, patch) = blockchain.create_patch(
        ValidatorId::zero().into(),
        Height(1),
        &[],
        &mut BTreeMap::new(),
    );
    blockchain.merge(patch).unwrap();
    let snapshot = blockchain.snapshot();
    let index = snapshot.get_list("service_good.val");
    // One time `after_transactions` was invoked on the genesis block, and then we created one more block.
    assert_eq!(index.len(), 2);
    assert_eq!(index.get(0), Some(1));
    assert_eq!(index.get(1), Some(1));
}

fn assert_service_execute_panic(blockchain: &mut BlockchainMut) {
    let (_, patch) = blockchain.create_patch(
        ValidatorId::zero().into(),
        Height(1),
        &[],
        &mut BTreeMap::new(),
    );
    blockchain.merge(patch).unwrap();
    let snapshot = blockchain.snapshot();
    assert!(snapshot
        .as_ref()
        .get_list::<_, u32>("service_panic.val")
        .is_empty());
}

fn execute_transaction(
    blockchain: &mut BlockchainMut,
    tx: Verified<AnyTx>,
) -> Result<(), ExecutionError> {
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
        ValidatorId::zero().into(),
        Height(1),
        &[tx_hash],
        &mut BTreeMap::new(),
    );

    blockchain
        .commit(patch, block_hash, vec![], &mut BTreeMap::new())
        .unwrap();
    let snapshot = blockchain.snapshot();
    let schema = Schema::new(&snapshot);
    let location = schema.transactions_locations().get(&tx_hash).unwrap();
    schema.transaction_result(location).unwrap()
}

/// Attempts to create a blockchain, returning an error if the genesis block
/// was not created.
fn maybe_create_blockchain(
    services: Vec<Box<dyn ServiceFactory>>,
    instances: Vec<impl Into<InstanceInitParams>>,
) -> Result<BlockchainMut, failure::Error> {
    let config = generate_testnet_config(1, 0)[0].clone();
    let service_keypair = config.service_keypair();

    let rust_runtime = services
        .into_iter()
        .fold(RustRuntime::new(mpsc::channel(0).0), |runtime, factory| {
            runtime.with_factory(factory)
        });

    let genesis_config = instances
        .into_iter()
        .fold(
            GenesisConfigBuilder::with_consensus_config(config.consensus),
            |builder, instance| {
                let instance = instance.into();
                builder
                    .with_artifact(instance.instance_spec.artifact.clone())
                    .with_instance(instance)
            },
        )
        .build();

    Blockchain::new(TemporaryDB::new(), service_keypair, ApiSender::closed())
        .into_mut(genesis_config)
        .with_runtime(rust_runtime)
        .build()
}

/// Creates a blockchain from provided services and instances.
/// Panics if genesis block was not created.
fn create_blockchain(
    services: Vec<Box<dyn ServiceFactory>>,
    instances: Vec<impl Into<InstanceInitParams>>,
) -> BlockchainMut {
    maybe_create_blockchain(services, instances).unwrap()
}

/// Checks that `Schema::height` and `Schema::next_height` work as expected.
#[test]
fn blockchain_height() {
    let mut blockchain = create_blockchain(
        Vec::<Box<dyn ServiceFactory>>::new(),
        Vec::<InstanceInitParams>::new(),
    );

    // Check that height is 0 after genesis creation.
    let snapshot = blockchain.snapshot();
    let schema = Schema::new(&snapshot);
    assert_eq!(schema.height(), Height(0));
    assert_eq!(schema.next_height(), Height(1));

    // Create one block.
    let (_, patch) = blockchain.create_patch(
        ValidatorId::zero().into(),
        Height::zero(),
        &[],
        &mut BTreeMap::new(),
    );
    blockchain.merge(patch).unwrap();

    // Check that height is 1.
    let snapshot = blockchain.snapshot();
    let schema = Schema::new(&snapshot);
    assert_eq!(schema.height(), Height(1));
    assert_eq!(schema.next_height(), Height(2));
}

/// Checks that before genesis creation `Schema::height` panics.
#[test]
#[should_panic(
    expected = "An attempt to get the actual `height` during creating the genesis block"
)]
fn blockchain_height_panics_before_genesis() {
    let service_keypair = gen_keypair();

    // Create a blockchain *without* creating a genesis block.
    let blockchain = Blockchain::new(TemporaryDB::new(), service_keypair, ApiSender::closed());

    let snapshot = blockchain.snapshot();
    let schema = Schema::new(&snapshot);
    let _height = schema.height();
}

/// Checks that before genesis creation `Schema::next_height` doesn't panic.
#[test]
fn blockchain_next_height_does_not_panic_before_genesis() {
    let service_keypair = gen_keypair();

    // Create a blockchain *without* creating a genesis block.
    let blockchain = Blockchain::new(TemporaryDB::new(), service_keypair, ApiSender::closed());

    let snapshot = blockchain.snapshot();
    let schema = Schema::new(&snapshot);
    let height = schema.next_height();
    assert_eq!(height, Height(0))
}

/// Checks that `after_transactions` is invoked for services added
/// within genesis block.
#[test]
fn after_transactions_invoked_on_genesis() {
    // `ServiceGoodImpl` sets the value in schema within `after_transactions`.
    let blockchain = create_blockchain(
        vec![ServiceGoodImpl.into()],
        vec![ServiceGoodImpl
            .artifact_id()
            .into_default_instance(3, "service_good")],
    );

    // After creation of the genesis block, check that value was set.
    let snapshot = blockchain.snapshot();
    let index = snapshot.get_list("service_good.val");
    assert_eq!(index.len(), 1);
    assert_eq!(index.get(0), Some(1));
}

/// Checks that if `after_transactions` fails on the genesis block,
/// the blockchain is not created.
#[test]
fn after_transactions_failure_causes_genesis_failure() {
    let blockchain_result = maybe_create_blockchain(
        vec![ServiceGenesisPanicImpl.into()],
        vec![ServiceGenesisPanicImpl
            .artifact_id()
            .into_default_instance(TEST_SERVICE_ID, TEST_SERVICE_NAME)],
    );

    const EXPECTED_ERR_TEXT: &str = "`after_transactions` failed for at least one service";
    let actual_err = blockchain_result
        .expect_err("Blockchain shouldn't be created after failure within genesis block");

    // Unfortunately, `failure::Error` doesn't implement `PartialEq`, so we have to string-compare them.
    assert!(
        actual_err.to_string().contains(EXPECTED_ERR_TEXT),
        "Expected error should be caused by `after_transactions` hook"
    );
}

#[test]
fn handling_tx_panic_error() {
    let mut blockchain = create_blockchain(
        vec![TestDispatcherService.into()],
        vec![TestDispatcherService
            .artifact_id()
            .into_default_instance(TEST_SERVICE_ID, TEST_SERVICE_NAME)],
    );

    let keypair = gen_keypair();
    let tx_ok1 = keypair.test_execute(TEST_SERVICE_ID, 3);
    let tx_ok2 = keypair.test_execute(TEST_SERVICE_ID, 4);
    let tx_failed = keypair.test_execute(TEST_SERVICE_ID, 0);
    let tx_storage_error = keypair.test_execute(TEST_SERVICE_ID, 42);

    let fork = blockchain.fork();
    let mut schema = Schema::new(&fork);
    schema.add_transaction_into_pool(tx_ok1.clone());
    schema.add_transaction_into_pool(tx_ok2.clone());
    schema.add_transaction_into_pool(tx_failed.clone());
    schema.add_transaction_into_pool(tx_storage_error.clone());
    blockchain.merge(fork.into_patch()).unwrap();

    let (_, patch) = blockchain.create_patch(
        ValidatorId::zero().into(),
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
    let mut blockchain = create_blockchain(
        vec![TestDispatcherService.into()],
        vec![TestDispatcherService
            .artifact_id()
            .into_default_instance(TEST_SERVICE_ID, IDX_NAME)],
    );

    let keypair = gen_keypair();
    let tx_ok1 = keypair.test_execute(TEST_SERVICE_ID, 3);
    let tx_ok2 = keypair.test_execute(TEST_SERVICE_ID, 4);
    let tx_failed = keypair.test_execute(TEST_SERVICE_ID, 0);
    let tx_storage_error = keypair.test_execute(TEST_SERVICE_ID, 42);

    let fork = blockchain.fork();
    let mut schema = Schema::new(&fork);
    schema.add_transaction_into_pool(tx_ok1.clone());
    schema.add_transaction_into_pool(tx_ok2.clone());
    schema.add_transaction_into_pool(tx_failed.clone());
    schema.add_transaction_into_pool(tx_storage_error.clone());
    blockchain.merge(fork.into_patch()).unwrap();
    blockchain.create_patch(
        ValidatorId::zero().into(),
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
    let mut blockchain = create_blockchain(
        vec![ServiceGoodImpl.into()],
        vec![ServiceGoodImpl
            .artifact_id()
            .into_default_instance(3, "service_good")],
    );
    assert_service_execute(&mut blockchain);
}

#[test]
fn service_execute_panic() {
    let mut blockchain = create_blockchain(
        vec![ServicePanicImpl.into()],
        vec![ServicePanicImpl
            .artifact_id()
            .into_default_instance(4, "service_panic")],
    );
    assert_service_execute_panic(&mut blockchain);
}

#[test]
#[should_panic]
fn service_execute_panic_storage_error() {
    let mut blockchain = create_blockchain(
        vec![ServicePanicStorageErrorImpl.into()],
        vec![ServicePanicStorageErrorImpl
            .artifact_id()
            .into_default_instance(5, "service_panic")],
    );
    assert_service_execute_panic(&mut blockchain);
}

#[test]
fn error_discards_transaction_changes() {
    let statuses = [
        Err(ExecutionError::new(ErrorKind::Service { code: 0 }, "")),
        Err(ExecutionError::new(
            ErrorKind::Dispatcher { code: 5 },
            "Foo",
        )),
        Err(ExecutionError::new(
            ErrorKind::Runtime { code: 0 },
            "Strange bar",
        )),
        Err(ExecutionError::new(ErrorKind::Unexpected, "PANIC")),
        Ok(()),
    ];

    let keypair = gen_keypair();
    let mut blockchain = create_blockchain(
        vec![ResultCheckService.into()],
        vec![ResultCheckService
            .artifact_id()
            .into_default_instance(CHECK_RESULT_SERVICE_ID, "check_result")],
    );
    let db = TemporaryDB::new();

    for (index, status) in statuses.iter().enumerate() {
        let index = index as u64;
        *EXECUTION_STATUS.lock().unwrap() = status.clone();

        let transaction = keypair.tx_result(CHECK_RESULT_SERVICE_ID, index);
        let hash = transaction.object_hash();
        let fork = blockchain.fork();
        let mut schema = Schema::new(&fork);
        schema.add_transaction_into_pool(transaction.clone());
        blockchain.merge(fork.into_patch()).unwrap();

        let (_, patch) = blockchain.create_patch(
            ValidatorId::zero().into(),
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
fn test_dispatcher_deploy_good() {
    let mut blockchain = create_blockchain(
        vec![TestDispatcherService.into(), ServiceGoodImpl.into()],
        vec![TestDispatcherService
            .artifact_id()
            .into_default_instance(TEST_SERVICE_ID, TEST_SERVICE_NAME)],
    );

    let artifact_id = ServiceGoodImpl.artifact_id();

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
        .contains(&artifact_id));
    execute_transaction(
        &mut blockchain,
        gen_keypair().test_deploy(TEST_SERVICE_ID, 1),
    )
    .unwrap();
    let snapshot = blockchain.snapshot();
    assert!(DispatcherSchema::new(&snapshot)
        .artifacts()
        .contains(&artifact_id));
    assert_eq!(snapshot.get_entry(IDX_NAME).get(), Some(1_u64));
}

#[test]
fn test_dispatcher_already_deployed() {
    let mut blockchain = create_blockchain(
        vec![TestDispatcherService.into(), ServiceGoodImpl.into()],
        vec![
            TestDispatcherService
                .artifact_id()
                .into_default_instance(TEST_SERVICE_ID, TEST_SERVICE_NAME),
            ServiceGoodImpl
                .artifact_id()
                .into_default_instance(11, "good"),
        ],
    );
    let artifact_id = ServiceGoodImpl.artifact_id();

    // Tests that we get an error if we try to deploy already deployed artifact.
    assert!(blockchain.dispatcher.is_artifact_deployed(&artifact_id));
    let err = blockchain
        .dispatcher
        .deploy_artifact(artifact_id.clone(), vec![])
        .wait()
        .unwrap_err();
    assert_eq!(
        err,
        ErrorMatch::from_fail(&DispatcherError::ArtifactAlreadyDeployed)
    );
    // Tests that we cannot register artifact twice.
    let res = execute_transaction(
        &mut blockchain,
        gen_keypair().test_deploy(TEST_SERVICE_ID, 1),
    );
    assert_eq!(
        res.unwrap_err(),
        ErrorMatch::from_fail(&DispatcherError::ArtifactAlreadyDeployed)
            .in_runtime(0)
            .for_service(TEST_SERVICE_ID)
    );
}

#[test]
#[should_panic(expected = "Unable to deploy registered artifact")]
fn test_dispatcher_register_unavailable() {
    let mut blockchain = create_blockchain(
        vec![TestDispatcherService.into(), ServiceGoodImpl.into()],
        vec![TestDispatcherService
            .artifact_id()
            .into_default_instance(TEST_SERVICE_ID, TEST_SERVICE_NAME)],
    );

    let artifact_id: ArtifactId = ServiceGoodImpl.artifact_id();
    blockchain
        .dispatcher
        .deploy_artifact(artifact_id.clone(), vec![])
        .wait()
        .unwrap();
    // Tests ExecutionError during the register artifact execution.
    execute_transaction(
        &mut blockchain,
        gen_keypair().test_deploy(TEST_SERVICE_ID, 42),
    )
    .unwrap_err();

    let snapshot = blockchain.snapshot();
    assert!(!DispatcherSchema::new(&snapshot)
        .artifacts()
        .contains(&artifact_id));
    assert!(!snapshot.get_entry::<_, u64>(IDX_NAME).exists());
    // Tests that an unavailable artifact will not be registered.
    execute_transaction(
        &mut blockchain,
        gen_keypair().test_deploy(TEST_SERVICE_ID, 24),
    )
    .unwrap();
}

#[test]
fn test_dispatcher_start_service_good() {
    let mut blockchain = create_blockchain(
        vec![TestDispatcherService.into()],
        vec![TestDispatcherService
            .artifact_id()
            .into_default_instance(TEST_SERVICE_ID, TEST_SERVICE_NAME)],
    );
    // Tests start service for the good service.
    let snapshot = blockchain.snapshot();
    assert!(!DispatcherSchema::new(&snapshot)
        .instances()
        .contains("good-service-1"));
    execute_transaction(&mut blockchain, gen_keypair().test_add(TEST_SERVICE_ID, 1)).unwrap();
    let snapshot = blockchain.snapshot();
    assert_eq!(
        DispatcherSchema::new(&snapshot)
            .instances()
            .get("good-service-1")
            .unwrap()
            .status,
        Some(InstanceStatus::Active)
    );
    assert_eq!(snapshot.get_entry(IDX_NAME).get(), Some(1_u64));

    execute_transaction(
        &mut blockchain,
        gen_keypair().test_stop(TEST_SERVICE_ID, TEST_SERVICE_ID),
    )
    .unwrap();

    let snapshot = blockchain.snapshot();
    assert_eq!(
        DispatcherSchema::new(&snapshot)
            .instances()
            .get(TEST_SERVICE_NAME)
            .unwrap()
            .status,
        Some(InstanceStatus::Stopped)
    );
}

#[test]
fn test_dispatcher_start_service_rollback() {
    let keypair = gen_keypair();
    let mut blockchain = create_blockchain(
        vec![TestDispatcherService.into()],
        vec![TestDispatcherService
            .artifact_id()
            .into_default_instance(TEST_SERVICE_ID, TEST_SERVICE_NAME)],
    );

    // Tests that a service with an unregistered artifact will not be started.
    let snapshot = blockchain.snapshot();
    assert!(!DispatcherSchema::new(&snapshot)
        .instances()
        .contains(&"good-service-24".to_owned()));
    let res = execute_transaction(&mut blockchain, keypair.test_add(TEST_SERVICE_ID, 24));
    assert_eq!(
        res.unwrap_err(),
        ErrorMatch::from_fail(&DispatcherError::ArtifactNotDeployed)
    );

    let snapshot = blockchain.snapshot();
    assert!(!DispatcherSchema::new(&snapshot)
        .instances()
        .contains(&"good-service-24".to_owned()));
    assert!(!snapshot.get_entry::<_, u64>(IDX_NAME).exists());

    // Tests that a service with panic during the configure will not be started.
    assert!(!DispatcherSchema::new(&snapshot)
        .instances()
        .contains(&"good-service-42".to_owned()));
    execute_transaction(&mut blockchain, keypair.test_add(TEST_SERVICE_ID, 42)).unwrap_err();
    let snapshot = blockchain.snapshot();
    assert!(!DispatcherSchema::new(&snapshot)
        .instances()
        .contains(&"good-service-42".to_owned()));
    assert!(!snapshot.get_entry::<_, u64>(IDX_NAME).exists());

    // Tests that a service with execution error during the initialization will not be started.
    assert!(!DispatcherSchema::new(&snapshot)
        .instances()
        .contains(&"good-service-18".to_owned()));
    execute_transaction(&mut blockchain, keypair.test_add(TEST_SERVICE_ID, 18)).unwrap_err();
    let snapshot = blockchain.snapshot();
    assert!(!DispatcherSchema::new(&snapshot)
        .instances()
        .contains(&"good-service-18".to_owned()));
    assert!(!snapshot.get_entry::<_, u64>(IDX_NAME).exists());
}

/// Checks that `Blockchain::check_tx` discards transactions with incorrect
/// instance IDs.
#[test]
fn test_check_tx() {
    let keypair = gen_keypair();
    let mut blockchain = create_blockchain(
        vec![TestDispatcherService.into()],
        vec![TestDispatcherService
            .artifact_id()
            .into_default_instance(TEST_SERVICE_ID, TEST_SERVICE_NAME)],
    );
    let snapshot = blockchain.snapshot();

    let correct_tx = keypair.test_add(TEST_SERVICE_ID, 1);
    Blockchain::check_tx(&snapshot, &correct_tx).unwrap();
    let incorrect_tx = keypair.test_add(TEST_SERVICE_ID + 1, 1);
    assert_eq!(
        Blockchain::check_tx(&snapshot, &incorrect_tx).unwrap_err(),
        ErrorMatch::from_fail(&DispatcherError::IncorrectInstanceId)
    );

    execute_transaction(
        &mut blockchain,
        keypair.test_stop(TEST_SERVICE_ID, TEST_SERVICE_ID),
    )
    .unwrap();
    // Check that previously correct transaction become incorrect.
    let snapshot = blockchain.snapshot();
    assert_eq!(
        Blockchain::check_tx(&snapshot, &correct_tx).unwrap_err(),
        ErrorMatch::from_fail(&DispatcherError::ServiceNotActive)
    );
}
