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

use exonum_crypto::{gen_keypair, Hash};
use exonum_merkledb::{BinaryValue, Database, Fork, ObjectHash, Patch, Snapshot, TemporaryDB};
use pretty_assertions::assert_eq;
use semver::Version;

use std::{
    collections::{BTreeMap, HashMap},
    mem, panic,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::Duration,
};

use crate::runtime::CommonError;
use crate::{
    blockchain::{AdditionalHeaders, ApiSender, Block, Blockchain, Schema as CoreSchema},
    helpers::Height,
    runtime::{
        dispatcher::{Action, ArtifactStatus, Dispatcher, Mailbox},
        migrations::{InitMigrationError, MigrationScript},
        oneshot::{self, Receiver},
        ArtifactId, BlockchainData, CallInfo, CoreError, DispatcherSchema, ErrorKind, ErrorMatch,
        ExecutionContext, ExecutionError, InstanceDescriptor, InstanceId, InstanceSpec,
        InstanceState, InstanceStatus, MethodId, Runtime, RuntimeFeature, RuntimeInstance,
        SnapshotExt,
    },
};

/// We guarantee that the genesis block will be committed by the time
/// `Runtime::after_commit()` is called. Thus, we need to perform this commitment
/// manually here, emulating the relevant part of `BlockchainMut::create_genesis_block()`.
pub fn create_genesis_block(dispatcher: &mut Dispatcher, fork: Fork) -> Patch {
    let is_genesis_block = CoreSchema::new(&fork).block_hashes_by_height().is_empty();
    assert!(is_genesis_block);
    Dispatcher::activate_pending(&fork);

    let block = Block {
        height: Height(0),
        tx_count: 0,
        prev_hash: Hash::zero(),
        tx_hash: Hash::zero(),
        state_hash: Hash::zero(),
        error_hash: Hash::zero(),
        additional_headers: AdditionalHeaders::new(),
    };

    let block_hash = block.object_hash();
    let schema = CoreSchema::new(&fork);
    schema.block_hashes_by_height().push(block_hash);
    schema.blocks().put(&block_hash, block);

    let patch = dispatcher.commit_block(fork);
    dispatcher.notify_runtimes_about_commit(&patch);
    patch
}

impl Dispatcher {
    /// Similar to `Dispatcher::execute()`, but accepts arbitrary `call_info`.
    pub(crate) fn call(
        &self,
        fork: &mut Fork,
        call_info: &CallInfo,
        arguments: &[u8],
    ) -> Result<(), ExecutionError> {
        let instance = self
            .get_service(call_info.instance_id)
            .ok_or(CoreError::IncorrectInstanceId)?;
        let (_, runtime) = self
            .runtime_for_service(call_info.instance_id)
            .ok_or(CoreError::IncorrectInstanceId)?;

        let mut should_rollback = false;
        let context = ExecutionContext::for_block_call(self, fork, &mut should_rollback, instance);
        let res = runtime.execute(context, call_info.method_id, arguments);

        assert!(!should_rollback);
        res
    }
}

#[derive(Debug, Clone, Copy)]
enum SampleRuntimes {
    First = 5,
    Second = 6,
}

#[derive(Debug)]
pub struct DispatcherBuilder {
    runtimes: Vec<RuntimeInstance>,
}

impl DispatcherBuilder {
    pub fn new() -> Self {
        Self {
            runtimes: Vec::new(),
        }
    }

    pub fn with_runtime(mut self, id: u32, runtime: impl Into<Box<dyn Runtime>>) -> Self {
        self.runtimes.push(RuntimeInstance::new(id, runtime.into()));
        self
    }

    pub fn finalize(self, blockchain: &Blockchain) -> Dispatcher {
        Dispatcher::new(blockchain, self.runtimes)
    }
}

impl Default for DispatcherBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
struct SampleRuntime {
    runtime_type: u32,
    instance_id: InstanceId,
    method_id: MethodId,
    services: BTreeMap<InstanceId, InstanceStatus>,
    // `BTreeMap` is used to make services order predictable.
    new_services: BTreeMap<InstanceId, InstanceStatus>,
    new_service_sender: mpsc::Sender<(u32, Vec<(InstanceId, InstanceStatus)>)>,
}

impl SampleRuntime {
    fn new(
        runtime_type: u32,
        instance_id: InstanceId,
        method_id: MethodId,
        changes_sender: mpsc::Sender<(u32, Vec<(InstanceId, InstanceStatus)>)>,
    ) -> Self {
        Self {
            runtime_type,
            instance_id,
            method_id,
            services: BTreeMap::new(),
            new_services: BTreeMap::new(),
            new_service_sender: changes_sender,
        }
    }
}

#[allow(clippy::use_self)] // false positive
impl From<SampleRuntime> for Arc<dyn Runtime> {
    fn from(value: SampleRuntime) -> Self {
        Arc::new(value)
    }
}

impl Runtime for SampleRuntime {
    fn is_supported(&self, feature: &RuntimeFeature) -> bool {
        match feature {
            RuntimeFeature::FreezingServices => self.runtime_type == SampleRuntimes::First as u32,
        }
    }

    fn on_resume(&mut self) {
        if !self.new_services.is_empty() {
            let changes = mem::replace(&mut self.new_services, BTreeMap::new());
            self.new_service_sender
                .send((self.runtime_type, changes.into_iter().collect()))
                .ok();
        }
    }

    fn deploy_artifact(&mut self, artifact: ArtifactId, _spec: Vec<u8>) -> Receiver {
        let res = if artifact.runtime_id == self.runtime_type {
            Ok(())
        } else {
            Err(CoreError::IncorrectRuntime.into())
        };
        Receiver::with_result(res)
    }

