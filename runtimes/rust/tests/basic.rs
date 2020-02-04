// Copyright 2020 The Exonum Team
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

use exonum::{
    blockchain::{
        config::{GenesisConfig, InstanceInitParams},
        Blockchain, BlockchainBuilder, BlockchainMut,
    },
    helpers::Height,
    merkledb::{access::AccessExt, BinaryValue, SystemSchema},
    runtime::{
        Caller, CommonError, CoreError, ErrorMatch, ExecutionContext, ExecutionError,
        InstanceStatus, SnapshotExt,
    },
};
use exonum_derive::{exonum_interface, BinaryValue, ServiceDispatcher, ServiceFactory};
use pretty_assertions::assert_eq;
use serde_derive::*;

use std::collections::BTreeMap;

use exonum_rust_runtime::{DefaultInstance, RustRuntimeBuilder, Service, ServiceFactory};

use self::inspected::{
    create_block_with_transactions, create_genesis_config_builder, execute_transaction,
    DeployArtifact, EventsHandle, Inspected, RuntimeEvent, StartService, StopService,
    ToySupervisor, ToySupervisorService,
};

mod inspected;

#[derive(Debug, Clone, Serialize, Deserialize, BinaryValue)]
#[binary_value(codec = "bincode")]
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

#[exonum_interface(auto_ids)]
trait Test<Ctx> {
    type Output;
    fn method_a(&self, ctx: Ctx, arg: u64) -> Self::Output;
    fn method_b(&self, ctx: Ctx, arg: u64) -> Self::Output;
}

#[derive(Debug, ServiceFactory, ServiceDispatcher)]
#[service_dispatcher(implements("Test"))]
#[service_factory(artifact_name = "test_service", artifact_version = "0.1.0")]
pub struct TestServiceImpl;

impl Test<ExecutionContext<'_>> for TestServiceImpl {
    type Output = Result<(), ExecutionError>;

    fn method_a(&self, mut ctx: ExecutionContext<'_>, arg: u64) -> Result<(), ExecutionError> {
        ctx.service_data()
            .get_proof_entry("method_a_entry")
            .set(arg);
        // Test calling one service from another.
        ctx.method_b(TestServiceImpl::INSTANCE_NAME, arg)
    }

    fn method_b(&self, ctx: ExecutionContext<'_>, arg: u64) -> Result<(), ExecutionError> {
        ctx.service_data()
            .get_proof_entry("method_b_entry")
            .set(arg);
        Ok(())
    }
}

