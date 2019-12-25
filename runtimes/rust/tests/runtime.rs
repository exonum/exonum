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
extern crate pretty_assertions;

use exonum::{
    blockchain::{
        config::{ConsensusConfig, GenesisConfig, GenesisConfigBuilder, InstanceInitParams},
        Blockchain, BlockchainBuilder, BlockchainMut, Schema as CoreSchema,
    },
    helpers::{generate_testnet_config, Height, ValidatorId},
    messages::{AnyTx, Verified},
    runtime::{
        ArtifactId, CallInfo, Caller, Dispatcher, DispatcherError, DispatcherSchema, ErrorKind,
        ErrorMatch, ExecutionContext, ExecutionError, InstanceId, InstanceSpec, InstanceStatus,
        Mailbox, Runtime, SnapshotExt, WellKnownRuntime, SUPERVISOR_INSTANCE_ID,
    },
};
use exonum_crypto::{Hash, PublicKey, PUBLIC_KEY_LENGTH};
use exonum_derive::exonum_interface;
use exonum_derive::*;
use exonum_merkledb::{
    access::AccessExt, BinaryValue, Fork, ObjectHash, Patch, Snapshot, SystemSchema,
};
use exonum_proto::ProtobufConvert;
use exonum_rust_runtime::{
    CallContext, RustRuntime, {DefaultInstance, Service, ServiceFactory},
};
use futures::{sync::mpsc, Future};
use serde_derive::*;

use std::{
    collections::BTreeMap,
    mem,
    sync::{Arc, Mutex, MutexGuard},
};

macro_rules! impl_binary_value_for_bincode {
    ($( $type:ty ),*) => {
        $(
            impl BinaryValue for $type {
                fn to_bytes(&self) -> Vec<u8> {
                    bincode::serialize(self).expect("Error while serializing value")
                }
                fn from_bytes(bytes: std::borrow::Cow<'_, [u8]>) -> Result<Self, failure::Error> {
                    bincode::deserialize(bytes.as_ref()).map_err(From::from)
                }
            }
        )*
    };
}

fn add_transactions_into_pool(
    blockchain: &mut BlockchainMut,
    txs: Vec<Verified<AnyTx>>,
) -> Vec<Hash> {
    blockchain
        .merge({
            let fork = blockchain.fork();
            let mut schema = CoreSchema::new(&fork);
            for tx in txs.clone() {
                schema.add_transaction_into_pool(tx);
            }
            fork.into_patch()
        })
        .unwrap();

    txs.into_iter().map(|x| x.object_hash()).collect()
}

fn execute_transaction(
    blockchain: &mut BlockchainMut,
    tx: Verified<AnyTx>,
) -> Result<(), ExecutionError> {
    let tx_hash = tx.object_hash();

    let (block_hash, patch) = create_block_with_transactions(blockchain, vec![tx]);
    blockchain
        .commit(patch, block_hash, vec![], &mut BTreeMap::new())
        .unwrap();

    let snapshot = blockchain.snapshot();
    let schema = CoreSchema::new(&snapshot);
    let location = schema.transactions_locations().get(&tx_hash).unwrap();
    schema.transaction_result(location).unwrap()
}

fn create_block_with_transactions(
    blockchain: &mut BlockchainMut,
    transactions: Vec<Verified<AnyTx>>,
) -> (Hash, Patch) {
    let tx_hashes = add_transactions_into_pool(blockchain, transactions);

    let height = {
        let snapshot = blockchain.snapshot();
        CoreSchema::new(&snapshot).next_height()
    };

    blockchain.create_patch(
        ValidatorId::zero().into(),
        height,
        &tx_hashes,
        &mut BTreeMap::new(),
    )
}

#[derive(Debug, PartialEq)]
enum RuntimeEvent {
    Initialize,
    Resume,
    BeforeTransactions(Height, InstanceId),
    DeployArtifact(ArtifactId, Vec<u8>),
    StartAdding(InstanceSpec, Vec<u8>),
    CommitService(Height, InstanceSpec, InstanceStatus),
    AfterTransactions(Height, InstanceId),
    AfterCommit(Height),
    Shutdown,
}

#[derive(Debug, Clone, Default)]
struct EventsHandle(Arc<Mutex<Vec<RuntimeEvent>>>);

impl EventsHandle {
    fn push(&self, event: RuntimeEvent) {
        self.0.lock().unwrap().push(event);
    }

    #[must_use]
    fn take(&self) -> Vec<RuntimeEvent> {
        self.0.lock().unwrap().drain(..).collect()
    }
}

/// Test runtime wrapper logging all the events (as `RuntimeEvent`) happening within it.
/// For service hooks the logged height is the height of the block **being processed**.
/// Other than logging, it just redirects all the calls to the inner runtime.
/// Used to test that workflow invariants are respected.
#[derive(Debug)]
struct Inspected<T> {
    runtime: T,
    events: EventsHandle,
}

impl<T: Runtime> Inspected<T> {
    fn new(runtime: T) -> Self {
        Self {
            runtime,
            events: Default::default(),
        }
    }
}

