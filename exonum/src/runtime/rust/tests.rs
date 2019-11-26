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
use exonum_derive::exonum_interface;
use exonum_merkledb::{access::AccessExt, BinaryValue, Fork, Snapshot};
use exonum_proto::ProtobufConvert;
use futures::{sync::mpsc, Future};

use std::{
    collections::BTreeMap,
    mem,
    sync::{Arc, Mutex},
};

use crate::{
    blockchain::{Blockchain, BlockchainMut, InstanceConfig, Schema as CoreSchema},
    helpers::{generate_testnet_config, Height, ValidatorId},
    proto::schema::tests::{TestServiceInit, TestServiceTx},
    runtime::{
        error::{ErrorKind, ExecutionError},
        BlockchainData, CallInfo, Caller, DeployStatus, Dispatcher, DispatcherError,
        DispatcherSchema, ExecutionContext, InstanceId, InstanceSpec, Mailbox, Runtime,
        StateHashAggregator,
    },
};

use super::{
    service::{Service, ServiceFactory},
    ArtifactId, CallContext, RustRuntime,
};

const SERVICE_INSTANCE_ID: InstanceId = 2;
const SERVICE_INSTANCE_NAME: &str = "test_service_name";

fn create_block(blockchain: &BlockchainMut) -> Fork {
    let height = CoreSchema::new(&blockchain.snapshot()).height();
    let (_, patch) =
        blockchain.create_patch(ValidatorId(0), height.next(), &[], &mut BTreeMap::new());
    Fork::from(patch)
}

fn commit_block(blockchain: &mut BlockchainMut, fork: Fork) {
    blockchain
        .commit(
            fork.into_patch(),
            Hash::zero(),
            vec![],
            &mut BTreeMap::new(),
        )
        .unwrap();
}

fn create_runtime() -> (Inspected<RustRuntime>, Arc<Mutex<Vec<RuntimeEvent>>>) {
    let mut runtime = RustRuntime::new(mpsc::channel(1).0);
    let service_factory = Box::new(TestServiceImpl);
    runtime.add_service_factory(service_factory);
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
    CommitService(Option<Height>, InstanceSpec),
    BeforeCommit(Height, InstanceId),
    AfterCommit(Height),
    Shutdown,
}

/// Test runtime wrapper logging all the events (as `RuntimeEvent`) happening within it.
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

    fn start_adding_service(
        &self,
        context: ExecutionContext<'_>,
        spec: &InstanceSpec,
        parameters: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        self.events.lock().unwrap().push(RuntimeEvent::StartAdding(
            spec.to_owned(),
            parameters.clone(),
        ));
        self.inner.start_adding_service(context, spec, parameters)
    }

    fn commit_service(
        &mut self,
        snapshot: &dyn Snapshot,
        spec: &InstanceSpec,
    ) -> Result<(), ExecutionError> {
        DispatcherSchema::new(snapshot)
            .get_instance(spec.id)
            .unwrap();
        let core_schema = CoreSchema::new(snapshot);
        let height = if core_schema.block_hashes_by_height().is_empty() {
            None
        } else {
            Some(core_schema.height())
        };

        self.events
            .lock()
            .unwrap()
            .push(RuntimeEvent::CommitService(height, spec.to_owned()));
        self.inner.commit_service(snapshot, spec)
    }

    fn execute(
        &self,
        context: ExecutionContext<'_>,
        call_info: &CallInfo,
        arguments: &[u8],
    ) -> Result<(), ExecutionError> {
        self.inner.execute(context, call_info, arguments)
    }

    fn state_hashes(&self, snapshot: &dyn Snapshot) -> StateHashAggregator {
        self.inner.state_hashes(snapshot)
    }

    fn before_transactions(
        &self,
        context: ExecutionContext<'_>,
        instance_id: u32,
    ) -> Result<(), ExecutionError> {
        let height = CoreSchema::new(&*context.fork).height();
        self.events
            .lock()
            .unwrap()
            .push(RuntimeEvent::BeforeTransactions(height, instance_id));
        self.inner.before_commit(context, instance_id)
    }

    fn before_commit(
        &self,
        context: ExecutionContext<'_>,
        instance_id: u32,
    ) -> Result<(), ExecutionError> {
        let height = CoreSchema::new(&*context.fork).height();
        self.events
            .lock()
            .unwrap()
            .push(RuntimeEvent::BeforeCommit(height, instance_id));
        self.inner.before_commit(context, instance_id)
    }

    fn after_commit(&mut self, snapshot: &dyn Snapshot, mailbox: &mut Mailbox) {
        let height = CoreSchema::new(snapshot).height();
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

impl Into<(u32, Box<dyn Runtime>)> for Inspected<RustRuntime> {
    fn into(self) -> (u32, Box<dyn Runtime>) {
        (RustRuntime::ID as u32, Box::new(self))
    }
}

#[derive(Debug, Clone, ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "TestServiceInit")]
pub struct Init {
    msg: String,
}

#[derive(Debug, ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "TestServiceTx")]
struct TxA {
    value: u64,
}

#[derive(Debug, ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "TestServiceTx")]
struct TxB {
    value: u64,
}

#[exonum_interface(crate = "crate")]
trait TestService {
    fn method_a(&self, context: CallContext<'_>, arg: TxA) -> Result<(), ExecutionError>;
    fn method_b(&self, context: CallContext<'_>, arg: TxB) -> Result<(), ExecutionError>;
}

#[derive(Debug, ServiceFactory, ServiceDispatcher)]
#[service_dispatcher(crate = "crate", implements("TestService"))]
#[service_factory(
    crate = "crate",
    artifact_name = "test_service",
    artifact_version = "0.1.0",
    proto_sources = "crate::proto::schema"
)]
pub struct TestServiceImpl;

#[derive(Debug)]
struct TestServiceClient<'a>(CallContext<'a>);

