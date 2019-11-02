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

use exonum_crypto::{gen_keypair, Hash};
use exonum_merkledb::{Database, Fork, ObjectHash, Snapshot, TemporaryDB};
use futures::{sync::mpsc, Future, IntoFuture};

use std::{
    mem,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{channel, Sender},
        Arc,
    },
};

use crate::runtime::DispatcherState;
use crate::{
    blockchain::{Block, Blockchain, Schema as CoreSchema},
    helpers::{Height, ValidatorId},
    node::ApiSender,
    runtime::{
        dispatcher::{Dispatcher, Mailbox},
        rust::{Error as RustRuntimeError, RustRuntime},
        ArtifactId, ArtifactProtobufSpec, CallInfo, Caller, DispatcherError, DispatcherSchema,
        ExecutionContext, ExecutionError, InstanceId, InstanceSpec, MethodId, Runtime,
        RuntimeIdentifier, StateHashAggregator,
    },
};

/// We guarantee that the genesis block will be committed by the time
/// `Runtime::after_commit()` is called. Thus, we need to perform this commitment
/// manually here, emulating the relevant part of `BlockchainMut::create_genesis_block()`.
fn create_genesis_block(dispatcher: &mut Dispatcher, fork: &mut Fork) {
    let is_genesis_block = CoreSchema::get(&*fork).is_none();
    assert!(is_genesis_block);
    DispatcherSchema::initialize(fork);
    dispatcher.commit_block(fork);

    let block = Block::new(
        ValidatorId(0),
        Height(0),
        0,
        Hash::zero(),
        Hash::zero(),
        Hash::zero(),
    );
    let block_hash = block.object_hash();
    let schema = CoreSchema::initialize(&*fork);
    schema.block_hashes_by_height().push(block_hash);
    schema.blocks().put(&block_hash, block);
    fork.flush();
    dispatcher.notify_runtimes_about_commit(fork.snapshot_with_flushed_changes());
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
                shared_state: DispatcherState::default(),
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
#[exonum(crate = "crate")]
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
    ) -> Box<dyn Future<Item = ArtifactProtobufSpec, Error = ExecutionError>> {
        let res = if artifact.runtime_id == self.runtime_type {
            Ok(ArtifactProtobufSpec::default())
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
        _context: ExecutionContext,
        _spec: &InstanceSpec,
        _parameters: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn execute(
        &self,
        _context: ExecutionContext,
        call_info: &CallInfo,
        _parameters: &[u8],
    ) -> Result<(), ExecutionError> {
        if call_info.instance_id == self.instance_id && call_info.method_id == self.method_id {
            Ok(())
        } else {
            Err(SampleError::Foo.into())
        }
    }

    fn state_hashes(&self, _snapshot: &dyn Snapshot) -> StateHashAggregator {
        StateHashAggregator::default()
    }

    fn before_commit(
        &self,
        _context: ExecutionContext,
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
    DispatcherSchema::initialize(&fork);
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
    create_genesis_block(&mut dispatcher, &mut fork);

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
        .restore_state(fork.snapshot_with_flushed_changes())
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

    create_genesis_block(&mut dispatcher, &mut fork);
    db.merge(fork.into_patch()).unwrap();

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
    ) -> Box<dyn Future<Item = ArtifactProtobufSpec, Error = ExecutionError>> {
        Box::new(Ok(ArtifactProtobufSpec::default()).into_future())
    }

    fn is_artifact_deployed(&self, _id: &ArtifactId) -> bool {
        false
    }

    fn start_adding_service(
        &self,
        _context: ExecutionContext,
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
        _context: ExecutionContext,
        _call_info: &CallInfo,
        _parameters: &[u8],
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn state_hashes(&self, _snapshot: &dyn Snapshot) -> StateHashAggregator {
        StateHashAggregator::default()
    }

    fn before_commit(
        &self,
        _context: ExecutionContext,
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