impl<T: Runtime> Runtime for Inspected<T> {
    fn initialize(&mut self, blockchain: &Blockchain) {
        self.events.push(RuntimeEvent::Initialize);
        self.runtime.initialize(blockchain)
    }

    fn on_resume(&mut self) {
        self.events.push(RuntimeEvent::Resume);
        self.runtime.on_resume()
    }

    fn deploy_artifact(
        &mut self,
        test_service_artifact: ArtifactId,
        deploy_spec: Vec<u8>,
    ) -> Box<dyn Future<Item = (), Error = ExecutionError>> {
        self.events.push(RuntimeEvent::DeployArtifact(
            test_service_artifact.clone(),
            deploy_spec.clone(),
        ));
        self.runtime
            .deploy_artifact(test_service_artifact, deploy_spec)
    }

    fn is_artifact_deployed(&self, id: &ArtifactId) -> bool {
        self.runtime.is_artifact_deployed(id)
    }

    fn initiate_adding_service(
        &self,
        context: ExecutionContext<'_>,
        spec: &InstanceSpec,
        parameters: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        self.events.push(RuntimeEvent::StartAdding(
            spec.to_owned(),
            parameters.clone(),
        ));
        self.runtime
            .initiate_adding_service(context, spec, parameters)
    }

    fn update_service_status(
        &mut self,
        snapshot: &dyn Snapshot,
        spec: &InstanceSpec,
        status: InstanceStatus,
    ) -> Result<(), ExecutionError> {
        snapshot
            .for_dispatcher()
            .get_instance(spec.id)
            .expect("Service instance should be exists");

        let core_schema = CoreSchema::new(snapshot);
        let height = core_schema.next_height();

        self.events
            .push(RuntimeEvent::CommitService(height, spec.to_owned(), status));
        self.runtime.update_service_status(snapshot, spec, status)
    }

    fn execute(
        &self,
        context: ExecutionContext<'_>,
        call_info: &CallInfo,
        arguments: &[u8],
    ) -> Result<(), ExecutionError> {
        self.runtime.execute(context, call_info, arguments)
    }

    fn before_transactions(
        &self,
        context: ExecutionContext<'_>,
        instance_id: u32,
    ) -> Result<(), ExecutionError> {
        let height = CoreSchema::new(&*context.fork).next_height();
        self.events
            .push(RuntimeEvent::BeforeTransactions(height, instance_id));
        self.runtime.after_transactions(context, instance_id)
    }

    fn after_transactions(
        &self,
        context: ExecutionContext<'_>,
        instance_id: u32,
    ) -> Result<(), ExecutionError> {
        let schema = CoreSchema::new(&*context.fork);
        let height = schema.next_height();
        self.events
            .push(RuntimeEvent::AfterTransactions(height, instance_id));
        self.runtime.after_transactions(context, instance_id)
    }

    fn after_commit(&mut self, snapshot: &dyn Snapshot, mailbox: &mut Mailbox) {
        let height = CoreSchema::new(snapshot).next_height();
        self.events.push(RuntimeEvent::AfterCommit(height));
        self.runtime.after_commit(snapshot, mailbox);
    }

    fn shutdown(&mut self) {
        self.events.push(RuntimeEvent::Shutdown);
        self.runtime.shutdown();
    }
}

impl WellKnownRuntime for Inspected<RustRuntime> {
    const ID: u32 = RustRuntime::ID;
}

#[derive(Debug, Serialize, Deserialize)]
struct DeployArtifact {
    test_service_artifact: ArtifactId,
    spec: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
struct StartService {
    spec: InstanceSpec,
    constructor: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
struct StopService {
    instance_id: InstanceId,
}

impl_binary_value_for_bincode! { DeployArtifact, StartService, StopService }

#[exonum_interface]
trait ToySupervisor<Ctx> {
    type Output;

    fn deploy_artifact(&self, context: Ctx, request: DeployArtifact) -> Self::Output;
    fn start_service(&self, context: Ctx, request: StartService) -> Self::Output;
    fn stop_service(&self, context: Ctx, request: StopService) -> Self::Output;
}

#[derive(Debug, ServiceFactory, ServiceDispatcher)]
#[service_dispatcher(implements("ToySupervisor"))]
#[service_factory(artifact_name = "toy_supervisor", artifact_version = "0.1.0")]
struct ToySupervisorService;

impl ToySupervisor<CallContext<'_>> for ToySupervisorService {
    type Output = Result<(), ExecutionError>;

    fn deploy_artifact(
        &self,
        mut context: CallContext<'_>,
        request: DeployArtifact,
    ) -> Self::Output {
        context
            .supervisor_extensions()
            .start_artifact_registration(request.test_service_artifact, request.spec)
    }

    fn start_service(&self, mut context: CallContext<'_>, request: StartService) -> Self::Output {
        context
            .supervisor_extensions()
            .initiate_adding_service(request.spec, request.constructor)
    }