impl<'a> From<CallContext<'a>> for TestServiceClient<'a> {
    fn from(context: CallContext<'a>) -> Self {
        Self(context)
    }
}

impl<'a> TestServiceClient<'a> {
    fn method_b(&mut self, arg: TxB) -> Result<(), ExecutionError> {
        self.0.call("", 1, arg)
    }
}

impl TestService for TestServiceImpl {
    fn method_a(&self, mut context: CallContext<'_>, arg: TxA) -> Result<(), ExecutionError> {
        context
            .service_data()
            .get_entry("method_a_entry")
            .set(arg.value);
        // Test calling one service from another.
        context
            .interface::<TestServiceClient<'_>>(SERVICE_INSTANCE_ID)?
            .method_b(TxB { value: arg.value })
            .expect("Failed to dispatch call");
        Ok(())
    }

    fn method_b(&self, context: CallContext<'_>, arg: TxB) -> Result<(), ExecutionError> {
        context
            .service_data()
            .get_entry("method_b_entry")
            .set(arg.value);
        Ok(())
    }
}

impl Service for TestServiceImpl {
    fn initialize(&self, context: CallContext<'_>, params: Vec<u8>) -> Result<(), ExecutionError> {
        let init = Init::from_bytes(params.into()).map_err(DispatcherError::malformed_arguments)?;
        context
            .service_data()
            .get_entry("constructor_entry")
            .set(init.msg);
        Ok(())
    }

    fn state_hash(&self, _data: BlockchainData<&dyn Snapshot>) -> Vec<Hash> {
        vec![]
    }
}