impl Service for TestServiceImpl {
    fn initialize(
        &self,
        context: ExecutionContext<'_>,
        params: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        let init = Init::from_bytes(params.into()).map_err(CommonError::malformed_arguments)?;
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

impl Test<ExecutionContext<'_>> for TestServiceImplV2 {
    type Output = Result<(), ExecutionError>;

    fn method_a(&self, _context: ExecutionContext<'_>, _arg: u64) -> Self::Output {
        Err(CommonError::NoSuchMethod.into())
    }

    fn method_b(&self, context: ExecutionContext<'_>, arg: u64) -> Self::Output {
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
    fn initialize(
        &self,
        context: ExecutionContext<'_>,
        params: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        // Due to the fact that our toy supervisor immediately executes start / stop requests,
        // caller might be `ToySupervisorService::INSTANCE_ID`.
        match context.caller() {
            Caller::Blockchain => {}
            Caller::Service { instance_id }
                if *instance_id == ToySupervisorService::INSTANCE_ID => {}
            other => panic!("Wrong caller type: {:?}", other),
        }

        let init = Init::from_bytes(params.into()).map_err(CommonError::malformed_arguments)?;
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

fn create_genesis_config_with_supervisor() -> GenesisConfig {
    create_genesis_config_builder()
        .with_artifact(ToySupervisorService.artifact_id())
        .with_instance(ToySupervisorService.default_instance())
        .build()
}

fn create_runtime(
    blockchain: Blockchain,
    genesis_config: GenesisConfig,
) -> (BlockchainMut, EventsHandle) {
    let inspected = Inspected::new(
        RustRuntimeBuilder::new()
            .with_factory(TestServiceImpl)
            .with_factory(TestServiceImplV2)
            .with_factory(ToySupervisorService)
            .with_factory(DependentServiceImpl)
            .build_for_tests(),
    );
    let events_handle = inspected.events.clone();

    let blockchain = BlockchainBuilder::new(blockchain)
        .with_genesis_config(genesis_config)
        .with_runtime(inspected)
        .build();
    (blockchain, events_handle)
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
    );
    let keypair = blockchain.as_ref().service_keypair().clone();

    // The dispatcher should initialize the runtime and call `after_commit` for
    // the genesis block.
    let supervisor = ToySupervisorService.default_instance();
    assert_eq!(
        events_handle.take(),
        vec![
            RuntimeEvent::InitializeRuntime,
            RuntimeEvent::DeployArtifact(ToySupervisorService.artifact_id(), vec![]),
            RuntimeEvent::StartAddingService(
                supervisor.instance_spec.clone(),
                supervisor.constructor
            ),
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
            RuntimeEvent::StartAddingService(
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

/// In this test, we try to create Rust runtime artifact with the non-empty spec.
#[test]
#[should_panic(expected = "Cannot deploy artifact because it has non-empty specification")]
fn create_runtime_non_empty_spec() {
    let genesis_config = create_genesis_config_builder()
        .with_parametric_artifact(TestServiceImpl.artifact_id(), vec![1, 2, 3, 4])
        .build();
    create_runtime(Blockchain::build_for_tests(), genesis_config);
}

/// In this test, we simulate blockchain restart and check events from inspector.
#[test]
fn runtime_restart() {
    // Create a runtime and a service test_service_artifact.
    let genesis_config = create_genesis_config_with_supervisor();
    let (mut blockchain, events_handle) =
        create_runtime(Blockchain::build_for_tests(), genesis_config.clone());
    let keypair = blockchain.as_ref().service_keypair().clone();

    // The dispatcher should initialize the runtime and call `after_commit` for
    // the genesis block.
    let supervisor = ToySupervisorService.default_instance();
    assert_eq!(
        events_handle.take(),
        vec![
            RuntimeEvent::InitializeRuntime,
            RuntimeEvent::DeployArtifact(ToySupervisorService.artifact_id(), vec![]),
            RuntimeEvent::StartAddingService(
                supervisor.instance_spec.clone(),
                supervisor.constructor
            ),
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
        create_runtime(blockchain.as_ref().clone(), genesis_config);
    let keypair = blockchain.as_ref().service_keypair().clone();

    assert_eq!(
        events_handle.take(),
        vec![
            RuntimeEvent::InitializeRuntime,
            RuntimeEvent::DeployArtifact(test_service_artifact, vec![]),
            RuntimeEvent::DeployArtifact(ToySupervisorService.artifact_id(), vec![]),
            // `Runtime::start_adding_service` is never called for the same service
            RuntimeEvent::CommitService(
                Height(2),
                supervisor.instance_spec.clone(),
                InstanceStatus::Active
            ),
            // `Runtime::after_commit` is never called for the same block
            RuntimeEvent::ResumeRuntime,
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
    let (blockchain, _) = create_runtime(Blockchain::build_for_tests(), genesis_config);

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
    let (mut blockchain, _) = create_runtime(Blockchain::build_for_tests(), genesis_config);
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
    assert_eq!(err, ErrorMatch::from_fail(&CommonError::NoSuchMethod));

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
    );
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
            RuntimeEvent::StartAddingService(
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
            RuntimeEvent::StartAddingService(
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
    assert_eq!(err, ErrorMatch::from_fail(&CoreError::IncorrectInstanceId));
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

    let (blockchain, _) = create_runtime(Blockchain::build_for_tests(), genesis_config);

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
#[should_panic(expected = "no dependency")]
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

    create_runtime(Blockchain::build_for_tests(), genesis_config);
}

#[test]
fn dependent_service_with_no_dependency() {
    let (mut blockchain, _) = create_runtime(
        Blockchain::build_for_tests(),
        create_genesis_config_with_supervisor(),
    );
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

#[test]
fn dependent_service_in_same_block() {
    let (mut blockchain, _) = create_runtime(
        Blockchain::build_for_tests(),
        create_genesis_config_with_supervisor(),
    );
    let keypair = blockchain.as_ref().service_keypair().clone();

    // Artifacts need to be deployed in a separate block due to checks in `RustRuntime`.
    let main_inst = TestServiceImpl.default_instance();
    let dep_inst = DependentServiceImpl.default_instance();

    let patch = create_block_with_transactions(
        &mut blockchain,
        vec![
            keypair.deploy_artifact(
                ToySupervisorService::INSTANCE_ID,
                DeployArtifact {
                    test_service_artifact: main_inst.instance_spec.artifact.clone(),
                    spec: vec![],
                },
            ),
            keypair.deploy_artifact(
                ToySupervisorService::INSTANCE_ID,
                DeployArtifact {
                    test_service_artifact: dep_inst.instance_spec.artifact.clone(),
                    spec: vec![],
                },
            ),
        ],
    );
    blockchain
        .commit(patch.1, patch.0, vec![], &mut BTreeMap::new())
        .unwrap();

    // Start both services in the same block.
    let patch = create_block_with_transactions(
        &mut blockchain,
        vec![
            keypair.start_service(
                ToySupervisorService::INSTANCE_ID,
                StartService {
                    spec: main_inst.instance_spec,
                    constructor: main_inst.constructor,
                },
            ),
            keypair.start_service(
                ToySupervisorService::INSTANCE_ID,
                StartService {
                    spec: dep_inst.instance_spec,
                    constructor: dep_inst.constructor,
                },
            ),
        ],
    );
    blockchain
        .commit(patch.1, patch.0, vec![], &mut BTreeMap::new())
        .unwrap();

    let snapshot = blockchain.snapshot();
    let schema = snapshot.for_dispatcher();
    assert_eq!(
        schema
            .get_instance(DependentServiceImpl::INSTANCE_NAME)
            .unwrap()
            .status
            .unwrap(),
        InstanceStatus::Active
    );
}

#[test]
fn dependent_service_in_successive_block() {
    let (mut blockchain, _) = create_runtime(
        Blockchain::build_for_tests(),
        create_genesis_config_builder()
            .with_artifact(ToySupervisorService.artifact_id())
            .with_instance(ToySupervisorService.default_instance())
            .with_artifact(TestServiceImpl.artifact_id())
            .with_instance(TestServiceImpl.default_instance())
            .build(),
    );
    let keypair = blockchain.as_ref().service_keypair().clone();

    let dep_service = DependentServiceImpl.default_instance();
    execute_transaction(
        &mut blockchain,
        keypair.deploy_artifact(
            ToySupervisorService::INSTANCE_ID,
            DeployArtifact {
                test_service_artifact: dep_service.instance_spec.artifact.clone(),
                spec: vec![],
            },
        ),
    )
    .unwrap();

    execute_transaction(
        &mut blockchain,
        keypair.start_service(
            ToySupervisorService::INSTANCE_ID,
            StartService {
                spec: dep_service.instance_spec.clone(),
                constructor: dep_service.constructor.clone(),
            },
        ),
    )
    .unwrap();

    let snapshot = blockchain.snapshot();
    let schema = snapshot.for_dispatcher();
    assert_eq!(
        schema
            .get_instance(DependentServiceImpl::INSTANCE_NAME)
            .unwrap()
            .status
            .unwrap(),
        InstanceStatus::Active
    );
}