    fn stop_service(&self, mut context: CallContext<'_>, request: StopService) -> Self::Output {
        context
            .supervisor_extensions()
            .initiate_stopping_service(request.instance_id)
    }
}

impl Service for ToySupervisorService {}

impl DefaultInstance for ToySupervisorService {
    const INSTANCE_ID: u32 = SUPERVISOR_INSTANCE_ID;
    const INSTANCE_NAME: &'static str = "supervisor";

    fn default_instance(&self) -> InstanceInitParams {
        self.artifact_id()
            .into_default_instance(Self::INSTANCE_ID, Self::INSTANCE_NAME)
            .with_constructor(Init::default())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Init {
    msg: String,
}

impl_binary_value_for_bincode! { Init }

impl Default for Init {
    fn default() -> Self {
        Self {
            msg: "constructor_message".to_owned(),
        }
    }
}

#[exonum_interface]
trait Test<Ctx> {
    type Output;
    fn method_a(&self, ctx: Ctx, arg: u64) -> Self::Output;
    fn method_b(&self, ctx: Ctx, arg: u64) -> Self::Output;
}

#[derive(Debug, ServiceFactory, ServiceDispatcher)]
#[service_dispatcher(implements("Test"))]
#[service_factory(artifact_name = "test_service", artifact_version = "0.1.0")]
pub struct TestServiceImpl;

impl Test<CallContext<'_>> for TestServiceImpl {
    type Output = Result<(), ExecutionError>;

    fn method_a(&self, mut ctx: CallContext<'_>, arg: u64) -> Result<(), ExecutionError> {
        ctx.service_data()
            .get_proof_entry("method_a_entry")
            .set(arg);
        // Test calling one service from another.
        ctx.method_b(TestServiceImpl::INSTANCE_NAME, arg)
    }

    fn method_b(&self, ctx: CallContext<'_>, arg: u64) -> Result<(), ExecutionError> {
        ctx.service_data()
            .get_proof_entry("method_b_entry")
            .set(arg);
        Ok(())
    }
}

impl Service for TestServiceImpl {
    fn initialize(&self, context: CallContext<'_>, params: Vec<u8>) -> Result<(), ExecutionError> {
        let init = Init::from_bytes(params.into()).map_err(DispatcherError::malformed_arguments)?;
        context
            .service_data()
            .get_proof_entry("constructor_entry")
            .set(init.msg);
        Ok(())
    }
}

impl DefaultInstance for TestServiceImpl {
    const INSTANCE_ID: u32 = 2;
    const INSTANCE_NAME: &'static str = "test_service_name";

    fn default_instance(&self) -> InstanceInitParams {
        self.artifact_id()
            .into_default_instance(Self::INSTANCE_ID, Self::INSTANCE_NAME)
            .with_constructor(Init::default())
    }
}

#[derive(Debug, ServiceFactory, ServiceDispatcher)]
#[service_dispatcher(implements("Test"))]
#[service_factory(artifact_name = "test_service", artifact_version = "0.2.0")]
pub struct TestServiceImplV2;

impl Test<CallContext<'_>> for TestServiceImplV2 {
    type Output = Result<(), ExecutionError>;

    fn method_a(&self, _context: CallContext<'_>, _arg: u64) -> Self::Output {
        Err(DispatcherError::NoSuchMethod.into())
    }

    fn method_b(&self, context: CallContext<'_>, arg: u64) -> Self::Output {
        context
            .service_data()
            .get_proof_entry("method_b_entry")
            .set(arg + 42);
        Ok(())
    }
}

impl Service for TestServiceImplV2 {}

impl DefaultInstance for TestServiceImplV2 {
    const INSTANCE_ID: u32 = 3;
    const INSTANCE_NAME: &'static str = "new_service";

    fn default_instance(&self) -> InstanceInitParams {
        self.artifact_id()
            .into_default_instance(Self::INSTANCE_ID, Self::INSTANCE_NAME)
            .with_constructor(Init::default())
    }
}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_dispatcher(implements())]
#[service_factory(artifact_name = "dependent_service", artifact_version = "0.1.0")]
pub struct DependentServiceImpl;

impl Service for DependentServiceImpl {
    fn initialize(&self, context: CallContext<'_>, params: Vec<u8>) -> Result<(), ExecutionError> {
        // Due to the fact that our toy supervisor immediately executes start / stop requests,
        // caller might be `ToySupervisorService::INSTANCE_ID`.
        match *context.caller() {
            Caller::Blockchain => {}
            Caller::Service { instance_id } if instance_id == ToySupervisorService::INSTANCE_ID => {
            }
            other => panic!("Wrong caller type: {:?}", other),
        }

        let init = Init::from_bytes(params.into()).map_err(DispatcherError::malformed_arguments)?;
        if context
            .data()
            .for_dispatcher()
            .get_instance(&*init.msg)
            .is_none()
        {
            return Err(ExecutionError::service(0, "no dependency"));
        }

        // Check that it is possible to access data of the dependency right away,
        // even if it is deployed in the same block.
        let dependency_data = context
            .data()
            .for_service(&*init.msg)
            .expect("Dependency exists, but its data does not");
        assert!(dependency_data
            .get_proof_entry::<_, String>("constructor_entry")
            .exists());

        Ok(())
    }
}

