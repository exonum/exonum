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

use byteorder::{ByteOrder, LittleEndian};
use exonum_crypto::{gen_keypair, Hash};
use exonum_merkledb::{Database, Fork, ObjectHash, Patch, Snapshot, TemporaryDB};
use futures::{future, sync::mpsc, Future, IntoFuture};

use std::{
    collections::HashMap,
    mem, panic,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{channel, Sender},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

use crate::{
    blockchain::{Block, Blockchain, Schema as CoreSchema},
    helpers::{Height, ValidatorId},
    node::ApiSender,
    runtime::{
        dispatcher::{Action, Dispatcher, Mailbox},
        rust::{Error as RustRuntimeError, RustRuntime},
        ArtifactId, CallInfo, Caller, DeployStatus, DispatcherError, DispatcherSchema, ErrorKind,
        ExecutionContext, ExecutionError, InstanceId, InstanceSpec, MethodId, Runtime,
        RuntimeIdentifier,
    },
};

/// We guarantee that the genesis block will be committed by the time
/// `Runtime::after_commit()` is called. Thus, we need to perform this commitment
/// manually here, emulating the relevant part of `BlockchainMut::create_genesis_block()`.
fn create_genesis_block(dispatcher: &mut Dispatcher, fork: Fork) -> Patch {
    let is_genesis_block = CoreSchema::new(&fork).block_hashes_by_height().is_empty();
    assert!(is_genesis_block);

    let block = Block::new(
        ValidatorId(0),
        Height(0),
        0,
        Hash::zero(),
        Hash::zero(),
        Hash::zero(),
    );
    let block_hash = block.object_hash();
    let schema = CoreSchema::new(&fork);
    schema.block_hashes_by_height().push(block_hash);
    schema.blocks().put(&block_hash, block);

    let patch = dispatcher.commit_block(fork);
    dispatcher.notify_runtimes_about_commit(&patch);
    patch
}

impl Dispatcher {
    /// Similar to `Dispatcher::execute()`, but accepts universal `caller` and `call_info`.
    pub(crate) fn call(
        &self,
        fork: &mut Fork,
        caller: Caller,
        call_info: &CallInfo,
        arguments: &[u8],
    ) -> Result<(), ExecutionError> {
        let runtime = self
            .runtime_for_service(call_info.instance_id)
            .ok_or(DispatcherError::IncorrectRuntime)?;
        let context = ExecutionContext::new(self, fork, caller);
        runtime.execute(context, call_info, arguments)
    }
}

enum SampleRuntimes {
    First = 5,
    Second = 6,
}

#[derive(Debug)]
pub struct DispatcherBuilder {
    dispatcher: Dispatcher,
}

impl DispatcherBuilder {
    fn new() -> Self {
        Self {
            dispatcher: Dispatcher {
                runtimes: Default::default(),
                service_infos: Default::default(),
            },
        }
    }

    fn with_runtime(mut self, id: u32, runtime: impl Into<Box<dyn Runtime>>) -> Self {
        self.dispatcher.runtimes.insert(id, runtime.into());
        self
    }

    fn finalize(mut self, blockchain: &Blockchain) -> Dispatcher {
        for runtime in self.dispatcher.runtimes.values_mut() {
            runtime.initialize(blockchain);
        }
        self.dispatcher
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
    new_services: Vec<InstanceId>,
    new_service_sender: Sender<(u32, Vec<InstanceId>)>,
}

#[derive(Debug, IntoExecutionError)]
#[execution_error(crate = "crate")]
enum SampleError {
    Foo = 15,
}

impl SampleRuntime {
    fn new(
        runtime_type: u32,
        instance_id: InstanceId,
        method_id: MethodId,
        api_changes_sender: Sender<(u32, Vec<InstanceId>)>,
    ) -> Self {
        Self {
            runtime_type,
            instance_id,
            method_id,
            new_services: vec![],
            new_service_sender: api_changes_sender,
        }
    }
}

impl From<SampleRuntime> for Arc<dyn Runtime> {
    fn from(value: SampleRuntime) -> Self {
        Arc::new(value)
    }
}

impl Runtime for SampleRuntime {
    fn on_resume(&mut self) {
        let changes = mem::replace(&mut self.new_services, vec![]);
        self.new_service_sender
            .send((self.runtime_type, changes))
            .unwrap();
    }

    fn deploy_artifact(
        &mut self,
        artifact: ArtifactId,
        _spec: Vec<u8>,
    ) -> Box<dyn Future<Item = (), Error = ExecutionError>> {
        let res = if artifact.runtime_id == self.runtime_type {
            Ok(())
        } else {
            Err(DispatcherError::IncorrectRuntime.into())
        };
        Box::new(res.into_future())
    }

    fn is_artifact_deployed(&self, id: &ArtifactId) -> bool {
        id.runtime_id == self.runtime_type
    }

    fn commit_service(
        &mut self,
        _snapshot: &dyn Snapshot,
        spec: &InstanceSpec,
    ) -> Result<(), ExecutionError> {
        if spec.artifact.runtime_id == self.runtime_type {
            self.new_services.push(spec.id);
            Ok(())
        } else {
            Err(DispatcherError::IncorrectRuntime.into())
        }
    }

    fn start_adding_service(
        &self,
        _context: ExecutionContext<'_>,
        _spec: &InstanceSpec,
        _parameters: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn execute(
        &self,
        _context: ExecutionContext<'_>,
        call_info: &CallInfo,
        _parameters: &[u8],
    ) -> Result<(), ExecutionError> {
        if call_info.instance_id == self.instance_id && call_info.method_id == self.method_id {
            Ok(())
        } else {
            Err(SampleError::Foo.into())
        }
    }

    fn before_commit(
        &self,
        _context: ExecutionContext<'_>,
        _id: InstanceId,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn after_commit(&mut self, _snapshot: &dyn Snapshot, _mailbox: &mut Mailbox) {
        self.on_resume();
    }
}

#[test]
fn test_builder() {
    let runtime_a = SampleRuntime::new(SampleRuntimes::First as u32, 0, 0, channel().0);
    let runtime_b = SampleRuntime::new(SampleRuntimes::Second as u32, 1, 0, channel().0);

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
        ApiSender(mpsc::channel(1).0),
    );

    let (changes_tx, changes_rx) = channel();
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

    let rust_artifact = ArtifactId {
        runtime_id: SampleRuntimes::First as u32,
        name: "first".into(),
    };
    let java_artifact = ArtifactId {
        runtime_id: SampleRuntimes::Second as u32,
        name: "second".into(),
    };

    // Check if the services are ready for deploy.
    let mut fork = db.fork();
    dispatcher
        .deploy_artifact_sync(&fork, rust_artifact.clone(), vec![])
        .unwrap();
    dispatcher
        .deploy_artifact_sync(&fork, java_artifact.clone(), vec![])
        .unwrap();

    // Check if the services are ready for initiation. Note that the artifacts are pending at this
    // point.
    let rust_service = InstanceSpec {
        artifact: rust_artifact.clone(),
        id: RUST_SERVICE_ID,
        name: RUST_SERVICE_NAME.into(),
    };
    let mut context = ExecutionContext::new(&dispatcher, &mut fork, Caller::Blockchain);
    context
        .start_adding_service(rust_service, vec![])
        .expect("`start_adding_service` failed for rust");

    let java_service = InstanceSpec {
        artifact: java_artifact.clone(),
        id: JAVA_SERVICE_ID,
        name: JAVA_SERVICE_NAME.into(),
    };
    context
        .start_adding_service(java_service, vec![])
        .expect("`start_adding_service` failed for java");

    // Since services are not active yet, transactions to them should fail.
    let tx_payload = [0x00_u8; 1];
    dispatcher
        .call(
            &mut fork,
            Caller::Service { instance_id: 1 },
            &CallInfo::new(RUST_SERVICE_ID, RUST_METHOD_ID),
            &tx_payload,
        )
        .expect_err("Rust service should not be active yet");

    // Check that we cannot start adding a service with conflicting IDs.
    let conflicting_rust_service = InstanceSpec {
        artifact: rust_artifact.clone(),
        id: RUST_SERVICE_ID,
        name: "inconspicuous-name".to_owned(),
    };

    let mut context = ExecutionContext::new(&dispatcher, &mut fork, Caller::Blockchain);
    let err = context
        .start_adding_service(conflicting_rust_service, vec![])
        .unwrap_err();
    assert_eq!(err, DispatcherError::ServiceIdExists.into());

    let conflicting_rust_service = InstanceSpec {
        artifact: rust_artifact.clone(),
        id: RUST_SERVICE_ID + 1,
        name: RUST_SERVICE_NAME.to_owned(),
    };
    let err = context
        .start_adding_service(conflicting_rust_service, vec![])
        .unwrap_err();
    assert_eq!(err, DispatcherError::ServiceNameExists.into());

    // Activate services / artifacts.
    let patch = create_genesis_block(&mut dispatcher, fork);
    db.merge(patch).unwrap();
    let mut fork = db.fork();

    // Check if transactions are ready for execution.
    dispatcher
        .call(
            &mut fork,
            Caller::Service { instance_id: 1 },
            &CallInfo::new(RUST_SERVICE_ID, RUST_METHOD_ID),
            &tx_payload,
        )
        .expect("Correct tx rust");
    dispatcher
        .call(
            &mut fork,
            Caller::Service { instance_id: 1 },
            &CallInfo::new(RUST_SERVICE_ID, JAVA_METHOD_ID),
            &tx_payload,
        )
        .expect_err("Incorrect tx rust");
    dispatcher
        .call(
            &mut fork,
            Caller::Service { instance_id: 1 },
            &CallInfo::new(JAVA_SERVICE_ID, JAVA_METHOD_ID),
            &tx_payload,
        )
        .expect("Correct tx java");
    dispatcher
        .call(
            &mut fork,
            Caller::Service { instance_id: 1 },
            &CallInfo::new(JAVA_SERVICE_ID, RUST_METHOD_ID),
            &tx_payload,
        )
        .expect_err("Incorrect tx java");

    // Check that API changes in the dispatcher contain the started services.
    let expected_new_services = vec![
        (SampleRuntimes::First as u32, vec![RUST_SERVICE_ID]),
        (SampleRuntimes::Second as u32, vec![JAVA_SERVICE_ID]),
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
    let fork = db.fork();
    dispatcher
        .restore_state(fork.snapshot_without_unflushed_changes())
        .unwrap();

    assert_eq!(
        expected_new_services,
        changes_rx.iter().take(2).collect::<Vec<_>>()
    );
}

#[test]
fn test_dispatcher_rust_runtime_no_service() {
    const RUST_SERVICE_ID: InstanceId = 2;
    const RUST_SERVICE_NAME: &str = "rust-service";
    const RUST_METHOD_ID: MethodId = 0;

    // Create dispatcher and test data.
    let db = Arc::new(TemporaryDB::new());
    let blockchain = Blockchain::new(
        Arc::clone(&db) as Arc<dyn Database>,
        gen_keypair(),
        ApiSender(mpsc::channel(1).0),
    );

    let mut dispatcher = DispatcherBuilder::default()
        .with_runtime(
            RuntimeIdentifier::Rust as u32,
            RustRuntime::new(mpsc::channel(0).0),
        )
        .finalize(&blockchain);
    let rust_artifact = ArtifactId::new(RuntimeIdentifier::Rust as u32, "foo:1.0.0").unwrap();

    assert_eq!(
        dispatcher
            .deploy_artifact(rust_artifact.clone(), vec![])
            .wait()
            .expect_err("deploy artifact succeed"),
        RustRuntimeError::UnableToDeploy.into()
    );

    let mut fork = db.fork();
    let rust_service = InstanceSpec {
        artifact: rust_artifact.clone(),
        id: RUST_SERVICE_ID,
        name: RUST_SERVICE_NAME.into(),
    };
    assert_eq!(
        ExecutionContext::new(&dispatcher, &mut fork, Caller::Blockchain)
            .start_adding_service(rust_service, vec![])
            .expect_err("start service succeed"),
        DispatcherError::ArtifactNotDeployed.into()
    );

    let patch = create_genesis_block(&mut dispatcher, fork);
    db.merge(patch).unwrap();

    let mut fork = db.fork();
    let tx_payload = [0x00_u8; 1];
    dispatcher
        .call(
            &mut fork,
            Caller::Service { instance_id: 15 },
            &CallInfo::new(RUST_SERVICE_ID, RUST_METHOD_ID),
            &tx_payload,
        )
        .expect_err("execute succeed");
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
    fn deploy_artifact(
        &mut self,
        _artifact: ArtifactId,
        _spec: Vec<u8>,
    ) -> Box<dyn Future<Item = (), Error = ExecutionError>> {
        Box::new(Ok(()).into_future())
    }

    fn is_artifact_deployed(&self, _id: &ArtifactId) -> bool {
        false
    }

    fn start_adding_service(
        &self,
        _context: ExecutionContext<'_>,
        _spec: &InstanceSpec,
        _parameters: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn commit_service(
        &mut self,
        _snapshot: &dyn Snapshot,
        _spec: &InstanceSpec,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn execute(
        &self,
        _context: ExecutionContext<'_>,
        _call_info: &CallInfo,
        _parameters: &[u8],
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn before_commit(
        &self,
        _context: ExecutionContext<'_>,
        _id: InstanceId,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn after_commit(&mut self, _snapshot: &dyn Snapshot, _mailbox: &mut Mailbox) {}

    fn shutdown(&mut self) {
        self.turned_off.store(true, Ordering::Relaxed);
    }
}

#[test]
fn test_shutdown() {
    let turned_off_a = Arc::new(AtomicBool::new(false));
    let turned_off_b = Arc::new(AtomicBool::new(false));
    let runtime_a = ShutdownRuntime::new(turned_off_a.clone());
    let runtime_b = ShutdownRuntime::new(turned_off_b.clone());

    let mut dispatcher = DispatcherBuilder::new()
        .with_runtime(2, runtime_a)
        .with_runtime(3, runtime_b)
        .finalize(&Blockchain::build_for_tests());
    dispatcher.shutdown();

    assert_eq!(turned_off_a.load(Ordering::Relaxed), true);
    assert_eq!(turned_off_b.load(Ordering::Relaxed), true);
}

#[derive(Debug, Clone, Copy, Default)]
struct ArtifactStatus {
    attempts: usize,
    is_deployed: bool,
}

#[derive(Debug, Default, Clone)]
struct DeploymentRuntime {
    // Map of artifact names to deploy attempts and the flag for successful deployment.
    artifacts: Arc<Mutex<HashMap<String, ArtifactStatus>>>,
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
        dispatcher: &mut Dispatcher,
        db: &Arc<TemporaryDB>,
    ) -> (ArtifactId, Vec<u8>) {
        let artifact = ArtifactId {
            runtime_id: 2,
            name: name.to_owned(),
        };
        self.mailbox_actions
            .lock()
            .unwrap()
            .push(Action::StartDeploy {
                artifact: artifact.clone(),
                spec: Self::SPEC.to_vec(),
                and_then: Box::new(|| Box::new(Ok(()).into_future())),
            });
        let fork = db.fork();
        let patch = dispatcher.commit_block_and_notify_runtimes(fork);
        db.merge_sync(patch).unwrap();
        (artifact, Self::SPEC.to_vec())
    }
}

impl Runtime for DeploymentRuntime {
    fn deploy_artifact(
        &mut self,
        artifact: ArtifactId,
        spec: Vec<u8>,
    ) -> Box<dyn Future<Item = (), Error = ExecutionError>> {
        let delay = LittleEndian::read_u64(&spec);
        let delay = Duration::from_millis(delay);

        let error_kind = ErrorKind::runtime(0);
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
        let task = future::lazy(move || {
            // This isn't a correct way to delay future completion, but the correct way
            // (`tokio::timer::Delay`) cannot be used since the futures returned by
            // `Runtime::deploy_artifact()` are not (yet?) run on the `tokio` runtime.
            // TODO: Elaborate constraints on `Runtime::deploy_artifact` futures (ECR-3840)
            thread::sleep(delay);
            result
        })
        .then(move |res| {
            let mut artifacts = artifacts.lock().unwrap();
            let status = artifacts.entry(artifact.name).or_default();
            status.attempts += 1;
            if res.is_ok() {
                status.is_deployed = true;
            }
            res
        });
        Box::new(task)
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

    fn start_adding_service(
        &self,
        _context: ExecutionContext<'_>,
        _spec: &InstanceSpec,
        _parameters: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn commit_service(
        &mut self,
        _snapshot: &dyn Snapshot,
        _spec: &InstanceSpec,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn execute(
        &self,
        _context: ExecutionContext<'_>,
        _call_info: &CallInfo,
        _parameters: &[u8],
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn before_commit(
        &self,
        _context: ExecutionContext<'_>,
        _id: InstanceId,
    ) -> Result<(), ExecutionError> {
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
        ApiSender(mpsc::channel(1).0),
    );
    let runtime = DeploymentRuntime::default();
    let mut dispatcher = DispatcherBuilder::new()
        .with_runtime(2, runtime.clone())
        .finalize(&blockchain);

    let patch = create_genesis_block(&mut dispatcher, db.fork());
    db.merge_sync(patch).unwrap();

    // Queue an artifact for deployment.
    let (artifact, spec) = runtime.deploy_test_artifact("good", &mut dispatcher, &db);
    // Note that deployment via `Mailbox` is currently blocking, so after the method completion
    // the artifact should be immediately marked as deployed.
    assert!(dispatcher.is_artifact_deployed(&artifact));
    assert_eq!(runtime.deploy_attempts(&artifact), 1);

    // Check that we don't require the runtime to deploy the artifact again if we mark it
    // as committed.
    let fork = db.fork();
    Dispatcher::commit_artifact(&fork, artifact.clone(), spec).unwrap();
    let patch = dispatcher.commit_block_and_notify_runtimes(fork);
    db.merge_sync(patch).unwrap();
    assert_eq!(runtime.deploy_attempts(&artifact), 1);
}

fn test_failed_deployment(db: Arc<TemporaryDB>, runtime: DeploymentRuntime, artifact_name: &str) {
    let blockchain = Blockchain::new(
        Arc::clone(&db) as Arc<dyn Database>,
        gen_keypair(),
        ApiSender(mpsc::channel(1).0),
    );
    let mut dispatcher = DispatcherBuilder::new()
        .with_runtime(2, runtime.clone())
        .finalize(&blockchain);

    let patch = create_genesis_block(&mut dispatcher, db.fork());
    db.merge_sync(patch).unwrap();

    // Queue an artifact for deployment.
    let (artifact, spec) = runtime.deploy_test_artifact(artifact_name, &mut dispatcher, &db);
    // We should not panic during async deployment.
    assert!(!dispatcher.is_artifact_deployed(&artifact));
    assert_eq!(runtime.deploy_attempts(&artifact), 1);

    let fork = db.fork();
    Dispatcher::commit_artifact(&fork, artifact, spec).unwrap();
    dispatcher.commit_block_and_notify_runtimes(fork); // << should panic
}

#[test]
#[should_panic(expected = "Unable to deploy registered artifact")]
fn failed_deployment() {
    let db = Arc::new(TemporaryDB::new());
    let runtime = DeploymentRuntime::default();
    test_failed_deployment(db, runtime, "bad");
}

#[test]
fn failed_deployment_with_node_restart() {
    let db = Arc::new(TemporaryDB::new());
    let runtime = DeploymentRuntime::default();
    let db_ = Arc::clone(&db);
    let runtime_ = runtime.clone();
    panic::catch_unwind(|| test_failed_deployment(db_, runtime_, "recoverable_after_restart"))
        .expect_err("Node didn't stop after unsuccessful sync deployment");

    let snapshot = db.snapshot();
    let schema = DispatcherSchema::new(&snapshot);
    assert!(schema.get_artifact("recoverable_after_restart").is_none());
    // ^-- Since the node panicked before merging the block, the artifact is forgotten.

    // Emulate node restart. The node will obtain the same block with the `commit_artifact`
    // instruction, which has tripped it the last time, and try to commit it again. This time,
    // the commitment will be successful (e.g., the node couldn't download the artifact before,
    // but its admin has fixed the issue).
    let blockchain = Blockchain::new(
        Arc::clone(&db) as Arc<dyn Database>,
        gen_keypair(),
        ApiSender(mpsc::channel(1).0),
    );
    let mut dispatcher = DispatcherBuilder::new()
        .with_runtime(2, runtime)
        .finalize(&blockchain);

    let artifact = ArtifactId {
        runtime_id: 2,
        name: "recoverable_after_restart".to_owned(),
    };
    let mut spec = vec![0_u8; 8];
    LittleEndian::write_u64(&mut spec, 100);

    let fork = db.fork();
    Dispatcher::commit_artifact(&fork, artifact.clone(), spec).unwrap();
    let patch = dispatcher.commit_block_and_notify_runtimes(fork);
    db.merge_sync(patch).unwrap();
    assert!(dispatcher.is_artifact_deployed(&artifact));

    let snapshot = db.snapshot();
    let (_, status) = DispatcherSchema::new(&snapshot)
        .get_artifact(&artifact.name)
        .unwrap();
    assert_eq!(status, DeployStatus::Active);
}

#[test]
fn recoverable_error_during_deployment() {
    let db = Arc::new(TemporaryDB::new());
    let blockchain = Blockchain::new(
        Arc::clone(&db) as Arc<dyn Database>,
        gen_keypair(),
        ApiSender(mpsc::channel(1).0),
    );
    let runtime = DeploymentRuntime::default();
    let mut dispatcher = DispatcherBuilder::new()
        .with_runtime(2, runtime.clone())
        .finalize(&blockchain);

    let patch = create_genesis_block(&mut dispatcher, db.fork());
    db.merge_sync(patch).unwrap();

    // Queue an artifact for deployment.
    let (artifact, spec) = runtime.deploy_test_artifact("recoverable", &mut dispatcher, &db);
    assert!(!dispatcher.is_artifact_deployed(&artifact));
    assert_eq!(runtime.deploy_attempts(&artifact), 1);

    let fork = db.fork();
    Dispatcher::commit_artifact(&fork, artifact.clone(), spec).unwrap();
    dispatcher.commit_block_and_notify_runtimes(fork);
    // The dispatcher should try to deploy the artifact again despite a previous failure.
    assert!(dispatcher.is_artifact_deployed(&artifact));
    assert_eq!(runtime.deploy_attempts(&artifact), 2);
}
