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

use exonum_crypto::{Hash, PublicKey, PUBLIC_KEY_LENGTH};
use exonum_derive::{exonum_interface, BinaryValue, ServiceDispatcher, ServiceFactory};
use exonum_merkledb::{access::AccessExt, BinaryValue, Fork, Snapshot, SystemSchema};
use exonum_proto::ProtobufConvert;
use futures::{sync::mpsc, Future};

use std::{
    collections::BTreeMap,
    mem,
    sync::{Arc, Mutex},
};

use crate::{
    blockchain::{
        config::{GenesisConfig, GenesisConfigBuilder, InstanceInitParams},
        Blockchain, BlockchainMut, Schema as CoreSchema,
    },
    helpers::{generate_testnet_config, Height, ValidatorId},
    proto::schema::tests::TestServiceInit,
    runtime::{
        CallInfo, Caller, Dispatcher, DispatcherError, DispatcherSchema, ErrorMatch,
        ExecutionContext, ExecutionError, InstanceId, InstanceSpec, InstanceStatus, Mailbox,
        Runtime, WellKnownRuntime,
    },
};

use super::{
    service::{DefaultInstance, Service, ServiceFactory},
    ArtifactId, CallContext, RustRuntime,
};

const SERVICE_INSTANCE_ID: InstanceId = 2;
const SERVICE_INSTANCE_NAME: &str = "test_service_name";

fn create_block(blockchain: &BlockchainMut) -> Fork {
    let height = CoreSchema::new(&blockchain.snapshot()).height();
    let (_, patch) = blockchain.create_patch(
        ValidatorId(0).into(),
        height.next(),
        &[],
        &mut BTreeMap::new(),
    );
    Fork::from(patch)
}

fn commit_block(blockchain: &mut BlockchainMut, fork: Fork) {
    // Since `BlockchainMut::create_patch` invocation in `create_block` does not use transactions,
    // the `after_transactions` hook does not change artifact / service statuses. Thus, we need to call
    // `activate_pending` manually.
    // FIXME: Fix this behavior [ECR-3222]
    blockchain.dispatcher().activate_pending(&fork);
    // Get state hash from the block proposal.
    let patch = fork.into_patch();
    let state_hash_in_patch = SystemSchema::new(&patch).state_hash();

    // Commit block to the blockchain.
    blockchain
        .commit(patch, Hash::zero(), vec![], &mut BTreeMap::new())
        .unwrap();

    // Make sure that the state hash is the same before and after the block is committed.
    let snapshot = blockchain.snapshot();
    let state_hash_in_block = SystemSchema::new(&snapshot).state_hash();
    assert_eq!(state_hash_in_block, state_hash_in_patch);
}