    fn is_artifact_deployed(&self, id: &ArtifactId) -> bool {
        id.runtime_id == self.runtime_type
    }

    fn initiate_adding_service(
        &self,
        _context: ExecutionContext<'_>,
        _artifact: &ArtifactId,
        _parameters: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn initiate_resuming_service(
        &self,
        _context: ExecutionContext<'_>,
        _artifact: &ArtifactId,
        _parameters: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn update_service_status(&mut self, _snapshot: &dyn Snapshot, new_state: &InstanceState) {
        let spec = &new_state.spec;
        let new_status = new_state.status.as_ref().unwrap();

        assert_eq!(spec.artifact.runtime_id, self.runtime_type);
        let status_changed = if let Some(status) = self.services.get(&spec.id) {
            status != new_status
        } else {
            true
        };

        if status_changed {
            self.services.insert(spec.id, new_status.to_owned());
            self.new_services.insert(spec.id, new_status.to_owned());
        }
    }

    fn migrate(
        &self,
        _new_artifact: &ArtifactId,
        _data_version: &Version,
    ) -> Result<Option<MigrationScript>, InitMigrationError> {
        Err(InitMigrationError::NotSupported)
    }

    fn execute(
        &self,
        context: ExecutionContext<'_>,
        method_id: MethodId,
        _parameters: &[u8],
    ) -> Result<(), ExecutionError> {
        if context.instance().id == self.instance_id && method_id == self.method_id {
            Ok(())
        } else {
            let kind = ErrorKind::Service { code: 15 };
            Err(ExecutionError::new(kind, "oops"))
        }
    }

    fn before_transactions(&self, _context: ExecutionContext<'_>) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn after_transactions(&self, _context: ExecutionContext<'_>) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn after_commit(&mut self, _snapshot: &dyn Snapshot, _mailbox: &mut Mailbox) {
        self.on_resume();
    }
}

#[test]
fn test_builder() {
    let runtime_a = SampleRuntime::new(SampleRuntimes::First as u32, 0, 0, mpsc::channel().0);
    let runtime_b = SampleRuntime::new(SampleRuntimes::Second as u32, 1, 0, mpsc::channel().0);

    let dispatcher = DispatcherBuilder::new()
        .with_runtime(runtime_a.runtime_type, runtime_a)
        .with_runtime(runtime_b.runtime_type, runtime_b)
        .finalize(&Blockchain::build_for_tests());

    assert!(dispatcher
        .runtimes
        .get(&(SampleRuntimes::First as u32))
        .is_some());
    assert!(dispatcher
        .runtimes
        .get(&(SampleRuntimes::Second as u32))
        .is_some());
}

#[test]
#[allow(clippy::too_many_lines)] // Adequate for a test
fn test_dispatcher_simple() {
    const RUST_SERVICE_ID: InstanceId = 2;
    const JAVA_SERVICE_ID: InstanceId = 3;
    const RUST_SERVICE_NAME: &str = "rust-service";
    const JAVA_SERVICE_NAME: &str = "java-service";
    const RUST_METHOD_ID: MethodId = 0;
    const JAVA_METHOD_ID: MethodId = 1;

    // Create dispatcher and test data.
    let db = Arc::new(TemporaryDB::new());
    let blockchain = Blockchain::new(
        Arc::clone(&db) as Arc<dyn Database>,
        gen_keypair(),
        ApiSender::closed(),
    );

    let (changes_tx, changes_rx) = mpsc::channel();
    let runtime_a = SampleRuntime::new(
        SampleRuntimes::First as u32,
        RUST_SERVICE_ID,
        RUST_METHOD_ID,
        changes_tx.clone(),
    );
    let runtime_b = SampleRuntime::new(
        SampleRuntimes::Second as u32,
        JAVA_SERVICE_ID,
        JAVA_METHOD_ID,
        changes_tx,
    );

    let mut dispatcher = DispatcherBuilder::new()
        .with_runtime(runtime_a.runtime_type, runtime_a.clone())
        .with_runtime(runtime_b.runtime_type, runtime_b.clone())
        .finalize(&blockchain);

    let rust_artifact = ArtifactId::from_raw_parts(
        SampleRuntimes::First as _,
        "first".to_owned(),
        "0.5.0".parse().unwrap(),
    );
    let java_artifact = ArtifactId::from_raw_parts(
        SampleRuntimes::Second as _,
        "second".to_owned(),
        "1.2.1".parse().unwrap(),
    );

    // Check if the services are ready for deploy.
    let mut fork = db.fork();
    dispatcher.add_builtin_artifact(&fork, rust_artifact.clone(), vec![]);
    dispatcher.add_builtin_artifact(&fork, java_artifact.clone(), vec![]);

    // Check if the services are ready for initiation. Note that the artifacts are pending at this
    // point.
    let rust_service = InstanceSpec::from_raw_parts(
        RUST_SERVICE_ID,
        RUST_SERVICE_NAME.into(),
        rust_artifact.clone(),
    );

    let mut should_rollback = false;
    let mut context = ExecutionContext::for_block_call(
        &dispatcher,
        &mut fork,
        &mut should_rollback,
        rust_service.as_descriptor(),
    );
    context
        .initiate_adding_service(rust_service, vec![])
        .expect("`initiate_adding_service` failed for rust");

    let java_service =
        InstanceSpec::from_raw_parts(JAVA_SERVICE_ID, JAVA_SERVICE_NAME.into(), java_artifact);
    context
        .initiate_adding_service(java_service, vec![])
        .expect("`initiate_adding_service` failed for java");

    // Since services are not active yet, transactions to them should fail.
    let tx_payload = [0x00_u8; 1];
    dispatcher
        .call(
            &mut fork,
            &CallInfo::new(RUST_SERVICE_ID, RUST_METHOD_ID),
            &tx_payload,
        )
        .expect_err("Rust service should not be active yet");

    // Check that we cannot start adding a service with conflicting IDs.
    let conflicting_rust_service = InstanceSpec::from_raw_parts(
        RUST_SERVICE_ID,
        "inconspicuous-name".into(),
        rust_artifact.clone(),
    );

    let mut context = ExecutionContext::for_block_call(
        &dispatcher,
        &mut fork,
        &mut should_rollback,
        conflicting_rust_service.as_descriptor(),
    );
    let err = context
        .initiate_adding_service(conflicting_rust_service, vec![])
        .unwrap_err();
    assert_eq!(
        err,
        ErrorMatch::from_fail(&CoreError::ServiceIdExists)
            .with_description_containing("Service with numeric ID 2 already exists")
    );

    let conflicting_rust_service =
        InstanceSpec::from_raw_parts(RUST_SERVICE_ID + 1, RUST_SERVICE_NAME.into(), rust_artifact);
    let err = context
        .initiate_adding_service(conflicting_rust_service, vec![])
        .unwrap_err();
    assert_eq!(
        err,
        ErrorMatch::from_fail(&CoreError::ServiceNameExists)
            .with_description_containing("Service with name `rust-service` already exists")
    );

    // Activate services / artifacts.
    let patch = create_genesis_block(&mut dispatcher, fork);
    db.merge(patch).unwrap();
    let mut fork = db.fork();

    // Check if transactions are ready for execution.
    dispatcher
        .call(
            &mut fork,
            &CallInfo::new(RUST_SERVICE_ID, RUST_METHOD_ID),
            &tx_payload,
        )
        .expect("Correct tx rust");
    dispatcher
        .call(
            &mut fork,
            &CallInfo::new(RUST_SERVICE_ID, JAVA_METHOD_ID),
            &tx_payload,
        )
        .expect_err("Incorrect tx rust");
    dispatcher
        .call(
            &mut fork,
            &CallInfo::new(JAVA_SERVICE_ID, JAVA_METHOD_ID),
            &tx_payload,
        )
        .expect("Correct tx java");
    dispatcher
        .call(
            &mut fork,
            &CallInfo::new(JAVA_SERVICE_ID, RUST_METHOD_ID),
            &tx_payload,
        )
        .expect_err("Incorrect tx java");

    // Check that changes in the dispatcher contain the started services.
    let expected_new_services = vec![
        (
            SampleRuntimes::First as u32,
            vec![(RUST_SERVICE_ID, InstanceStatus::Active)],
        ),
        (
            SampleRuntimes::Second as u32,
            vec![(JAVA_SERVICE_ID, InstanceStatus::Active)],
        ),
    ];
    assert_eq!(
        expected_new_services,
        changes_rx.iter().take(2).collect::<Vec<_>>()
    );

    // Check that dispatcher restarts service instances.
    db.merge(fork.into_patch()).unwrap();
    let mut dispatcher = DispatcherBuilder::new()
        .with_runtime(runtime_a.runtime_type, runtime_a)
        .with_runtime(runtime_b.runtime_type, runtime_b)
        .finalize(&blockchain);
    dispatcher.restore_state(&db.snapshot());

    assert_eq!(
        expected_new_services,
        changes_rx.iter().take(2).collect::<Vec<_>>()
    );

    assert!(!should_rollback);
}

struct FreezingRig {
    dispatcher: Dispatcher,
    db: Arc<dyn Database>,
    runtime: SampleRuntime,
    changes_rx: mpsc::Receiver<(u32, Vec<(InstanceId, InstanceStatus)>)>,
    service: InstanceSpec,
}

fn blockchain_with_frozen_service(rt: SampleRuntimes) -> Result<FreezingRig, ExecutionError> {
    const SERVICE_ID: InstanceId = 0;
    const METHOD_ID: MethodId = 0;

    // Create dispatcher and test data.
    let db = Arc::new(TemporaryDB::new());
    let blockchain = Blockchain::new(
        Arc::clone(&db) as Arc<dyn Database>,
        gen_keypair(),
        ApiSender::closed(),
    );

    let (changes_tx, changes_rx) = mpsc::channel();
    let runtime = SampleRuntime::new(rt as u32, SERVICE_ID, METHOD_ID, changes_tx);
    let mut dispatcher = DispatcherBuilder::new()
        .with_runtime(runtime.runtime_type, runtime.clone())
        .finalize(&blockchain);

    let artifact =
        ArtifactId::from_raw_parts(rt as _, "first".to_owned(), "0.5.0".parse().unwrap());

    // Deploy the artifact and instantiate the service.
    let mut fork = db.fork();
    dispatcher.add_builtin_artifact(&fork, artifact.clone(), vec![]);
    let service = InstanceSpec::from_raw_parts(SERVICE_ID, "some-service".to_owned(), artifact);
    let mut should_rollback = false;
    let mut context = ExecutionContext::for_block_call(
        &dispatcher,
        &mut fork,
        &mut should_rollback,
        service.as_descriptor(),
    );
    context.initiate_adding_service(service.clone(), vec![])?;

    let patch = create_genesis_block(&mut dispatcher, fork);
    db.merge(patch).unwrap();

    let instantiated_change = (rt as u32, vec![(SERVICE_ID, InstanceStatus::Active)]);
    assert_eq!(instantiated_change, changes_rx.iter().next().unwrap());

    // Check that it is impossible to unload the artifact for an active service.
    let mut fork = db.fork();
    let err = Dispatcher::unload_artifact(&fork, &service.artifact).unwrap_err();
    let expected_msg = "service `0:some-service` references it";
    assert_eq!(
        err,
        ErrorMatch::from_fail(&CoreError::CannotUnloadArtifact)
            .with_description_containing(expected_msg)
    );

    // Command service freeze.
    let mut context = ExecutionContext::for_block_call(
        &dispatcher,
        &mut fork,
        &mut should_rollback,
        service.as_descriptor(),
    );
    context
        .supervisor_extensions()
        .initiate_freezing_service(SERVICE_ID)?;
    Dispatcher::activate_pending(&fork);
    let patch = dispatcher.commit_block_and_notify_runtimes(fork);
    db.merge(patch).unwrap();

    let frozen_change = (rt as u32, vec![(SERVICE_ID, InstanceStatus::Frozen)]);
    assert_eq!(frozen_change, changes_rx.iter().next().unwrap());

    // Check that it is impossible to unload the artifact for a frozen service.
    let fork = db.fork();
    let err = Dispatcher::unload_artifact(&fork, &service.artifact).unwrap_err();
    assert_eq!(
        err,
        ErrorMatch::from_fail(&CoreError::CannotUnloadArtifact)
            .with_description_containing(expected_msg)
    );

    Ok(FreezingRig {
        dispatcher,
        db,
        runtime,
        changes_rx,
        service,
    })
}

#[test]
fn test_service_freezing() {
    const SERVICE_ID: InstanceId = 0;
    const METHOD_ID: MethodId = 0;

    let FreezingRig {
        db,
        mut dispatcher,
        service,
        ..
    } = blockchain_with_frozen_service(SampleRuntimes::First).unwrap();

    // The service schema should be available.
    let snapshot = db.snapshot();
    assert!(snapshot.for_service(SERVICE_ID).is_some());

    // Check that the service no longer processes transactions.
    let mut fork = db.fork();
    let err = dispatcher
        .call(&mut fork, &CallInfo::new(SERVICE_ID, METHOD_ID), &[])
        .expect_err("Transaction was dispatched to frozen service");
    assert_eq!(err, ErrorMatch::from_fail(&CoreError::IncorrectInstanceId));

    // Change service status to stopped.
    let mut should_rollback = false;
    let mut context = ExecutionContext::for_block_call(
        &dispatcher,
        &mut fork,
        &mut should_rollback,
        service.as_descriptor(),
    );
    context
        .supervisor_extensions()
        .initiate_stopping_service(SERVICE_ID)
        .expect("Cannot stop service");
    Dispatcher::activate_pending(&fork);
    let patch = dispatcher.commit_block_and_notify_runtimes(fork);
    db.merge(patch).unwrap();

    // Check that the service cannot be easily changed to frozen.
    let mut fork = db.fork();
    let mut context = ExecutionContext::for_block_call(
        &dispatcher,
        &mut fork,
        &mut should_rollback,
        service.as_descriptor(),
    );
    let err = context
        .supervisor_extensions()
        .initiate_freezing_service(SERVICE_ID)
        .expect_err("Service cannot be frozen from `Stopped` status");
    let expected_msg = "transition is precluded by the current service status (stopped)";
    assert_eq!(
        err,
        ErrorMatch::from_fail(&CoreError::InvalidServiceTransition)
            .with_description_containing(expected_msg)
    );
}

#[test]
fn test_service_freezing_without_runtime_support() {
    let err = blockchain_with_frozen_service(SampleRuntimes::Second)
        .map(drop)
        .unwrap_err();
    assert_eq!(
        err,
        ErrorMatch::from_fail(&CommonError::FeatureNotSupported)
            .with_description_containing("Runtime with ID 6 does not support freezing services")
    );
}

#[test]
fn service_freeze_then_restart() {
    const SERVICE_ID: InstanceId = 0;
    const METHOD_ID: MethodId = 0;

    let FreezingRig {
        db,
        runtime,
        changes_rx,
        service,
        ..
    } = blockchain_with_frozen_service(SampleRuntimes::First).unwrap();

    // Emulate blockchain restart.
    let blockchain = Blockchain::new(Arc::clone(&db), gen_keypair(), ApiSender::closed());
    let mut dispatcher = DispatcherBuilder::new()
        .with_runtime(runtime.runtime_type, runtime)
        .finalize(&blockchain);
    dispatcher.restore_state(&db.snapshot());

    let service_change = (
        SampleRuntimes::First as u32,
        vec![(SERVICE_ID, InstanceStatus::Frozen)],
    );
    assert_eq!(service_change, changes_rx.iter().next().unwrap());

    // Check that the service does not accept transactions after restart.
    let mut fork = db.fork();
    let err = dispatcher
        .call(&mut fork, &CallInfo::new(SERVICE_ID, METHOD_ID), &[])
        .expect_err("Transaction was dispatched to frozen service");
    assert_eq!(err, ErrorMatch::from_fail(&CoreError::IncorrectInstanceId));

    // Resume the service.
    let mut should_rollback = false;
    let mut context = ExecutionContext::for_block_call(
        &dispatcher,
        &mut fork,
        &mut should_rollback,
        service.as_descriptor(),
    );
    context
        .supervisor_extensions()
        .initiate_resuming_service(SERVICE_ID, ())
        .expect("Cannot resume service");
    Dispatcher::activate_pending(&fork);
    let patch = dispatcher.commit_block_and_notify_runtimes(fork);
    db.merge(patch).unwrap();

    // Check that the service can process transactions again.
    let mut fork = db.fork();
    dispatcher
        .call(&mut fork, &CallInfo::new(SERVICE_ID, METHOD_ID), &[])
        .expect("Transaction was not processed by resumed service");
}

#[derive(Debug, Clone)]
struct ShutdownRuntime {
    turned_off: Arc<AtomicBool>,
}

impl ShutdownRuntime {
    fn new(turned_off: Arc<AtomicBool>) -> Self {
        Self { turned_off }
    }
}

impl Runtime for ShutdownRuntime {
    fn deploy_artifact(&mut self, _artifact: ArtifactId, _spec: Vec<u8>) -> Receiver {
        Receiver::with_result(Ok(()))
    }

    fn is_artifact_deployed(&self, _id: &ArtifactId) -> bool {
        false
    }

    fn initiate_adding_service(
        &self,
        _context: ExecutionContext<'_>,
        _artifact: &ArtifactId,
        _parameters: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn initiate_resuming_service(
        &self,
        _context: ExecutionContext<'_>,
        _artifact: &ArtifactId,
        _parameters: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn update_service_status(&mut self, _snapshot: &dyn Snapshot, _state: &InstanceState) {}

    fn migrate(
        &self,
        _new_artifact: &ArtifactId,
        _data_version: &Version,
    ) -> Result<Option<MigrationScript>, InitMigrationError> {
        Err(InitMigrationError::NotSupported)
    }

    fn execute(
        &self,
        _context: ExecutionContext<'_>,
        _method_id: MethodId,
        _parameters: &[u8],
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn before_transactions(&self, _context: ExecutionContext<'_>) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn after_transactions(&self, _context: ExecutionContext<'_>) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn after_commit(&mut self, _snapshot: &dyn Snapshot, _mailbox: &mut Mailbox) {}
}

impl Drop for ShutdownRuntime {
    fn drop(&mut self) {
        self.turned_off.store(true, Ordering::Relaxed);
    }
}

#[test]
fn test_shutdown() {
    let turned_off_a = Arc::new(AtomicBool::new(false));
    let turned_off_b = Arc::new(AtomicBool::new(false));
    let runtime_a = ShutdownRuntime::new(turned_off_a.clone());
    let runtime_b = ShutdownRuntime::new(turned_off_b.clone());

    let dispatcher = DispatcherBuilder::new()
        .with_runtime(2, runtime_a)
        .with_runtime(3, runtime_b)
        .finalize(&Blockchain::build_for_tests());
    drop(dispatcher);

    assert_eq!(turned_off_a.load(Ordering::Relaxed), true);
    assert_eq!(turned_off_b.load(Ordering::Relaxed), true);
}

#[derive(Debug, Clone, Copy, Default)]
struct ArtifactDeployStatus {
    attempts: usize,
    is_deployed: bool,
}

#[derive(Debug, Default, Clone)]
struct DeploymentRuntime {
    // Map of artifact names to deploy attempts and the flag for successful deployment.
    artifacts: Arc<Mutex<HashMap<String, ArtifactDeployStatus>>>,
    mailbox_actions: Arc<Mutex<Vec<Action>>>,
}

impl DeploymentRuntime {
    /// Artifact deploy spec. This is u64-LE encoding of the deploy delay in milliseconds.
    const SPEC: [u8; 8] = [100, 0, 0, 0, 0, 0, 0, 0];

    fn deploy_attempts(&self, artifact: &ArtifactId) -> usize {
        self.artifacts
            .lock()
            .unwrap()
            .get(&artifact.name)
            .copied()
            .unwrap_or_default()
            .attempts
    }

    /// Deploys a test artifact. Returns artifact ID and the deploy argument.
    fn deploy_test_artifact(
        &self,
        name: &str,
        version: &str,
        dispatcher: &mut Dispatcher,
        db: &Arc<TemporaryDB>,
    ) -> (ArtifactId, Vec<u8>) {
        let artifact = ArtifactId::from_raw_parts(2, name.to_owned(), version.parse().unwrap());
        self.mailbox_actions
            .lock()
            .unwrap()
            .push(Action::StartDeploy {
                artifact: artifact.clone(),
                spec: Self::SPEC.to_vec(),
                then: Box::new(|_| Ok(())),
            });

        let fork = db.fork();
        Dispatcher::activate_pending(&fork);
        let patch = dispatcher.commit_block_and_notify_runtimes(fork);
        db.merge_sync(patch).unwrap();
        (artifact, Self::SPEC.to_vec())
    }
}

impl Runtime for DeploymentRuntime {
    fn deploy_artifact(&mut self, artifact: ArtifactId, spec: Vec<u8>) -> oneshot::Receiver {
        let delay = BinaryValue::from_bytes(spec.into()).unwrap();
        let delay = Duration::from_millis(delay);

        let error_kind = ErrorKind::Runtime { code: 0 };
        let result = match artifact.name.as_str() {
            "good" => Ok(()),
            "bad" => Err(ExecutionError::new(error_kind, "bad artifact!")),
            "recoverable" => {
                if self.deploy_attempts(&artifact) > 0 {
                    Ok(())
                } else {
                    Err(ExecutionError::new(error_kind, "bad artifact!"))
                }
            }
            "recoverable_after_restart" => {
                if self.deploy_attempts(&artifact) > 1 {
                    Ok(())
                } else {
                    Err(ExecutionError::new(error_kind, "bad artifact!"))
                }
            }
            _ => unreachable!(),
        };

        let artifacts = Arc::clone(&self.artifacts);

        let mut artifacts = artifacts.lock().unwrap();
        let status = artifacts.entry(artifact.name).or_default();
        status.attempts += 1;
        if result.is_ok() {
            status.is_deployed = true;
        }

        let (tx, rx) = oneshot::channel();

        thread::spawn(move || {
            // This isn't a correct way to delay deploy completion.
            thread::sleep(delay);
            tx.send(result);
        });

        rx
    }

    fn is_artifact_deployed(&self, id: &ArtifactId) -> bool {
        self.artifacts
            .lock()
            .unwrap()
            .get(&id.name)
            .copied()
            .unwrap_or_default()
            .is_deployed
    }

    fn initiate_adding_service(
        &self,
        _context: ExecutionContext<'_>,
        _artifact: &ArtifactId,
        _parameters: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn initiate_resuming_service(
        &self,
        _context: ExecutionContext<'_>,
        _artifact: &ArtifactId,
        _parameters: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn update_service_status(&mut self, _snapshot: &dyn Snapshot, _state: &InstanceState) {}

    fn migrate(
        &self,
        _new_artifact: &ArtifactId,
        _data_version: &Version,
    ) -> Result<Option<MigrationScript>, InitMigrationError> {
        Err(InitMigrationError::NotSupported)
    }

    fn execute(
        &self,
        _context: ExecutionContext<'_>,
        _method_id: MethodId,
        _parameters: &[u8],
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn before_transactions(&self, _context: ExecutionContext<'_>) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn after_transactions(&self, _context: ExecutionContext<'_>) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn after_commit(&mut self, _snapshot: &dyn Snapshot, mailbox: &mut Mailbox) {
        for action in mem::replace(&mut *self.mailbox_actions.lock().unwrap(), vec![]) {
            mailbox.push(action);
        }
    }
}

#[test]
fn delayed_deployment() {
    let db = Arc::new(TemporaryDB::new());
    let blockchain = Blockchain::new(
        Arc::clone(&db) as Arc<dyn Database>,
        gen_keypair(),
        ApiSender::closed(),
    );
    let runtime = DeploymentRuntime::default();
    let mut dispatcher = DispatcherBuilder::new()
        .with_runtime(2, runtime.clone())
        .finalize(&blockchain);

    let patch = create_genesis_block(&mut dispatcher, db.fork());
    db.merge_sync(patch).unwrap();

    // Queue an artifact for deployment.
    let (artifact, spec) = runtime.deploy_test_artifact("good", "1.0.0", &mut dispatcher, &db);
    // Note that deployment via `Mailbox` is currently blocking, so after the method completion
    // the artifact should be immediately marked as deployed.
    assert!(dispatcher.is_artifact_deployed(&artifact));
    assert_eq!(runtime.deploy_attempts(&artifact), 1);

    // Check that we don't require the runtime to deploy the artifact again if we mark it
    // as committed.
    let fork = db.fork();
    Dispatcher::commit_artifact(&fork, &artifact, spec);
    Dispatcher::activate_pending(&fork);
    let patch = dispatcher.commit_block_and_notify_runtimes(fork);
    db.merge_sync(patch).unwrap();
    assert_eq!(runtime.deploy_attempts(&artifact), 1);
}

fn test_failed_deployment(db: &Arc<TemporaryDB>, runtime: &DeploymentRuntime, artifact_name: &str) {
    let blockchain = Blockchain::new(
        Arc::clone(db) as Arc<dyn Database>,
        gen_keypair(),
        ApiSender::closed(),
    );
    let mut dispatcher = DispatcherBuilder::new()
        .with_runtime(2, runtime.clone())
        .finalize(&blockchain);

    let patch = create_genesis_block(&mut dispatcher, db.fork());
    db.merge_sync(patch).unwrap();

    // Queue an artifact for deployment.
    let (artifact, spec) =
        runtime.deploy_test_artifact(artifact_name, "1.0.0", &mut dispatcher, db);
    // We should not panic during async deployment.
    assert!(!dispatcher.is_artifact_deployed(&artifact));
    assert_eq!(runtime.deploy_attempts(&artifact), 1);

    let fork = db.fork();
    Dispatcher::commit_artifact(&fork, &artifact, spec);
    Dispatcher::activate_pending(&fork);
    dispatcher.commit_block_and_notify_runtimes(fork); // << should panic
}

#[test]
#[should_panic(expected = "Unable to deploy registered artifact")]
fn failed_deployment() {
    let db = Arc::new(TemporaryDB::new());
    let runtime = DeploymentRuntime::default();
    test_failed_deployment(&db, &runtime, "bad");
}

#[test]
fn failed_deployment_with_node_restart() {
    let db = Arc::new(TemporaryDB::new());
    let runtime = DeploymentRuntime::default();
    panic::catch_unwind(|| test_failed_deployment(&db, &runtime, "recoverable_after_restart"))
        .expect_err("Node didn't stop after unsuccessful sync deployment");

    let snapshot = db.snapshot();
    let schema = DispatcherSchema::new(&snapshot);
    let artifact = "2:recoverable_after_restart:1.0.0".parse().unwrap();
    assert!(schema.get_artifact(&artifact).is_none());
    // ^-- Since the node panicked before merging the block, the artifact is forgotten.

    // Emulate node restart. The node will obtain the same block with the `commit_artifact`
    // instruction, which has tripped it the last time, and try to commit it again. This time,
    // the commitment will be successful (e.g., the node couldn't download the artifact before,
    // but its admin has fixed the issue).
    let blockchain = Blockchain::new(
        Arc::clone(&db) as Arc<dyn Database>,
        gen_keypair(),
        ApiSender::closed(),
    );
    let mut dispatcher = DispatcherBuilder::new()
        .with_runtime(2, runtime)
        .finalize(&blockchain);

    let spec = 100_u64.to_bytes();

    let fork = db.fork();
    Dispatcher::commit_artifact(&fork, &artifact, spec);
    Dispatcher::activate_pending(&fork);
    let patch = dispatcher.commit_block_and_notify_runtimes(fork);
    db.merge_sync(patch).unwrap();
    assert!(dispatcher.is_artifact_deployed(&artifact));

    let snapshot = db.snapshot();
    let state = DispatcherSchema::new(&snapshot)
        .get_artifact(&artifact)
        .unwrap();
    assert_eq!(state.status, ArtifactStatus::Active);
}

#[test]
fn recoverable_error_during_deployment() {
    let db = Arc::new(TemporaryDB::new());
    let blockchain = Blockchain::new(
        Arc::clone(&db) as Arc<dyn Database>,
        gen_keypair(),
        ApiSender::closed(),
    );
    let runtime = DeploymentRuntime::default();
    let mut dispatcher = DispatcherBuilder::new()
        .with_runtime(2, runtime.clone())
        .finalize(&blockchain);

    let patch = create_genesis_block(&mut dispatcher, db.fork());
    db.merge_sync(patch).unwrap();

    // Queue an artifact for deployment.
    let (artifact, spec) =
        runtime.deploy_test_artifact("recoverable", "1.0.0", &mut dispatcher, &db);
    assert!(!dispatcher.is_artifact_deployed(&artifact));
    assert_eq!(runtime.deploy_attempts(&artifact), 1);

    let fork = db.fork();
    Dispatcher::commit_artifact(&fork, &artifact, spec);
    Dispatcher::activate_pending(&fork);
    dispatcher.commit_block_and_notify_runtimes(fork);
    // The dispatcher should try to deploy the artifact again despite a previous failure.
    assert!(dispatcher.is_artifact_deployed(&artifact));
    assert_eq!(runtime.deploy_attempts(&artifact), 2);
}

#[test]
#[allow(clippy::too_many_lines)] // Adequate for a test
fn stopped_service_workflow() {
    let instance_id = 0;
    let instance_name = "supervisor";

    // Create blockchain with the sample runtime with the active service instance.
    let db = Arc::new(TemporaryDB::new());
    let blockchain = Blockchain::new(
        Arc::clone(&db) as Arc<dyn Database>,
        gen_keypair(),
        ApiSender::closed(),
    );

    let (changes_tx, changes_rx) = mpsc::channel();
    let runtime = SampleRuntime::new(SampleRuntimes::First as u32, 0, 0, changes_tx);

    let mut dispatcher = DispatcherBuilder::new()
        .with_runtime(runtime.runtime_type, runtime.clone())
        .finalize(&blockchain);

    let mut fork = db.fork();

    // Check that it is impossible to stop nonexistent service instance.
    let actual_err = Dispatcher::initiate_stopping_service(&fork, instance_id)
        .expect_err("`initiate_stopping_service` should fail");
    assert_eq!(
        actual_err,
        ErrorMatch::from_fail(&CoreError::IncorrectInstanceId)
            .with_description_containing("Cannot stop unknown service with ID 0")
    );

    let artifact = ArtifactId::from_raw_parts(
        SampleRuntimes::First as _,
        "first".into(),
        Version::new(0, 1, 0),
    );
    dispatcher.add_builtin_artifact(&fork, artifact.clone(), vec![]);

    let service = InstanceSpec::from_raw_parts(instance_id, instance_name.into(), artifact);
    let mut should_rollback = false;
    let mut context = ExecutionContext::for_block_call(
        &dispatcher,
        &mut fork,
        &mut should_rollback,
        service.as_descriptor(),
    );
    context
        .initiate_adding_service(service.clone(), vec![])
        .expect("`initiate_adding_service` failed");

    // Activate artifact and service.
    Dispatcher::activate_pending(&fork);
    let patch = dispatcher.commit_block_and_notify_runtimes(fork);
    db.merge_sync(patch).unwrap();
    let mut fork = db.fork();

    // Change instance status to stopped.
    Dispatcher::initiate_stopping_service(&fork, instance_id).unwrap();

    // Check if second initiation request fails.
    let actual_err = Dispatcher::initiate_stopping_service(&fork, instance_id)
        .expect_err("`initiate_stopping_service` should fail");
    assert_eq!(
        actual_err,
        ErrorMatch::from_fail(&CoreError::ServicePending)
    );

    // Check if transactions are still ready for execution.
    dispatcher
        .call(&mut fork, &CallInfo::new(instance_id, 0), &[])
        .expect("Service is not stopped yet, transaction should be processed");

    let dummy_descriptor = InstanceDescriptor::new(2, "dummy");

    // Check that service schema is still reachable.
    BlockchainData::new(&fork, &dummy_descriptor.name)
        .for_service(instance_name)
        .expect("Schema should be reachable");

    // Check that it is impossible to unload the artifact associated with a stopping service.
    let err = Dispatcher::unload_artifact(&fork, &service.artifact).unwrap_err();
    assert_eq!(
        err,
        ErrorMatch::from_fail(&CoreError::CannotUnloadArtifact).with_any_description()
    );

    // Commit service status.
    Dispatcher::activate_pending(&fork);
    let patch = dispatcher.commit_block_and_notify_runtimes(fork);
    db.merge_sync(patch).unwrap();
    let mut fork = db.fork();

    // Check if transactions become incorrect.
    dispatcher
        .call(&mut fork, &CallInfo::new(instance_id, 0), &[])
        .expect_err("Service was stopped, transaction shouldn't be processed");

    // Check that service schema is now unreachable.
    assert!(
        BlockchainData::new(&fork, &dummy_descriptor.name)
            .for_service(instance_name)
            .is_none(),
        "Schema should be unreachable for stopped service"
    );

    // Check that it is impossible to unload the artifact associated with a stopped service.
    let err = Dispatcher::unload_artifact(&fork, &service.artifact).unwrap_err();
    assert_eq!(
        err,
        ErrorMatch::from_fail(&CoreError::CannotUnloadArtifact).with_any_description()
    );

    // Emulate dispatcher restart.
    let mut dispatcher = DispatcherBuilder::new()
        .with_runtime(runtime.runtime_type, runtime)
        .finalize(&blockchain);
    dispatcher.restore_state(&db.snapshot());

    // Check expected notifications.
    let expected_notifications = vec![
        (
            SampleRuntimes::First as u32,
            vec![(instance_id, InstanceStatus::Active)],
        ),
        (
            SampleRuntimes::First as u32,
            vec![(instance_id, InstanceStatus::Stopped)],
        ),
        (
            SampleRuntimes::First as u32,
            vec![(instance_id, InstanceStatus::Stopped)],
        ),
    ];

    assert_eq!(
        changes_rx.iter().take(3).collect::<Vec<_>>(),
        expected_notifications
    );

    // Check if transactions is incorrect.
    let mut fork = db.fork();
    dispatcher
        .call(&mut fork, &CallInfo::new(instance_id, 0), &[])
        .expect_err("Service was stopped before restart, transaction shouldn't be processed");

    // Check that service schema is now unreachable.
    assert!(
        BlockchainData::new(&fork, &dummy_descriptor.name)
            .for_service(instance_name)
            .is_none(),
        "Service was stopped before restart, schema should be unreachable"
    );

    // Check that it is impossible to add previously stopped service.
    let mut context = ExecutionContext::for_block_call(
        &dispatcher,
        &mut fork,
        &mut should_rollback,
        service.as_descriptor(),
    );
    context
        .initiate_adding_service(service, vec![])
        .expect_err("`initiate_adding_service` should failed");

    // Check that it is impossible to stop service twice.
    let actual_err = Dispatcher::initiate_stopping_service(&fork, instance_id)
        .expect_err("`initiate_stopping_service` should fail");
    let bogus_transition_msg = "transition is precluded by the current service status (stopped)";
    assert_eq!(
        actual_err,
        ErrorMatch::from_fail(&CoreError::InvalidServiceTransition)
            .with_description_containing(bogus_transition_msg)
    );
    assert!(!should_rollback);
}

#[test]
fn unload_artifact_workflow() {
    const RUNTIME_ID: u32 = 2;

    let db = Arc::new(TemporaryDB::new());
    let blockchain = Blockchain::new(
        Arc::clone(&db) as Arc<dyn Database>,
        gen_keypair(),
        ApiSender::closed(),
    );
    let runtime = DeploymentRuntime::default();
    let mut dispatcher = DispatcherBuilder::new()
        .with_runtime(RUNTIME_ID, runtime.clone())
        .finalize(&blockchain);

    let patch = create_genesis_block(&mut dispatcher, db.fork());
    db.merge_sync(patch).unwrap();

    // Check that a non-deployed artifact cannot be unloaded.
    let artifact = ArtifactId::new(RUNTIME_ID, "good", Version::new(1, 0, 0)).unwrap();
    let fork = db.fork();
    let err = Dispatcher::unload_artifact(&fork, &artifact).unwrap_err();
    assert_eq!(
        err,
        ErrorMatch::from_fail(&CoreError::ArtifactNotDeployed)
            .with_description_containing("artifact `2:good:1.0.0`, which is not deployed")
    );

    // Deploy the artifact.
    let fork = db.fork();
    let spec = DeploymentRuntime::SPEC.to_vec();
    Dispatcher::commit_artifact(&fork, &artifact, spec);
    Dispatcher::activate_pending(&fork);
    let patch = dispatcher.commit_block_and_notify_runtimes(fork);
    db.merge_sync(patch).unwrap();
    assert_eq!(runtime.deploy_attempts(&artifact), 1);

    // Unload the artifact.
    let mut fork = db.fork();
    Dispatcher::unload_artifact(&fork, &artifact).unwrap();
    // Check that a duplicate unload request fails.
    let err = Dispatcher::unload_artifact(&fork, &artifact).unwrap_err();
    assert_eq!(
        err,
        ErrorMatch::from_fail(&CoreError::ArtifactNotDeployed)
            .with_description_containing("artifact `2:good:1.0.0`, which has non-active status")
    );

    // Check that a service cannot be instantiated from the artifact now that it's being unloaded.
    let service = InstanceSpec::from_raw_parts(100, "some-service".into(), artifact.clone());
    let mut should_rollback = false;
    let mut context = ExecutionContext::for_block_call(
        &dispatcher,
        &mut fork,
        &mut should_rollback,
        service.as_descriptor(),
    );
    let err = context
        .initiate_adding_service(service, vec![])
        .unwrap_err();
    let expected_msg = "from non-active artifact `2:good:1.0.0` (artifact status: unloading)";
    assert_eq!(
        err,
        ErrorMatch::from_fail(&CoreError::ArtifactNotDeployed)
            .with_description_containing(expected_msg)
    );

    Dispatcher::activate_pending(&fork);
    let patch = dispatcher.commit_block_and_notify_runtimes(fork);
    db.merge_sync(patch).unwrap();
    let snapshot = db.snapshot();
    let schema = DispatcherSchema::new(&snapshot);
    assert!(schema.get_artifact(&artifact).is_none());
}