impl DefaultInstance for DependentServiceImpl {
    const INSTANCE_ID: u32 = TestServiceImpl::INSTANCE_ID + 1;
    const INSTANCE_NAME: &'static str = "dependent-service";

    fn default_instance(&self) -> InstanceInitParams {
        self.artifact_id()
            .into_default_instance(Self::INSTANCE_ID, Self::INSTANCE_NAME)
            .with_constructor(Init {
                msg: TestServiceImpl::INSTANCE_NAME.to_owned(),
            })
    }
}

fn create_genesis_config_builder() -> GenesisConfigBuilder {
    let consensus_config = generate_testnet_config(1, 0)[0].clone().consensus;
    GenesisConfigBuilder::with_consensus_config(consensus_config)
}

fn create_genesis_config_with_supervisor() -> GenesisConfig {
    create_genesis_config_builder()
        .with_artifact(ToySupervisorService.artifact_id())
        .with_instance(ToySupervisorService.default_instance())
        .build()
}

fn create_runtime(
    blockchain: Blockchain,
    genesis_config: GenesisConfig,
) -> Result<(BlockchainMut, EventsHandle), failure::Error> {
    let inspected = Inspected::new(
        RustRuntime::new(mpsc::channel(1).0)
            .with_available_service(TestServiceImpl)
            .with_available_service(TestServiceImplV2)
            .with_available_service(ToySupervisorService)
            .with_available_service(DependentServiceImpl),
    );
    let events_handle = inspected.events.clone();

    BlockchainBuilder::new(blockchain, genesis_config)
        .with_runtime(inspected)
        .build()
        .map(|blockchain| (blockchain, events_handle))
}