fn create_runtime() -> (Inspected<RustRuntime>, Arc<Mutex<Vec<RuntimeEvent>>>) {
    let runtime = RustRuntime::new(mpsc::channel(1).0).with_factory(TestServiceImpl);
    let event_handle = Arc::default();
    let runtime = Inspected {
        inner: runtime,
        events: Arc::clone(&event_handle),
    };
    (runtime, event_handle)
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

/// Test runtime wrapper logging all the events (as `RuntimeEvent`) happening within it.
/// For service hooks the logged height is the height of the block **being processed**.
/// Other than logging, it just redirects all the calls to the inner runtime.
/// Used to test that workflow invariants are respected.
#[derive(Debug, Clone)]
struct Inspected<T> {
    inner: T,
    events: Arc<Mutex<Vec<RuntimeEvent>>>,
}

impl<T: Runtime> Runtime for Inspected<T> {
    fn initialize(&mut self, blockchain: &Blockchain) {
        self.events.lock().unwrap().push(RuntimeEvent::Initialize);
        self.inner.initialize(blockchain);
    }

    fn on_resume(&mut self) {
        self.events.lock().unwrap().push(RuntimeEvent::Resume);
        self.inner.on_resume();
    }

    fn deploy_artifact(
        &mut self,
        artifact: ArtifactId,
        deploy_spec: Vec<u8>,
    ) -> Box<dyn Future<Item = (), Error = ExecutionError>> {
        self.events
            .lock()
            .unwrap()
            .push(RuntimeEvent::DeployArtifact(
                artifact.clone(),
                deploy_spec.clone(),
            ));
        self.inner.deploy_artifact(artifact, deploy_spec)
    }

    fn is_artifact_deployed(&self, id: &ArtifactId) -> bool {
        self.inner.is_artifact_deployed(id)
    }

    fn initiate_adding_service(
        &self,
        context: ExecutionContext<'_>,
        spec: &InstanceSpec,
        parameters: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        self.events.lock().unwrap().push(RuntimeEvent::StartAdding(
            spec.to_owned(),
            parameters.clone(),
        ));
        self.inner
            .initiate_adding_service(context, spec, parameters)
    }

    fn update_service_status(
        &mut self,
        snapshot: &dyn Snapshot,
        spec: &InstanceSpec,
        status: InstanceStatus,
    ) -> Result<(), ExecutionError> {
        DispatcherSchema::new(snapshot)
            .get_instance(spec.id)
            .unwrap();
        let core_schema = CoreSchema::new(snapshot);
        let height = core_schema.next_height();

        self.events
            .lock()
            .unwrap()
            .push(RuntimeEvent::CommitService(height, spec.to_owned(), status));
        self.inner.update_service_status(snapshot, spec, status)
    }

    fn execute(
        &self,
        context: ExecutionContext<'_>,
        call_info: &CallInfo,
        arguments: &[u8],
    ) -> Result<(), ExecutionError> {
        self.inner.execute(context, call_info, arguments)
    }

    fn before_transactions(
        &self,
        context: ExecutionContext<'_>,
        instance_id: u32,
    ) -> Result<(), ExecutionError> {
        let height = CoreSchema::new(&*context.fork).next_height();
        self.events
            .lock()
            .unwrap()
            .push(RuntimeEvent::BeforeTransactions(height, instance_id));
        self.inner.after_transactions(context, instance_id)
    }

    fn after_transactions(
        &self,
        context: ExecutionContext<'_>,
        instance_id: u32,
    ) -> Result<(), ExecutionError> {
        let schema = CoreSchema::new(&*context.fork);
        let height = schema.next_height();
        self.events
            .lock()
            .unwrap()
            .push(RuntimeEvent::AfterTransactions(height, instance_id));
        self.inner.after_transactions(context, instance_id)
    }

    fn after_commit(&mut self, snapshot: &dyn Snapshot, mailbox: &mut Mailbox) {
        let height = CoreSchema::new(snapshot).next_height();
        self.events
            .lock()
            .unwrap()
            .push(RuntimeEvent::AfterCommit(height));
        self.inner.after_commit(snapshot, mailbox);
    }

    fn shutdown(&mut self) {
        self.events.lock().unwrap().push(RuntimeEvent::Shutdown);
        self.inner.shutdown();
    }
}

impl WellKnownRuntime for Inspected<RustRuntime> {
    const ID: u32 = RustRuntime::ID;
}

#[derive(Debug, Clone, ProtobufConvert, BinaryValue)]
#[protobuf_convert(source = "TestServiceInit")]
pub struct Init {
    msg: String,
}

impl Default for Init {
    fn default() -> Self {
        Self {
            msg: "constructor_message".to_owned(),
        }
    }
}

#[exonum_interface(crate = "crate")]
trait Test<Ctx> {
    type Output;
    fn method_a(&self, ctx: Ctx, arg: u64) -> Self::Output;
    fn method_b(&self, ctx: Ctx, arg: u64) -> Self::Output;
}

#[derive(Debug, ServiceFactory, ServiceDispatcher)]
#[service_dispatcher(crate = "crate", implements("Test"))]
#[service_factory(
    crate = "crate",
    artifact_name = "test_service",
    artifact_version = "0.1.0",
    proto_sources = "crate::proto::schema"
)]
pub struct TestServiceImpl;

impl Test<CallContext<'_>> for TestServiceImpl {
    type Output = Result<(), ExecutionError>;

    fn method_a(&self, mut ctx: CallContext<'_>, arg: u64) -> Result<(), ExecutionError> {
        ctx.service_data()
            .get_proof_entry("method_a_entry")
            .set(arg);
        // Test calling one service from another.
        ctx.method_b(SERVICE_INSTANCE_ID, arg)
    }

    fn method_b(&self, ctx: CallContext<'_>, arg: u64) -> Result<(), ExecutionError> {
        ctx.service_data()
            .get_proof_entry("method_b_entry")
            .set(arg);
        Ok(())
    }
}

