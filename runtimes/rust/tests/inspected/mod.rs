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

//! A collection of helpers for testing Rust runtime.

use exonum::{
    blockchain::{
        config::{GenesisConfigBuilder, InstanceInitParams},
        Blockchain, BlockchainMut, ConsensusConfig, Schema as CoreSchema,
    },
    crypto::Hash,
    helpers::{Height, ValidatorId},
    merkledb::{ObjectHash, Patch, Snapshot},
    messages::{AnyTx, Verified},
    runtime::{
        migrations::{InitMigrationError, MigrationScript},
        versioning::Version,
        ArtifactId, CallInfo, ExecutionContext, ExecutionError, InstanceId, InstanceSpec,
        InstanceStatus, Mailbox, Runtime, SnapshotExt, WellKnownRuntime, SUPERVISOR_INSTANCE_ID,
    },
};
use exonum_derive::{exonum_interface, BinaryValue, ServiceDispatcher, ServiceFactory};
use futures::Future;
use serde_derive::*;

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use exonum_rust_runtime::{CallContext, DefaultInstance, RustRuntime, Service, ServiceFactory};

pub fn execute_transaction(
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

pub fn create_block_with_transactions(
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

pub fn create_genesis_config_builder() -> GenesisConfigBuilder {
    let (consensus_config, _) = ConsensusConfig::for_tests(1);
    GenesisConfigBuilder::with_consensus_config(consensus_config)
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

#[derive(Debug, PartialEq)]
pub enum RuntimeEvent {
    InitializeRuntime,
    ResumeRuntime,
    BeforeTransactions(Height, InstanceId),
    DeployArtifact(ArtifactId, Vec<u8>),
    StartAddingService(InstanceSpec, Vec<u8>),
    MigrateService(ArtifactId, Version),
    StartResumingService(InstanceSpec, Vec<u8>),
    CommitService(Height, InstanceSpec, InstanceStatus),
    AfterTransactions(Height, InstanceId),
    AfterCommit(Height),
    Shutdown,
}

#[derive(Debug, Clone, Default)]
pub struct EventsHandle(Arc<Mutex<Vec<RuntimeEvent>>>);

impl EventsHandle {
    pub fn push(&self, event: RuntimeEvent) {
        self.0.lock().unwrap().push(event);
    }

    #[must_use]
    pub fn take(&self) -> Vec<RuntimeEvent> {
        self.0.lock().unwrap().drain(..).collect()
    }
}

/// Test runtime wrapper logging all the events (as `RuntimeEvent`) happening within it.
/// For service hooks the logged height is the height of the block **being processed**.
/// Other than logging, it just redirects all the calls to the inner runtime.
/// Used to test that workflow invariants are respected.
#[derive(Debug)]
pub struct Inspected<T> {
    runtime: T,
    pub events: EventsHandle,
}

impl<T: Runtime> Inspected<T> {
    pub fn new(runtime: T) -> Self {
        Self {
            runtime,
            events: Default::default(),
        }
    }
}

impl<T: Runtime> Runtime for Inspected<T> {
    fn initialize(&mut self, blockchain: &Blockchain) {
        self.events.push(RuntimeEvent::InitializeRuntime);
        self.runtime.initialize(blockchain)
    }

    fn on_resume(&mut self) {
        self.events.push(RuntimeEvent::ResumeRuntime);
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
        self.events.push(RuntimeEvent::StartAddingService(
            spec.to_owned(),
            parameters.clone(),
        ));
        self.runtime
            .initiate_adding_service(context, spec, parameters)
    }

    fn initiate_resuming_service(
        &self,
        context: ExecutionContext<'_>,
        spec: &InstanceSpec,
        parameters: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        self.events.push(RuntimeEvent::StartResumingService(
            spec.to_owned(),
            parameters.clone(),
        ));
        self.runtime
            .initiate_resuming_service(context, spec, parameters)
    }

    fn update_service_status(
        &mut self,
        snapshot: &dyn Snapshot,
        spec: &InstanceSpec,
        status: &InstanceStatus,
    ) {
        snapshot
            .for_dispatcher()
            .get_instance(spec.id)
            .expect("Service instance should exist");

        let core_schema = CoreSchema::new(snapshot);
        let height = core_schema.next_height();

        self.events.push(RuntimeEvent::CommitService(
            height,
            spec.to_owned(),
            status.to_owned(),
        ));
        self.runtime.update_service_status(snapshot, spec, status)
    }

    fn migrate(
        &self,
        new_artifact: &ArtifactId,
        data_version: &Version,
    ) -> Result<Option<MigrationScript>, InitMigrationError> {
        self.events.push(RuntimeEvent::MigrateService(
            new_artifact.to_owned(),
            data_version.clone(),
        ));
        self.runtime.migrate(new_artifact, data_version)
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

#[derive(Debug, Serialize, Deserialize, BinaryValue)]
#[binary_value(codec = "bincode")]
pub struct DeployArtifact {
    pub test_service_artifact: ArtifactId,
    pub spec: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize, BinaryValue)]
#[binary_value(codec = "bincode")]
pub struct StartService {
    pub spec: InstanceSpec,
    pub constructor: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize, BinaryValue)]
#[binary_value(codec = "bincode")]
pub struct StopService {
    pub instance_id: InstanceId,
}

#[derive(Debug, Serialize, Deserialize, BinaryValue)]
#[binary_value(codec = "bincode")]
pub struct ResumeService {
    pub artifact: ArtifactId,
    pub instance_id: InstanceId,
    pub params: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize, BinaryValue)]
#[binary_value(codec = "bincode")]
pub struct MigrateService {
    pub instance_name: String,
    pub artifact: ArtifactId,
}

#[exonum_interface(auto_ids)]
pub trait ToySupervisor<Ctx> {
    type Output;

    fn deploy_artifact(&self, context: Ctx, request: DeployArtifact) -> Self::Output;
    fn start_service(&self, context: Ctx, request: StartService) -> Self::Output;
    fn stop_service(&self, context: Ctx, request: StopService) -> Self::Output;
    fn resume_service(&self, context: Ctx, request: ResumeService) -> Self::Output;
    fn migrate_service(&self, context: Ctx, request: MigrateService) -> Self::Output;
}

#[derive(Debug, ServiceFactory, ServiceDispatcher)]
#[service_dispatcher(implements("ToySupervisor"))]
#[service_factory(artifact_name = "toy_supervisor", artifact_version = "0.1.0")]
pub struct ToySupervisorService;

impl ToySupervisor<CallContext<'_>> for ToySupervisorService {
    type Output = Result<(), ExecutionError>;

    fn deploy_artifact(
        &self,
        mut context: CallContext<'_>,
        request: DeployArtifact,
    ) -> Self::Output {
        context
            .supervisor_extensions()
            .start_artifact_registration(request.test_service_artifact, request.spec);
        Ok(())
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

    fn resume_service(&self, mut context: CallContext<'_>, request: ResumeService) -> Self::Output {
        context.supervisor_extensions().initiate_resuming_service(
            request.instance_id,
            request.artifact,
            request.params,
        )
    }

    fn migrate_service(
        &self,
        mut context: CallContext<'_>,
        request: MigrateService,
    ) -> Self::Output {
        context
            .supervisor_extensions()
            .initiate_migration(request.artifact, &request.instance_name)
    }
}

impl Service for ToySupervisorService {}

impl DefaultInstance for ToySupervisorService {
    const INSTANCE_ID: u32 = SUPERVISOR_INSTANCE_ID;
    const INSTANCE_NAME: &'static str = "supervisor";

    fn default_instance(&self) -> InstanceInitParams {
        self.artifact_id()
            .into_default_instance(Self::INSTANCE_ID, Self::INSTANCE_NAME)
    }
}