/// In this test, we manually instruct the dispatcher to deploy artifacts / create / stop services
/// instead of using transactions. We still need to create patches using a `BlockchainMut`
/// in order to properly emulate the blockchain workflow.
#[test]
fn basic_runtime_workflow() {
    // Create a runtime and a service test_service_artifact.
    let (mut blockchain, events_handle) = create_runtime(
        Blockchain::build_for_tests(),
        create_genesis_config_with_supervisor(),
    )
    .unwrap();
    let keypair = blockchain.as_ref().service_keypair().clone();

    // The dispatcher should initialize the runtime and call `after_commit` for
    // the genesis block.
    let supervisor = ToySupervisorService.default_instance();
    assert_eq!(
        events_handle.take(),
        vec![
            RuntimeEvent::Initialize,
            RuntimeEvent::DeployArtifact(ToySupervisorService.artifact_id(), vec![]),
            RuntimeEvent::StartAdding(supervisor.instance_spec.clone(), supervisor.constructor),
            RuntimeEvent::CommitService(
                Height(0),
                supervisor.instance_spec.clone(),
                InstanceStatus::Active
            ),
            RuntimeEvent::AfterTransactions(Height(0), ToySupervisorService::INSTANCE_ID),
            RuntimeEvent::AfterCommit(Height(1))
        ]
    );

    // Deploy service test_service_artifact.
    let test_service_artifact = TestServiceImpl.artifact_id();
    execute_transaction(
        &mut blockchain,
        keypair.deploy_artifact(
            ToySupervisorService::INSTANCE_ID,
            DeployArtifact {
                test_service_artifact: test_service_artifact.clone(),
                spec: vec![],
            },
        ),
    )
    .unwrap();

    assert_eq!(
        events_handle.take(),
        vec![
            RuntimeEvent::BeforeTransactions(Height(1), ToySupervisorService::INSTANCE_ID),
            RuntimeEvent::AfterTransactions(Height(1), ToySupervisorService::INSTANCE_ID),
            RuntimeEvent::DeployArtifact(test_service_artifact, vec![]),
            RuntimeEvent::AfterCommit(Height(2)),
        ]
    );

    // Add service instance.
    let test_instance = TestServiceImpl.default_instance();
    execute_transaction(
        &mut blockchain,
        keypair.start_service(
            ToySupervisorService::INSTANCE_ID,
            StartService {
                spec: test_instance.instance_spec.clone(),
                constructor: test_instance.constructor.clone(),
            },
        ),
    )
    .unwrap();

    assert_eq!(
        events_handle.take(),
        // The service is not active at the beginning of the block, so `after_transactions`
        // and `before_transactions` should not be called for it.
        vec![
            RuntimeEvent::BeforeTransactions(Height(2), ToySupervisorService::INSTANCE_ID),
            RuntimeEvent::StartAdding(
                test_instance.instance_spec.clone(),
                test_instance.constructor
            ),
            RuntimeEvent::AfterTransactions(Height(2), ToySupervisorService::INSTANCE_ID),
            RuntimeEvent::CommitService(
                Height(3),
                test_instance.instance_spec.clone(),
                InstanceStatus::Active
            ),
            RuntimeEvent::AfterCommit(Height(3)),
        ]
    );

    assert_eq!(
        blockchain
            .snapshot()
            .for_service(TestServiceImpl::INSTANCE_NAME)
            .unwrap()
            .get_proof_entry("constructor_entry")
            .get(),
        Some(Init::default().msg)
    );

    // Execute transaction method A.
    let arg_a = 11;
    execute_transaction(
        &mut blockchain,
        keypair.method_a(TestServiceImpl::INSTANCE_ID, arg_a),
    )
    .unwrap();
    // Check usual events from runtime.
    assert_eq!(
        events_handle.take(),
        vec![
            RuntimeEvent::BeforeTransactions(Height(3), ToySupervisorService::INSTANCE_ID),
            RuntimeEvent::BeforeTransactions(Height(3), TestServiceImpl::INSTANCE_ID),
            RuntimeEvent::AfterTransactions(Height(3), ToySupervisorService::INSTANCE_ID),
            RuntimeEvent::AfterTransactions(Height(3), TestServiceImpl::INSTANCE_ID),
            RuntimeEvent::AfterCommit(Height(4)),
        ]
    );
    // Check instance data modifications.
    {
        let snapshot = blockchain.snapshot();
        let data = snapshot
            .for_service(TestServiceImpl::INSTANCE_NAME)
            .unwrap();
        assert_eq!(
            data.clone().get_proof_entry("method_a_entry").get(),
            Some(arg_a)
        );
        assert_eq!(data.get_proof_entry("method_b_entry").get(), Some(arg_a));
    }

    // Execute transaction method B.
    let arg_b = 22;
    execute_transaction(
        &mut blockchain,
        keypair.method_b(TestServiceImpl::INSTANCE_ID, arg_b),
    )
    .unwrap();
    // Discard usual events from runtime.
    drop(events_handle.take());
    // Check instance data modifications.
    {
        let snapshot = blockchain.snapshot();
        let data = snapshot
            .for_service(TestServiceImpl::INSTANCE_NAME)
            .unwrap();
        assert_eq!(
            data.clone().get_proof_entry("method_a_entry").get(),
            Some(arg_a)
        );
        assert_eq!(data.get_proof_entry("method_b_entry").get(), Some(arg_b));
    }

    // Stop service instance.
    execute_transaction(
        &mut blockchain,
        keypair.stop_service(
            ToySupervisorService::INSTANCE_ID,
            StopService {
                instance_id: TestServiceImpl::INSTANCE_ID,
            },
        ),
    )
    .unwrap();
    assert_eq!(
        events_handle.take(),
        vec![
            RuntimeEvent::BeforeTransactions(Height(5), ToySupervisorService::INSTANCE_ID),
            RuntimeEvent::BeforeTransactions(Height(5), TestServiceImpl::INSTANCE_ID),
            RuntimeEvent::AfterTransactions(Height(5), ToySupervisorService::INSTANCE_ID),
            RuntimeEvent::AfterTransactions(Height(5), TestServiceImpl::INSTANCE_ID),
            RuntimeEvent::CommitService(
                Height(6),
                test_instance.instance_spec.clone(),
                InstanceStatus::Stopped,
            ),
            RuntimeEvent::AfterCommit(Height(6)),
        ]
    );

    execute_transaction(
        &mut blockchain,
        keypair.method_a(TestServiceImpl::INSTANCE_ID, 0),
    )
    .expect_err("incorrect transaction");
}