impl TestServiceImpl {
    fn genesis_config() -> GenesisConfig {
        let artifact = TestServiceImpl.artifact_id();
        let config = generate_testnet_config(1, 0)[0].clone();
        GenesisConfigBuilder::with_consensus_config(config.consensus)
            .with_artifact(artifact)
            .with_instance(TestServiceImpl.default_instance())
            .build()
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
    const INSTANCE_ID: u32 = SERVICE_INSTANCE_ID;
    const INSTANCE_NAME: &'static str = SERVICE_INSTANCE_NAME;

    fn default_instance(&self) -> InstanceInitParams {
        self.artifact_id()
            .into_default_instance(Self::INSTANCE_ID, Self::INSTANCE_NAME)
            .with_constructor(Init::default())
    }
}

#[derive(Debug, ServiceFactory, ServiceDispatcher)]
#[service_dispatcher(crate = "crate", implements("Test"))]
#[service_factory(
    crate = "crate",
    artifact_name = "test_service",
    artifact_version = "0.2.0",
    proto_sources = "crate::proto::schema"
)]
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

/// In this test, we manually instruct the dispatcher to deploy artifacts / create / stop services
/// instead of using transactions. We still need to create patches using a `BlockchainMut`
/// in order to properly emulate the blockchain workflow.
#[test]
fn basic_rust_runtime() {
    // Create a runtime and a service artifact.
    let (runtime, event_handle) = create_runtime();
    let artifact = TestServiceImpl.artifact_id();
    // Create dummy dispatcher.
    let config = generate_testnet_config(1, 0)[0].clone();
    let genesis_config = GenesisConfigBuilder::with_consensus_config(config.consensus).build();
    let mut blockchain = Blockchain::build_for_tests()
        .into_mut(genesis_config)
        .with_runtime(runtime)
        .build()
        .unwrap();

    // The dispatcher should initialize the runtime and call `after_commit` for
    // the genesis block.
    let events = mem::replace(&mut *event_handle.lock().unwrap(), vec![]);
    assert_eq!(
        events,
        vec![
            RuntimeEvent::Initialize,
            RuntimeEvent::AfterCommit(Height(1))
        ]
    );

    // Deploy service artifact.
    let fork = create_block(&blockchain);
    Dispatcher::commit_artifact(&fork, artifact.clone(), vec![]).unwrap();
    commit_block(&mut blockchain, fork);
    let events = mem::replace(&mut *event_handle.lock().unwrap(), vec![]);
    assert_eq!(
        events,
        vec![
            RuntimeEvent::DeployArtifact(artifact.clone(), vec![]),
            RuntimeEvent::AfterCommit(Height(2)),
        ]
    );

    // Add a service.
    let spec = InstanceSpec {
        artifact,
        id: SERVICE_INSTANCE_ID,
        name: SERVICE_INSTANCE_NAME.to_owned(),
    };
    let constructor = Init::default();

    let mut fork = create_block(&blockchain);
    ExecutionContext::new(blockchain.dispatcher(), &mut fork, Caller::Blockchain)
        .initiate_adding_service(spec.clone(), constructor.clone())
        .unwrap();

    {
        let idx_name = format!("{}.constructor_entry", SERVICE_INSTANCE_NAME);
        let entry = fork.get_proof_entry(idx_name.as_str());
        assert_eq!(entry.get(), Some(Init::default().msg));
    }
    commit_block(&mut blockchain, fork);
    let events = mem::replace(&mut *event_handle.lock().unwrap(), vec![]);
    // The service is not active at the beginning of the block, so `after_transactions`
    // and `before_transactions` should not be called for it.
    assert_eq!(
        events,
        vec![
            RuntimeEvent::StartAdding(spec.clone(), constructor.into_bytes()),
            RuntimeEvent::CommitService(Height(3), spec.clone(), InstanceStatus::Active),
            RuntimeEvent::AfterCommit(Height(3)),
        ]
    );

    // Execute transaction method A.
    const ARG_A_VALUE: u64 = 11;
    let call_info = CallInfo {
        instance_id: SERVICE_INSTANCE_ID,
        method_id: 0,
    };
    let payload = ARG_A_VALUE.into_bytes();
    let caller = Caller::Service {
        instance_id: SERVICE_INSTANCE_ID,
    };
    let mut fork = create_block(&blockchain);
    blockchain
        .dispatcher()
        .call(&mut fork, caller, &call_info, &payload)
        .unwrap();

    {
        let idx_name = format!("{}.method_a_entry", SERVICE_INSTANCE_NAME);
        let entry = fork.get_proof_entry(idx_name.as_str());
        assert_eq!(entry.get(), Some(ARG_A_VALUE));
        let idx_name = format!("{}.method_b_entry", SERVICE_INSTANCE_NAME);
        let entry = fork.get_proof_entry(idx_name.as_str());
        assert_eq!(entry.get(), Some(ARG_A_VALUE));
    }
    commit_block(&mut blockchain, fork);
    let events = mem::replace(&mut *event_handle.lock().unwrap(), vec![]);
    assert_eq!(
        events,
        vec![
            RuntimeEvent::BeforeTransactions(Height(3), SERVICE_INSTANCE_ID),
            RuntimeEvent::AfterTransactions(Height(3), SERVICE_INSTANCE_ID),
            RuntimeEvent::AfterCommit(Height(4)),
        ]
    );

    // Execute transaction method B.
    const ARG_B_VALUE: u64 = 22;
    let call_info = CallInfo {
        instance_id: SERVICE_INSTANCE_ID,
        method_id: 1,
    };
    let payload = ARG_B_VALUE.into_bytes();
    let caller = Caller::Service {
        instance_id: SERVICE_INSTANCE_ID,
    };
    let mut fork = create_block(&blockchain);
    blockchain
        .dispatcher()
        .call(&mut fork, caller, &call_info, &payload)
        .unwrap();

    {
        let idx_name = format!("{}.method_b_entry", SERVICE_INSTANCE_NAME);
        let entry = fork.get_proof_entry(idx_name.as_str());
        assert_eq!(entry.get(), Some(ARG_B_VALUE));
    }
    commit_block(&mut blockchain, fork);
    let events = mem::replace(&mut *event_handle.lock().unwrap(), vec![]);
    assert_eq!(
        events,
        vec![
            RuntimeEvent::BeforeTransactions(Height(4), SERVICE_INSTANCE_ID),
            RuntimeEvent::AfterTransactions(Height(4), SERVICE_INSTANCE_ID),
            RuntimeEvent::AfterCommit(Height(5)),
        ]
    );

    // Stop service instance.
    let fork = create_block(&blockchain);
    Dispatcher::initiate_stopping_service(&fork, SERVICE_INSTANCE_ID).unwrap();
    commit_block(&mut blockchain, fork);
    let events = mem::replace(&mut *event_handle.lock().unwrap(), vec![]);

    assert_eq!(
        events,
        vec![
            RuntimeEvent::BeforeTransactions(Height(5), SERVICE_INSTANCE_ID),
            RuntimeEvent::AfterTransactions(Height(5), SERVICE_INSTANCE_ID),
            RuntimeEvent::CommitService(Height(6), spec.clone(), InstanceStatus::Stopped),
            RuntimeEvent::AfterCommit(Height(6)),
        ]
    );

    // Execute transaction method B.
    let call_info = CallInfo {
        instance_id: SERVICE_INSTANCE_ID,
        method_id: 1,
    };
    let payload = ARG_B_VALUE.into_bytes();
    let caller = Caller::Service {
        instance_id: SERVICE_INSTANCE_ID,
    };
    let mut fork = create_block(&blockchain);
    blockchain
        .dispatcher()
        .call(&mut fork, caller, &call_info, &payload)
        .expect_err("incorrect transaction");
}