/// In this test, we manually instruct the dispatcher to deploy artifacts / create services
/// instead of using transactions. We still need to create patches using a `BlockchainMut`
/// in order to properly emulate the blockchain workflow.
#[test]
fn basic_rust_runtime() {
    // Create a runtime and a service artifact.
    let (runtime, event_handle) = create_runtime();
    let artifact: ArtifactId = TestServiceImpl.artifact_id().into();
    // Create dummy dispatcher.
    let config = generate_testnet_config(1, 0)[0].clone();
    let mut blockchain = Blockchain::build_for_tests()
        .into_mut(config.consensus)
        .with_additional_runtime(runtime)
        .build()
        .unwrap();

    // The dispatcher should initialize the runtime and call `after_commit` for
    // the genesis block.
    let events = mem::replace(&mut *event_handle.lock().unwrap(), vec![]);
    assert_eq!(
        events,
        vec![
            RuntimeEvent::Initialize,
            RuntimeEvent::AfterCommit(Height(0))
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
            RuntimeEvent::AfterCommit(Height(1)),
        ]
    );

    // Add a service.
    let spec = InstanceSpec {
        artifact,
        id: SERVICE_INSTANCE_ID,
        name: SERVICE_INSTANCE_NAME.to_owned(),
    };
    let constructor = Init {
        msg: "constructor_message".to_owned(),
    };

    let mut fork = create_block(&blockchain);
    ExecutionContext::new(blockchain.dispatcher(), &mut fork, Caller::Blockchain)
        .start_adding_service(spec.clone(), constructor.clone())
        .unwrap();

    {
        let idx_name = format!("{}.constructor_entry", SERVICE_INSTANCE_NAME);
        let entry = fork.get_entry(idx_name.as_str());
        assert_eq!(entry.get(), Some("constructor_message".to_owned()));
    }
    commit_block(&mut blockchain, fork);
    let events = mem::replace(&mut *event_handle.lock().unwrap(), vec![]);
    // The service is not active at the beginning of the block, so `before_commit`
    // and `before_transactions` should not be called for it.
    assert_eq!(
        events,
        vec![
            RuntimeEvent::StartAdding(spec.clone(), constructor.into_bytes()),
            RuntimeEvent::CommitService(Some(Height(2)), spec.clone()),
            RuntimeEvent::AfterCommit(Height(2)),
        ]
    );

    // Execute transaction method A.
    const ARG_A_VALUE: u64 = 11;
    let call_info = CallInfo {
        instance_id: SERVICE_INSTANCE_ID,
        method_id: 0,
    };
    let payload = TxA { value: ARG_A_VALUE }.into_bytes();
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
        let entry = fork.get_entry(idx_name.as_str());
        assert_eq!(entry.get(), Some(ARG_A_VALUE));
        let idx_name = format!("{}.method_b_entry", SERVICE_INSTANCE_NAME);
        let entry = fork.get_entry(idx_name.as_str());
        assert_eq!(entry.get(), Some(ARG_A_VALUE));
    }
    commit_block(&mut blockchain, fork);
    let events = mem::replace(&mut *event_handle.lock().unwrap(), vec![]);
    assert_eq!(
        events,
        vec![
            RuntimeEvent::BeforeTransactions(Height(2), SERVICE_INSTANCE_ID),
            RuntimeEvent::BeforeCommit(Height(2), SERVICE_INSTANCE_ID),
            RuntimeEvent::AfterCommit(Height(3)),
        ]
    );

    // Execute transaction method B.
    const ARG_B_VALUE: u64 = 22;
    let call_info = CallInfo {
        instance_id: SERVICE_INSTANCE_ID,
        method_id: 1,
    };
    let payload = TxB { value: ARG_B_VALUE }.into_bytes();
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
        let entry = fork.get_entry(idx_name.as_str());
        assert_eq!(entry.get(), Some(ARG_B_VALUE));
    }
    commit_block(&mut blockchain, fork);
    let events = mem::replace(&mut *event_handle.lock().unwrap(), vec![]);
    assert_eq!(
        events,
        vec![
            RuntimeEvent::BeforeTransactions(Height(3), SERVICE_INSTANCE_ID),
            RuntimeEvent::BeforeCommit(Height(3), SERVICE_INSTANCE_ID),
            RuntimeEvent::AfterCommit(Height(4)),
        ]
    );
}

#[test]
fn rust_runtime_with_builtin_services() {
    let (runtime, event_handle) = create_runtime();
    let artifact: ArtifactId = TestServiceImpl.artifact_id().into();
    let config = generate_testnet_config(1, 0)[0].clone();
    let spec = InstanceSpec {
        artifact: artifact.clone(),
        id: SERVICE_INSTANCE_ID,
        name: SERVICE_INSTANCE_NAME.to_owned(),
    };
    let constructor = Init {
        msg: "constructor_message".to_owned(),
    };

    let mut blockchain = Blockchain::build_for_tests()
        .into_mut(config.consensus.clone())
        .with_additional_runtime(runtime)
        .with_builtin_instances(vec![InstanceConfig::new(
            spec.clone(),
            Some(vec![]),
            constructor.clone().into_bytes(),
        )])
        .build()
        .unwrap();

    let events = mem::replace(&mut *event_handle.lock().unwrap(), vec![]);
    assert_eq!(
        events,
        vec![
            RuntimeEvent::Initialize,
            RuntimeEvent::DeployArtifact(artifact.clone(), vec![]),
            RuntimeEvent::StartAdding(spec.clone(), constructor.clone().into_bytes()),
            RuntimeEvent::CommitService(None, spec.clone()),
            RuntimeEvent::AfterCommit(Height(0)),
        ]
    );

    let fork = create_block(&blockchain);
    commit_block(&mut blockchain, fork);
    let events = mem::replace(&mut *event_handle.lock().unwrap(), vec![]);
    assert_eq!(
        events,
        vec![
            RuntimeEvent::BeforeTransactions(Height(0), SERVICE_INSTANCE_ID),
            RuntimeEvent::BeforeCommit(Height(0), SERVICE_INSTANCE_ID),
            RuntimeEvent::AfterCommit(Height(1)),
        ]
    );

    // Emulate node restart.
    let blockchain = blockchain.inner().to_owned();
    let (runtime, event_handle) = create_runtime();
    let mut blockchain = blockchain
        .into_mut(config.consensus)
        .with_additional_runtime(runtime)
        .build()
        .unwrap();

    let events = mem::replace(&mut *event_handle.lock().unwrap(), vec![]);
    assert_eq!(
        events,
        vec![
            RuntimeEvent::Initialize,
            RuntimeEvent::DeployArtifact(artifact, vec![]),
            // `Runtime::start_adding_service` is never called for the same service
            RuntimeEvent::CommitService(Some(Height(1)), spec),
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
            RuntimeEvent::BeforeTransactions(Height(1), SERVICE_INSTANCE_ID),
            RuntimeEvent::BeforeCommit(Height(1), SERVICE_INSTANCE_ID),
            RuntimeEvent::AfterCommit(Height(2)),
        ]
    );
}