/// In this test, we simulate blockchain restart and check events from inspector.
#[test]
fn runtime_restart() {
    // Create a runtime and a service test_service_artifact.
    let genesis_config = create_genesis_config_with_supervisor();
    let (mut blockchain, events_handle) =
        create_runtime(Blockchain::build_for_tests(), genesis_config.clone()).unwrap();
    let keypair = blockchain.as_ref().service_keypair().clone();

    // The dispatcher should initialize the runtime and call `after_commit` for
    // the genesis block.
    let supervisor = ToySupervisorService.default_instance();
    assert_eq!(
        events_handle.take(),
        vec![
            RuntimeEvent::Initialize,
            RuntimeEvent::DeployArtifact(ToySupervisorService.artifact_id(), vec![]),
            RuntimeEvent::StartAdding(supervisor.instance_spec.clone(), supervisor.constructor),
            RuntimeEvent::CommitService(
                Height(0),
                supervisor.instance_spec.clone(),
                InstanceStatus::Active
            ),
            RuntimeEvent::AfterTransactions(Height(0), ToySupervisorService::INSTANCE_ID),
            RuntimeEvent::AfterCommit(Height(1))
        ]
    );

    // Deploy service test_service_artifact.
    let test_service_artifact = TestServiceImpl.artifact_id();
    execute_transaction(
        &mut blockchain,
        keypair.deploy_artifact(
            ToySupervisorService::INSTANCE_ID,
            DeployArtifact {
                test_service_artifact: test_service_artifact.clone(),
                spec: vec![],
            },
        ),
    )
    .unwrap();

    assert_eq!(
        events_handle.take(),
        vec![
            RuntimeEvent::BeforeTransactions(Height(1), ToySupervisorService::INSTANCE_ID),
            RuntimeEvent::AfterTransactions(Height(1), ToySupervisorService::INSTANCE_ID),
            RuntimeEvent::DeployArtifact(test_service_artifact.clone(), vec![]),
            RuntimeEvent::AfterCommit(Height(2)),
        ]
    );

    // Emulate node restart.
    let (mut blockchain, events_handle) =
        create_runtime(blockchain.as_ref().clone(), genesis_config).unwrap();
    let keypair = blockchain.as_ref().service_keypair().clone();

    assert_eq!(
        events_handle.take(),
        vec![
            RuntimeEvent::Initialize,
            RuntimeEvent::DeployArtifact(test_service_artifact, vec![]),
            RuntimeEvent::DeployArtifact(ToySupervisorService.artifact_id(), vec![]),
            // `Runtime::start_adding_service` is never called for the same service
            RuntimeEvent::CommitService(
                Height(2),
                supervisor.instance_spec.clone(),
                InstanceStatus::Active
            ),
            // `Runtime::after_commit` is never called for the same block
            RuntimeEvent::Resume,
        ]
    );

    // Execute supervisor transaction.
    let test_instance = TestServiceImpl.default_instance();
    execute_transaction(
        &mut blockchain,
        keypair.start_service(
            ToySupervisorService::INSTANCE_ID,
            StartService {
                spec: test_instance.instance_spec.clone(),
                constructor: test_instance.constructor.clone(),
            },
        ),
    )
    .unwrap();
}

#[test]
fn state_aggregation() {
    // Create a runtime and a service test_service_artifact.
    let genesis_config = create_genesis_config_builder()
        .with_artifact(TestServiceImpl.artifact_id())
        .with_instance(TestServiceImpl.default_instance())
        .build();
    let (blockchain, _) = create_runtime(Blockchain::build_for_tests(), genesis_config).unwrap();

    // The constructor entry has been written to; `method_*` `ProofEntry`s are empty.
    let snapshot = blockchain.snapshot();
    assert_eq!(
        SystemSchema::new(&snapshot)
            .state_aggregator()
            .keys()
            .collect::<Vec<_>>(),
        vec![
            "core.consensus_config",
            "dispatcher_artifacts",
            "dispatcher_instances",
            "test_service_name.constructor_entry"
        ]
    );
}

// Create a blockchain instance with two service instances with the same `Test` interface,
// but with different implementation versions.
#[test]
fn multiple_service_versions() {
    let genesis_config = create_genesis_config_builder()
        .with_artifact(TestServiceImpl.artifact_id())
        .with_artifact(TestServiceImplV2.artifact_id())
        .with_instance(TestServiceImpl.default_instance())
        .with_instance(TestServiceImplV2.default_instance())
        .build();
    let (mut blockchain, _) =
        create_runtime(Blockchain::build_for_tests(), genesis_config).unwrap();
    let keypair = blockchain.as_ref().service_keypair().clone();

    // Check that both test_service_artifact versions are present in the dispatcher schema.
    {
        let snapshot = blockchain.snapshot();
        let schema = snapshot.for_dispatcher();
        assert!(schema
            .get_artifact(&TestServiceImpl.artifact_id())
            .is_some());
        assert!(schema
            .get_artifact(&TestServiceImplV2.artifact_id())
            .is_some());
        assert!(schema
            .get_instance(TestServiceImpl::INSTANCE_NAME)
            .is_some());
        assert!(schema
            .get_instance(TestServiceImplV2::INSTANCE_NAME)
            .is_some());
    }
    // Check that both services are active by calling transactions for them.
    execute_transaction(
        &mut blockchain,
        keypair.method_a(TestServiceImpl::INSTANCE_ID, 11),
    )
    .unwrap();
    let err = execute_transaction(
        &mut blockchain,
        keypair.method_a(TestServiceImplV2::INSTANCE_ID, 11),
    )
    .unwrap_err();
    // `method_a` is removed from the newer service version.
    assert_eq!(err, ErrorMatch::from_fail(&DispatcherError::NoSuchMethod));

    {
        let snapshot = blockchain.snapshot();
        assert_eq!(
            snapshot
                .for_service(TestServiceImpl::INSTANCE_NAME)
                .unwrap()
                .get_proof_entry("method_a_entry")
                .get(),
            Some(11)
        );
        assert!(!snapshot
            .for_service(TestServiceImplV2::INSTANCE_NAME)
            .unwrap()
            .get_proof_entry::<_, u64>("method_a_entry")
            .exists());
    }

    // Check method_a in a `TestServiceImplV2` instance.
    execute_transaction(
        &mut blockchain,
        keypair.method_b(TestServiceImplV2::INSTANCE_ID, 12),
    )
    .unwrap();

    {
        let snapshot = blockchain.snapshot();
        assert_eq!(
            snapshot
                .for_service(TestServiceImplV2::INSTANCE_NAME)
                .unwrap()
                .get_proof_entry("method_b_entry")
                .get(),
            Some(54) // 12 + 42
        );
    }
}