#[test]
fn rust_runtime_with_builtin_services() {
    let (runtime, event_handle) = create_runtime();
    let genesis_config = TestServiceImpl::genesis_config();
    let mut blockchain = Blockchain::build_for_tests()
        .into_mut(genesis_config.clone())
        .with_runtime(runtime)
        .build()
        .expect("Can't create a blockchain instance");

    let events = mem::replace(&mut *event_handle.lock().unwrap(), vec![]);
    let artifact = TestServiceImpl.artifact_id();
    let instance_spec = TestServiceImpl.default_instance().instance_spec;
    assert_eq!(
        events,
        vec![
            RuntimeEvent::Initialize,
            RuntimeEvent::DeployArtifact(artifact.clone(), vec![]),
            RuntimeEvent::StartAdding(instance_spec.clone(), Init::default().into_bytes()),
            RuntimeEvent::CommitService(Height(0), instance_spec.clone(), InstanceStatus::Active),
            RuntimeEvent::AfterTransactions(Height(0), SERVICE_INSTANCE_ID),
            RuntimeEvent::AfterCommit(Height(1)),
        ]
    );

    let fork = create_block(&blockchain);
    commit_block(&mut blockchain, fork);
    let events = mem::replace(&mut *event_handle.lock().unwrap(), vec![]);
    assert_eq!(
        events,
        vec![
            RuntimeEvent::BeforeTransactions(Height(1), SERVICE_INSTANCE_ID),
            RuntimeEvent::AfterTransactions(Height(1), SERVICE_INSTANCE_ID),
            RuntimeEvent::AfterCommit(Height(2)),
        ]
    );

    // Emulate node restart.
    let blockchain = blockchain.inner().to_owned();
    let (runtime, event_handle) = create_runtime();
    let mut blockchain = blockchain
        .into_mut(genesis_config)
        .with_runtime(runtime)
        .build()
        .unwrap();

    let events = mem::replace(
        &mut *event_handle
            .lock()
            .expect("Can't obtain lock on a event handle"),
        vec![],
    );
    assert_eq!(
        events,
        vec![
            RuntimeEvent::Initialize,
            RuntimeEvent::DeployArtifact(artifact, vec![]),
            // `Runtime::start_adding_service` is never called for the same service
            RuntimeEvent::CommitService(Height(2), instance_spec, InstanceStatus::Active),
            // `Runtime::after_commit` is never called for the same block
            RuntimeEvent::Resume,
        ]
    );

    let fork = create_block(&blockchain);
    commit_block(&mut blockchain, fork);
    let events = mem::replace(&mut *event_handle.lock().unwrap(), vec![]);
    assert_eq!(
        events,
        vec![
            RuntimeEvent::BeforeTransactions(Height(2), SERVICE_INSTANCE_ID),
            RuntimeEvent::AfterTransactions(Height(2), SERVICE_INSTANCE_ID),
            RuntimeEvent::AfterCommit(Height(3)),
        ]
    );
}