#[test]
fn conflicting_service_instances() {
    let (runtime, event_handle) = create_runtime();
    let artifact: ArtifactId = TestServiceImpl.artifact_id().into();
    let config = generate_testnet_config(1, 0)[0].clone();
    let mut blockchain = Blockchain::build_for_tests()
        .into_mut(config.consensus.clone())
        .with_additional_runtime(runtime)
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
    let constructor = Init {
        msg: "constructor_message".to_owned(),
    };
    let mut fork = create_block(&blockchain);
    ExecutionContext::new(blockchain.dispatcher(), &mut fork, Caller::Blockchain)
        .start_adding_service(spec.clone(), constructor.clone())
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
    .start_adding_service(alternative_spec.clone(), constructor.clone())
    .unwrap();

    commit_block(&mut blockchain, fork);
    let events = mem::replace(&mut *event_handle.lock().unwrap(), vec![]);
    assert_eq!(
        events,
        vec![
            RuntimeEvent::StartAdding(spec.clone(), constructor.clone().into_bytes()),
            RuntimeEvent::StartAdding(alternative_spec, constructor.into_bytes()),
            RuntimeEvent::CommitService(Some(Height(2)), spec),
            RuntimeEvent::AfterCommit(Height(2)),
        ]
    );

    // Check that the added service is accessible only by its proper ID.
    let mut call_info = CallInfo {
        instance_id: SERVICE_INSTANCE_ID,
        method_id: 0,
    };
    let payload = TxA { value: 10 }.into_bytes();
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
            return Err(ExecutionError::new(ErrorKind::service(0), "no dependency"));
        }

        // Check that it is possible to access data of the dependency right away,
        // even if it is deployed in the same block.
        let dependency_data = context
            .data()
            .for_service(&*init.msg)
            .expect("Dependency exists, but its data does not");
        assert!(dependency_data
            .get_entry::<_, String>("constructor_entry")
            .exists());

        Ok(())
    }

    fn state_hash(&self, _data: BlockchainData<&dyn Snapshot>) -> Vec<Hash> {
        vec![]
    }
}

fn instance_configs() -> (InstanceConfig, InstanceConfig) {
    let main_spec = InstanceSpec {
        id: SERVICE_INSTANCE_ID,
        name: SERVICE_INSTANCE_NAME.to_owned(),
        artifact: TestServiceImpl.artifact_id().into(),
    };
    let dependent_spec = InstanceSpec {
        id: SERVICE_INSTANCE_ID + 1,
        name: "dependent-service".to_owned(),
        artifact: DependentServiceImpl.artifact_id().into(),
    };
    let dependent_constructor = Init {
        msg: SERVICE_INSTANCE_NAME.to_owned(),
    };

    (
        InstanceConfig::new(main_spec, None, vec![]),
        InstanceConfig::new(dependent_spec, None, dependent_constructor.into_bytes()),
    )
}

#[test]
fn dependent_builtin_service() {
    let mut runtime = RustRuntime::new(mpsc::channel(1).0);
    runtime.add_service_factory(Box::new(TestServiceImpl));
    runtime.add_service_factory(Box::new(DependentServiceImpl));
    let (main_service, dep_service) = instance_configs();

    // Create a blockchain with both main and dependent services initialized in the genesis block.
    let config = generate_testnet_config(1, 0)[0].clone();
    let blockchain = Blockchain::build_for_tests()
        .into_mut(config.consensus)
        .with_additional_runtime(runtime)
        .with_builtin_instances(vec![main_service, dep_service])
        .build()
        .unwrap();

    let snapshot = blockchain.snapshot();
    let schema = DispatcherSchema::new(&snapshot);
    assert_eq!(
        schema.get_instance(SERVICE_INSTANCE_ID).unwrap().1,
        DeployStatus::Active
    );
    assert_eq!(
        schema.get_instance("dependent-service").unwrap().1,
        DeployStatus::Active
    );
}