#[test]
fn conflicting_service_instances() {
    let (mut blockchain, events_handle) = create_runtime(
        Blockchain::build_for_tests(),
        create_genesis_config_with_supervisor(),
    )
    .unwrap();
    let keypair = blockchain.as_ref().service_keypair().clone();

    // Deploy service test_service_artifact.
    let test_service_artifact = TestServiceImpl.artifact_id();
    execute_transaction(
        &mut blockchain,
        keypair.deploy_artifact(
            ToySupervisorService::INSTANCE_ID,
            DeployArtifact {
                test_service_artifact: test_service_artifact.clone(),
                spec: vec![],
            },
        ),
    )
    .unwrap();
    drop(events_handle.take());

    // Fork #1.
    let init_params = TestServiceImpl.default_instance();
    let patch = create_block_with_transactions(
        &mut blockchain,
        vec![keypair.start_service(
            ToySupervisorService::INSTANCE_ID,
            StartService {
                spec: init_params.clone().instance_spec,
                constructor: init_params.clone().constructor,
            },
        )],
    );

    assert_eq!(
        events_handle.take(),
        vec![
            RuntimeEvent::BeforeTransactions(Height(2), ToySupervisorService::INSTANCE_ID),
            RuntimeEvent::StartAdding(
                init_params.instance_spec.clone(),
                init_params.constructor.clone()
            ),
            RuntimeEvent::AfterTransactions(Height(2), ToySupervisorService::INSTANCE_ID),
        ]
    );

    // Fork #2: same service, but with different ID.
    let mut init_params_2 = TestServiceImpl.default_instance();
    init_params_2.instance_spec.id += 1;
    let _alternative_patch = create_block_with_transactions(
        &mut blockchain,
        vec![keypair.start_service(
            ToySupervisorService::INSTANCE_ID,
            StartService {
                spec: init_params_2.clone().instance_spec,
                constructor: init_params_2.clone().constructor,
            },
        )],
    );

    assert_eq!(
        events_handle.take(),
        vec![
            RuntimeEvent::BeforeTransactions(Height(2), ToySupervisorService::INSTANCE_ID),
            RuntimeEvent::StartAdding(
                init_params_2.instance_spec.clone(),
                init_params_2.constructor
            ),
            RuntimeEvent::AfterTransactions(Height(2), ToySupervisorService::INSTANCE_ID),
        ]
    );

    // Commit first fork.
    blockchain
        .commit(patch.1, patch.0, vec![], &mut BTreeMap::new())
        .unwrap();

    assert_eq!(
        events_handle.take(),
        vec![
            RuntimeEvent::CommitService(
                Height(3),
                init_params.instance_spec.clone(),
                InstanceStatus::Active
            ),
            RuntimeEvent::AfterCommit(Height(3)),
        ]
    );

    // Check that the added service is accessible only by its proper ID.
    execute_transaction(
        &mut blockchain,
        keypair.method_a(TestServiceImpl::INSTANCE_ID, 11),
    )
    .unwrap();
    let err = execute_transaction(
        &mut blockchain,
        keypair.method_a(TestServiceImpl::INSTANCE_ID + 1, 11),
    )
    .unwrap_err();
    // Alternative instance was discarded.
    assert_eq!(
        err,
        ErrorMatch::from_fail(&DispatcherError::IncorrectInstanceId)
    );
}

#[test]
fn dependent_builtin_service() {
    let main_service = TestServiceImpl;
    let dep_service = DependentServiceImpl;

    // Create a blockchain with both main and dependent services initialized in the genesis block.
    let genesis_config = create_genesis_config_builder()
        .with_artifact(main_service.artifact_id())
        .with_instance(main_service.default_instance())
        .with_artifact(dep_service.artifact_id())
        .with_instance(dep_service.default_instance())
        .build();

    let (blockchain, _) = create_runtime(Blockchain::build_for_tests(), genesis_config).unwrap();

    let snapshot = blockchain.snapshot();
    let schema = snapshot.for_dispatcher();
    assert_eq!(
        schema
            .get_instance(TestServiceImpl::INSTANCE_ID)
            .unwrap()
            .status
            .unwrap(),
        InstanceStatus::Active
    );
    assert_eq!(
        schema
            .get_instance(DependentServiceImpl::INSTANCE_ID)
            .unwrap()
            .status
            .unwrap(),
        InstanceStatus::Active
    );
}

#[test]
fn dependent_builtin_service_with_incorrect_order() {
    let main_service = TestServiceImpl;
    let dep_service = DependentServiceImpl;

    // Error in the service instantiation in the genesis block bubbles up.
    let genesis_config = create_genesis_config_builder()
        .with_artifact(main_service.artifact_id())
        .with_artifact(dep_service.artifact_id())
        .with_instance(dep_service.default_instance()) // <-- Incorrect service ordering
        .with_instance(main_service.default_instance())
        .build();

    let err = create_runtime(Blockchain::build_for_tests(), genesis_config).unwrap_err();
    assert!(err.to_string().contains("no dependency"));
}