#[test]
fn state_aggregation() {
    let runtime = RustRuntime::new(mpsc::channel(1).0).with_factory(TestServiceImpl);
    let genesis_config = TestServiceImpl::genesis_config();
    let blockchain = Blockchain::build_for_tests()
        .into_mut(genesis_config)
        .with_runtime(runtime)
        .build()
        .unwrap();

    // The constructor entry has been written to; `method_*` `ProofEntry`s are empty.
    let snapshot = blockchain.snapshot();
    let expected_indexes = vec![
        "core.consensus_config",
        "dispatcher_artifacts",
        "dispatcher_instances",
        "test_service_name.constructor_entry",
    ];
    let actual_indexes: Vec<_> = SystemSchema::new(&snapshot)
        .state_aggregator()
        .keys()
        .collect();
    assert_eq!(actual_indexes, expected_indexes);
}

#[test]
fn multiple_service_versions() {
    const NEW_INSTANCE_ID: InstanceId = SERVICE_INSTANCE_ID + 1;

    let runtime = RustRuntime::new(mpsc::channel(1).0)
        .with_factory(TestServiceImpl)
        .with_factory(TestServiceImplV2);
    let config = generate_testnet_config(1, 0)[0].clone();
    let genesis_config = GenesisConfigBuilder::with_consensus_config(config.consensus).build();
    let mut blockchain = Blockchain::build_for_tests()
        .into_mut(genesis_config)
        .with_runtime(runtime)
        .build()
        .unwrap();

    let fork = create_block(&blockchain);
    let artifact = TestServiceImpl.artifact_id();
    Dispatcher::commit_artifact(&fork, artifact.clone(), vec![]).unwrap();
    commit_block(&mut blockchain, fork);

    let mut fork = create_block(&blockchain);
    let spec = InstanceSpec {
        artifact: artifact.clone(),
        id: SERVICE_INSTANCE_ID,
        name: SERVICE_INSTANCE_NAME.to_owned(),
    };
    let constructor = Init { msg: String::new() };
    ExecutionContext::new(blockchain.dispatcher(), &mut fork, Caller::Blockchain)
        .initiate_adding_service(spec, constructor)
        .unwrap();

    commit_block(&mut blockchain, fork);
    let snapshot = blockchain.snapshot();
    let schema = DispatcherSchema::new(&snapshot);
    assert!(schema.get_artifact(&artifact).is_some());
    assert!(blockchain.dispatcher().is_artifact_deployed(&artifact));

    // Add a newer artifact version.
    let fork = blockchain.fork();
    let new_artifact = TestServiceImplV2.artifact_id();
    assert_ne!(new_artifact, artifact);
    assert!(schema.get_artifact(&new_artifact).is_none());
    Dispatcher::commit_artifact(&fork, new_artifact.clone(), vec![]).unwrap();
    commit_block(&mut blockchain, fork);

    // ...and a service based on the new artifact.
    let mut fork = blockchain.fork();
    let spec = InstanceSpec {
        artifact: new_artifact.clone(),
        id: NEW_INSTANCE_ID,
        name: "new_service".to_owned(),
    };
    ExecutionContext::new(blockchain.dispatcher(), &mut fork, Caller::Blockchain)
        .initiate_adding_service(spec, ())
        .unwrap();

    // Check that both artifact versions are present in the dispatcher schema.
    commit_block(&mut blockchain, fork);
    let snapshot = blockchain.snapshot();
    let schema = DispatcherSchema::new(&snapshot);
    assert!(schema.get_artifact(&artifact).is_some());
    assert!(blockchain.dispatcher().is_artifact_deployed(&artifact));
    assert!(schema.get_artifact(&new_artifact).is_some());
    assert!(blockchain.dispatcher().is_artifact_deployed(&new_artifact));
    assert!(schema.get_instance(SERVICE_INSTANCE_ID).is_some());
    assert!(schema.get_instance("new_service").is_some());

    // Check that both services are active by calling transactions for them.
    let mut call_info = CallInfo {
        instance_id: SERVICE_INSTANCE_ID,
        method_id: 0,
    };
    let payload = 11_u64.into_bytes();
    let caller = Caller::Service {
        instance_id: SERVICE_INSTANCE_ID,
    };

    let mut fork = create_block(&blockchain);
    blockchain
        .dispatcher()
        .call(&mut fork, caller, &call_info, &payload)
        .unwrap();
    call_info.instance_id = NEW_INSTANCE_ID;
    let err = blockchain
        .dispatcher()
        .call(&mut fork, caller, &call_info, &payload)
        .unwrap_err();
    // `method_a` is removed from the newer service version.
    assert_eq!(err, ErrorMatch::from_fail(&DispatcherError::NoSuchMethod));

    {
        let idx_name = format!("{}.method_a_entry", SERVICE_INSTANCE_NAME);
        let entry = fork.get_proof_entry(idx_name.as_str());
        assert_eq!(entry.get(), Some(11));
        let entry = fork.get_proof_entry::<_, u64>("new_service.method_a_entry");
        assert!(!entry.exists());
    }

    call_info.method_id = 1;
    call_info.instance_id = SERVICE_INSTANCE_ID;
    let payload = 12_u64.into_bytes();
    blockchain
        .dispatcher()
        .call(&mut fork, caller, &call_info, &payload)
        .unwrap();
    call_info.instance_id = NEW_INSTANCE_ID;
    blockchain
        .dispatcher()
        .call(&mut fork, caller, &call_info, &payload)
        .unwrap();

    {
        let idx_name = format!("{}.method_b_entry", SERVICE_INSTANCE_NAME);
        let entry = fork.get_proof_entry(idx_name.as_str());
        assert_eq!(entry.get(), Some(12));
        let entry = fork.get_proof_entry("new_service.method_b_entry");
        assert_eq!(entry.get(), Some(54)); // 12 + 42
    }
}