#[test]
fn dependent_builtin_service_with_incorrect_order() {
    let mut runtime = RustRuntime::new(mpsc::channel(1).0);
    runtime.add_service_factory(Box::new(TestServiceImpl));
    runtime.add_service_factory(Box::new(DependentServiceImpl));
    let (main_service, dep_service) = instance_configs();

    let config = generate_testnet_config(1, 0)[0].clone();
    // Error in the service instantiation in the genesis block bubbles up.
    let err = Blockchain::build_for_tests()
        .into_mut(config.consensus)
        .with_additional_runtime(runtime)
        .with_builtin_instances(vec![dep_service, main_service])
        // ^-- Incorrect service ordering
        .build()
        .unwrap_err();
    assert!(err.to_string().contains("no dependency"));
}

#[test]
fn dependent_service_with_no_dependency() {
    let mut runtime = RustRuntime::new(mpsc::channel(1).0);
    runtime.add_service_factory(Box::new(TestServiceImpl));
    runtime.add_service_factory(Box::new(DependentServiceImpl));
    let (_, dep_service) = instance_configs();

    let config = generate_testnet_config(1, 0)[0].clone();
    let mut blockchain = Blockchain::build_for_tests()
        .into_mut(config.consensus)
        .with_additional_runtime(runtime)
        .build()
        .unwrap();

    let fork = create_block(&blockchain);
    Dispatcher::commit_artifact(&fork, dep_service.instance_spec.artifact.clone(), vec![]).unwrap();
    commit_block(&mut blockchain, fork);

    let mut fork = create_block(&blockchain);
    let mut ctx = ExecutionContext::new(blockchain.dispatcher(), &mut fork, Caller::Blockchain);
    let err = ctx
        .start_adding_service(dep_service.instance_spec, dep_service.constructor)
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
    let mut runtime = RustRuntime::new(mpsc::channel(1).0);
    runtime.add_service_factory(Box::new(TestServiceImpl));
    runtime.add_service_factory(Box::new(DependentServiceImpl));
    let (main_service, dep_service) = instance_configs();

    let config = generate_testnet_config(1, 0)[0].clone();
    let mut blockchain = Blockchain::build_for_tests()
        .into_mut(config.consensus)
        .with_additional_runtime(runtime)
        .build()
        .unwrap();

    // Artifacts need to be deployed in a separate block due to checks in `RustRuntime`.
    let fork = create_block(&blockchain);
    Dispatcher::commit_artifact(&fork, main_service.instance_spec.artifact.clone(), vec![])
        .unwrap();
    Dispatcher::commit_artifact(&fork, dep_service.instance_spec.artifact.clone(), vec![]).unwrap();
    commit_block(&mut blockchain, fork);

    // Deploy both services in the same block after genesis.
    let mut fork = create_block(&blockchain);
    let mut ctx = ExecutionContext::new(blockchain.dispatcher(), &mut fork, Caller::Blockchain);
    ctx.start_adding_service(main_service.instance_spec, main_service.constructor)
        .unwrap();
    ctx.start_adding_service(dep_service.instance_spec, dep_service.constructor)
        .unwrap();
    commit_block(&mut blockchain, fork);

    let snapshot = blockchain.snapshot();
    let schema = DispatcherSchema::new(&snapshot);
    assert_eq!(
        schema.get_instance("dependent-service").unwrap().1,
        DeployStatus::Active
    );
}

#[test]
fn dependent_service_in_successive_block() {
    let mut runtime = RustRuntime::new(mpsc::channel(1).0);
    runtime.add_service_factory(Box::new(TestServiceImpl));
    runtime.add_service_factory(Box::new(DependentServiceImpl));
    let (main_service, dep_service) = instance_configs();

    let config = generate_testnet_config(1, 0)[0].clone();
    let mut blockchain = Blockchain::build_for_tests()
        .into_mut(config.consensus)
        .with_additional_runtime(runtime)
        .with_builtin_instances(vec![main_service])
        .build()
        .unwrap();

    let fork = create_block(&blockchain);
    Dispatcher::commit_artifact(&fork, dep_service.instance_spec.artifact.clone(), vec![]).unwrap();
    commit_block(&mut blockchain, fork);

    let mut fork = create_block(&blockchain);
    ExecutionContext::new(blockchain.dispatcher(), &mut fork, Caller::Blockchain)
        .start_adding_service(dep_service.instance_spec, dep_service.constructor)
        .unwrap();
    commit_block(&mut blockchain, fork);

    let snapshot = blockchain.snapshot();
    let schema = DispatcherSchema::new(&snapshot);
    assert_eq!(
        schema.get_instance("dependent-service").unwrap().1,
        DeployStatus::Active
    );
}