#[test]
fn dependent_service_with_no_dependency() {
    let (mut blockchain, _) = create_runtime(
        Blockchain::build_for_tests(),
        create_genesis_config_with_supervisor(),
    )
    .unwrap();
    let keypair = blockchain.as_ref().service_keypair().clone();

    // Deploy dependent service test_service_artifact.
    execute_transaction(
        &mut blockchain,
        keypair.deploy_artifact(
            ToySupervisorService::INSTANCE_ID,
            DeployArtifact {
                test_service_artifact: DependentServiceImpl.artifact_id(),
                spec: vec![],
            },
        ),
    )
    .unwrap();

    // Try to add dependent service instance.
    let dep_instance = DependentServiceImpl.default_instance();
    let err = execute_transaction(
        &mut blockchain,
        keypair.start_service(
            ToySupervisorService::INSTANCE_ID,
            StartService {
                spec: dep_instance.instance_spec.clone(),
                constructor: dep_instance.constructor.clone(),
            },
        ),
    )
    .unwrap_err();

    assert_eq!(err, ExecutionError::service(0, "no dependency").to_match());
    // Check that the information about the service hasn't persisted in the dispatcher schema.
    let snapshot = blockchain.snapshot();
    assert!(snapshot
        .for_dispatcher()
        .get_instance(DependentServiceImpl::INSTANCE_NAME)
        .is_none());
}

// #[test]
// fn dependent_service_in_same_block() {
//     let runtime = RustRuntime::new(mpsc::channel(1).0)
//         .with_factory(TestServiceImpl)
//         .with_factory(DependentServiceImpl);

//     let config = generate_testnet_config(1, 0)[0].clone();
//     let genesis_config = GenesisConfigBuilder::with_consensus_config(config.consensus).build();

//     let mut blockchain = Blockchain::build_for_tests()
//         .into_mut(genesis_config)
//         .with_runtime(runtime)
//         .build()
//         .unwrap();

//     // Artifacts need to be deployed in a separate block due to checks in `RustRuntime`.
//     let fork = create_block(&blockchain);
//     let main_inst = TestServiceImpl.default_instance();
//     let dep_inst = DependentServiceImpl.default_instance();
//     Dispatcher::commit_artifact(&fork, main_inst.instance_spec.test_service_artifact.clone(), vec![]).unwrap();
//     Dispatcher::commit_artifact(&fork, dep_inst.instance_spec.test_service_artifact.clone(), vec![]).unwrap();
//     commit_block(&mut blockchain, fork);

//     // Deploy both services in the same block after genesis.
//     let mut fork = create_block(&blockchain);
//     let mut ctx = ExecutionContext::new(blockchain.dispatcher(), &mut fork, Caller::Blockchain);
//     ctx.initiate_adding_service(main_inst.instance_spec, main_inst.constructor)
//         .unwrap();
//     ctx.initiate_adding_service(dep_inst.instance_spec, dep_inst.constructor)
//         .unwrap();
//     commit_block(&mut blockchain, fork);

//     let snapshot = blockchain.snapshot();
//     let schema = DispatcherSchema::new(&snapshot);
//     assert_eq!(
//         schema
//             .get_instance("dependent-service")
//             .unwrap()
//             .status
//             .unwrap(),
//         InstanceStatus::Active
//     );
// }

// #[test]
// fn dependent_service_in_successive_block() {
//     let main_service = TestServiceImpl;
//     let dep_service = DependentServiceImpl;
//     let genesis_config = TestServiceImpl::genesis_config();

//     let runtime = RustRuntime::new(mpsc::channel(1).0)
//         .with_factory(main_service)
//         .with_factory(dep_service);

//     let mut blockchain = Blockchain::build_for_tests()
//         .into_mut(genesis_config)
//         .with_runtime(runtime)
//         .build()
//         .unwrap();

//     let fork = create_block(&blockchain);
//     let dep_spec = DependentServiceImpl.default_instance();
//     Dispatcher::commit_artifact(&fork, dep_spec.instance_spec.test_service_artifact.clone(), vec![]).unwrap();
//     commit_block(&mut blockchain, fork);

//     let mut fork = create_block(&blockchain);
//     ExecutionContext::new(blockchain.dispatcher(), &mut fork, Caller::Blockchain)
//         .initiate_adding_service(dep_spec.instance_spec, dep_spec.constructor)
//         .unwrap();
//     commit_block(&mut blockchain, fork);

//     let snapshot = blockchain.snapshot();
//     let schema = DispatcherSchema::new(&snapshot);
//     assert_eq!(
//         schema
//             .get_instance("dependent-service")
//             .unwrap()
//             .status
//             .unwrap(),
//         InstanceStatus::Active
//     );
// }