#[test]
fn conflicting_service_instances() {
    let (runtime, event_handle) = create_runtime();
    let artifact = TestServiceImpl.artifact_id();
    let config = generate_testnet_config(1, 0)[0].clone();
    let genesis_config = GenesisConfigBuilder::with_consensus_config(config.consensus).build();
    let mut blockchain = Blockchain::build_for_tests()
        .into_mut(genesis_config)
        .with_runtime(runtime)
        .build()
        .unwrap();

    let fork = create_block(&blockchain);
    Dispatcher::commit_artifact(&fork, artifact.clone(), vec![]).unwrap();
    commit_block(&mut blockchain, fork);
    event_handle.lock().unwrap().clear();

    // Fork #1.
    let spec = InstanceSpec {
        artifact: artifact.clone(),
        id: SERVICE_INSTANCE_ID,
        name: SERVICE_INSTANCE_NAME.to_owned(),
    };
    let constructor = Init::default();
    let mut fork = create_block(&blockchain);
    ExecutionContext::new(blockchain.dispatcher(), &mut fork, Caller::Blockchain)
        .initiate_adding_service(spec.clone(), constructor.clone())
        .unwrap();

    // Fork #2: same service, but with different ID.
    let alternative_spec = InstanceSpec {
        id: SERVICE_INSTANCE_ID + 1, // << alternative numeric ID
        ..spec.clone()
    };
    let mut alternative_fork = create_block(&blockchain);
    ExecutionContext::new(
        blockchain.dispatcher(),
        &mut alternative_fork,
        Caller::Blockchain,
    )
    .initiate_adding_service(alternative_spec.clone(), constructor.clone())
    .unwrap();

    commit_block(&mut blockchain, fork);
    let events = mem::replace(&mut *event_handle.lock().unwrap(), vec![]);
    assert_eq!(
        events,
        vec![
            RuntimeEvent::StartAdding(spec.clone(), constructor.clone().into_bytes()),
            RuntimeEvent::StartAdding(alternative_spec, constructor.into_bytes()),
            RuntimeEvent::CommitService(Height(3), spec, InstanceStatus::Active),
            RuntimeEvent::AfterCommit(Height(3)),
        ]
    );

    // Check that the added service is accessible only by its proper ID.
    let mut call_info = CallInfo {
        instance_id: SERVICE_INSTANCE_ID,
        method_id: 0,
    };
    let payload = 10_u64.into_bytes();
    let caller = Caller::Transaction {
        hash: Hash::zero(),
        author: PublicKey::new([0; PUBLIC_KEY_LENGTH]),
    };
    let mut fork = create_block(&blockchain);
    blockchain
        .dispatcher()
        .call(&mut fork, caller, &call_info, &payload)
        .unwrap();

    call_info.instance_id += 1;
    blockchain
        .dispatcher()
        .call(&mut fork, caller, &call_info, &payload)
        .unwrap_err();
}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_dispatcher(crate = "crate", implements())]
#[service_factory(
    crate = "crate",
    artifact_name = "dependent_service",
    artifact_version = "0.1.0",
    proto_sources = "crate::proto::schema"
)]
pub struct DependentServiceImpl;

impl Service for DependentServiceImpl {
    fn initialize(&self, context: CallContext<'_>, params: Vec<u8>) -> Result<(), ExecutionError> {
        assert_eq!(*context.caller(), Caller::Blockchain);
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
    const INSTANCE_ID: u32 = SERVICE_INSTANCE_ID + 1;
    const INSTANCE_NAME: &'static str = "dependent-service";

    fn default_instance(&self) -> InstanceInitParams {
        self.artifact_id()
            .into_default_instance(Self::INSTANCE_ID, Self::INSTANCE_NAME)
            .with_constructor(Init {
                msg: SERVICE_INSTANCE_NAME.to_owned(),
            })
    }
}

#[test]
fn dependent_builtin_service() {
    let main_service = TestServiceImpl;
    let dep_service = DependentServiceImpl;

    // Create a blockchain with both main and dependent services initialized in the genesis block.
    let config = generate_testnet_config(1, 0)[0].clone();

    let genesis_config = GenesisConfigBuilder::with_consensus_config(config.consensus)
        .with_artifact(main_service.artifact_id())
        .with_instance(main_service.default_instance())
        .with_artifact(dep_service.artifact_id())
        .with_instance(dep_service.default_instance())
        .build();

    let runtime = RustRuntime::new(mpsc::channel(1).0)
        .with_factory(main_service)
        .with_factory(dep_service);

    let blockchain = Blockchain::build_for_tests()
        .into_mut(genesis_config)
        .with_runtime(runtime)
        .build()
        .unwrap();

    let snapshot = blockchain.snapshot();
    let schema = DispatcherSchema::new(&snapshot);
    assert_eq!(
        schema
            .get_instance(SERVICE_INSTANCE_ID)
            .unwrap()
            .status
            .unwrap(),
        InstanceStatus::Active
    );
    assert_eq!(
        schema
            .get_instance("dependent-service")
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

    let config = generate_testnet_config(1, 0)[0].clone();

    // Error in the service instantiation in the genesis block bubbles up.
    let genesis_config = GenesisConfigBuilder::with_consensus_config(config.consensus)
        .with_artifact(main_service.artifact_id())
        .with_artifact(dep_service.artifact_id())
        .with_instance(dep_service.default_instance()) // <-- Incorrect service ordering
        .with_instance(main_service.default_instance())
        .build();

    let runtime = RustRuntime::new(mpsc::channel(1).0)
        .with_factory(main_service)
        .with_factory(dep_service);

    let err = Blockchain::build_for_tests()
        .into_mut(genesis_config)
        .with_runtime(runtime)
        .build()
        .unwrap_err();
    assert!(err.to_string().contains("no dependency"));
}

#[test]
fn dependent_service_with_no_dependency() {
    let runtime = RustRuntime::new(mpsc::channel(1).0)
        .with_factory(TestServiceImpl)
        .with_factory(DependentServiceImpl);

    let config = generate_testnet_config(1, 0)[0].clone();
    let genesis_config = GenesisConfigBuilder::with_consensus_config(config.consensus).build();

    let mut blockchain = Blockchain::build_for_tests()
        .into_mut(genesis_config)
        .with_runtime(runtime)
        .build()
        .unwrap();

    let fork = create_block(&blockchain);
    let inst = DependentServiceImpl.default_instance();
    Dispatcher::commit_artifact(&fork, inst.instance_spec.artifact.clone(), vec![]).unwrap();
    commit_block(&mut blockchain, fork);

    let mut fork = create_block(&blockchain);
    let mut ctx = ExecutionContext::new(blockchain.dispatcher(), &mut fork, Caller::Blockchain);
    let err = ctx
        .initiate_adding_service(inst.instance_spec, inst.constructor)
        .unwrap_err();
    assert!(err.to_string().contains("no dependency"));

    // Check that the information about the service hasn't persisted in the dispatcher schema.
    commit_block(&mut blockchain, fork);
    let snapshot = blockchain.snapshot();
    assert!(DispatcherSchema::new(&snapshot)
        .get_instance("dependent-service")
        .is_none());
}

#[test]
fn dependent_service_in_same_block() {
    let runtime = RustRuntime::new(mpsc::channel(1).0)
        .with_factory(TestServiceImpl)
        .with_factory(DependentServiceImpl);

    let config = generate_testnet_config(1, 0)[0].clone();
    let genesis_config = GenesisConfigBuilder::with_consensus_config(config.consensus).build();

    let mut blockchain = Blockchain::build_for_tests()
        .into_mut(genesis_config)
        .with_runtime(runtime)
        .build()
        .unwrap();

    // Artifacts need to be deployed in a separate block due to checks in `RustRuntime`.
    let fork = create_block(&blockchain);
    let main_inst = TestServiceImpl.default_instance();
    let dep_inst = DependentServiceImpl.default_instance();
    Dispatcher::commit_artifact(&fork, main_inst.instance_spec.artifact.clone(), vec![]).unwrap();
    Dispatcher::commit_artifact(&fork, dep_inst.instance_spec.artifact.clone(), vec![]).unwrap();
    commit_block(&mut blockchain, fork);

    // Deploy both services in the same block after genesis.
    let mut fork = create_block(&blockchain);
    let mut ctx = ExecutionContext::new(blockchain.dispatcher(), &mut fork, Caller::Blockchain);
    ctx.initiate_adding_service(main_inst.instance_spec, main_inst.constructor)
        .unwrap();
    ctx.initiate_adding_service(dep_inst.instance_spec, dep_inst.constructor)
        .unwrap();
    commit_block(&mut blockchain, fork);

    let snapshot = blockchain.snapshot();
    let schema = DispatcherSchema::new(&snapshot);
    assert_eq!(
        schema
            .get_instance("dependent-service")
            .unwrap()
            .status
            .unwrap(),
        InstanceStatus::Active
    );
}

#[test]
fn dependent_service_in_successive_block() {
    let main_service = TestServiceImpl;
    let dep_service = DependentServiceImpl;
    let genesis_config = TestServiceImpl::genesis_config();

    let runtime = RustRuntime::new(mpsc::channel(1).0)
        .with_factory(main_service)
        .with_factory(dep_service);

    let mut blockchain = Blockchain::build_for_tests()
        .into_mut(genesis_config)
        .with_runtime(runtime)
        .build()
        .unwrap();

    let fork = create_block(&blockchain);
    let dep_spec = DependentServiceImpl.default_instance();
    Dispatcher::commit_artifact(&fork, dep_spec.instance_spec.artifact.clone(), vec![]).unwrap();
    commit_block(&mut blockchain, fork);

    let mut fork = create_block(&blockchain);
    ExecutionContext::new(blockchain.dispatcher(), &mut fork, Caller::Blockchain)
        .initiate_adding_service(dep_spec.instance_spec, dep_spec.constructor)
        .unwrap();
    commit_block(&mut blockchain, fork);

    let snapshot = blockchain.snapshot();
    let schema = DispatcherSchema::new(&snapshot);
    assert_eq!(
        schema
            .get_instance("dependent-service")
            .unwrap()
            .status
            .unwrap(),
        InstanceStatus::Active
    );
}
