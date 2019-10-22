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

use exonum_merkledb::{Database, TemporaryDB};
use futures::{sync::mpsc, Future, IntoFuture};

use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{sync_channel, SyncSender},
    Arc,
};

use crate::{
    crypto::{self, PublicKey, SecretKey},
    merkledb::{Fork, Snapshot},
    node::ApiSender,
    runtime::{
        dispatcher::{Dispatcher, Mailbox},
        rust::{Error as RustRuntimeError, RustRuntime},
        ApiChange, ApiContext, ArtifactId, ArtifactProtobufSpec, CallInfo, Caller, DispatcherError,
        ExecutionContext, ExecutionError, InstanceId, InstanceSpec, MethodId, Runtime,
        RuntimeIdentifier, StateHashAggregator,
    },
};

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
            dispatcher: Dispatcher::default(),
        }
    }

    fn with_runtime(mut self, id: u32, runtime: impl Into<Box<dyn Runtime>>) -> Self {
        self.dispatcher.runtimes.insert(id, runtime.into());
        self
    }

    fn finalize(self) -> Dispatcher {
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
    api_changes_sender: SyncSender<(u32, Vec<ApiChange>)>,
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
        api_changes_sender: SyncSender<(u32, Vec<ApiChange>)>,
    ) -> Self {
        Self {
            runtime_type,
            instance_id,
            method_id,
            api_changes_sender,
        }
    }
}

impl From<SampleRuntime> for Arc<dyn Runtime> {
    fn from(value: SampleRuntime) -> Self {
        Arc::new(value)
    }
}

impl Runtime for SampleRuntime {
    fn deploy_artifact(
        &mut self,
        artifact: ArtifactId,
        _spec: Vec<u8>,
    ) -> Box<dyn Future<Item = (), Error = ExecutionError>> {
        Box::new(
            if artifact.runtime_id == self.runtime_type {
                Ok(())
            } else {
                Err(DispatcherError::IncorrectRuntime.into())
            }
            .into_future(),
        )
    }

    fn is_artifact_deployed(&self, id: &ArtifactId) -> bool {
        id.runtime_id == self.runtime_type
    }

    fn add_service(&mut self, spec: &InstanceSpec) -> Result<(), ExecutionError> {
        if spec.artifact.runtime_id == self.runtime_type {
            Ok(())
        } else {
            Err(DispatcherError::IncorrectRuntime.into())
        }
    }

    fn start_adding_service(
        &self,
        _fork: &Fork,
        _spec: &InstanceSpec,
        _parameters: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn execute(
        &self,
        _: ExecutionContext,
        call_info: &CallInfo,
        _: &[u8],
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

    fn after_commit(
        &mut self,
        _mailbox: &mut Mailbox,
        _snapshot: &dyn Snapshot,
        _service_keypair: &(PublicKey, SecretKey),
        _tx_sender: &ApiSender,
    ) {
    }

    fn artifact_protobuf_spec(&self, _id: &ArtifactId) -> Option<ArtifactProtobufSpec> {
        Some(ArtifactProtobufSpec::default())
    }

    fn notify_api_changes(&self, _context: &ApiContext, changes: &[ApiChange]) {
        let changes = (self.runtime_type, changes.to_vec());
        self.api_changes_sender.send(changes).unwrap();
    }
}

#[test]
fn test_builder() {
    let runtime_a = SampleRuntime::new(SampleRuntimes::First as u32, 0, 0, sync_channel(1).0);
    let runtime_b = SampleRuntime::new(SampleRuntimes::Second as u32, 1, 0, sync_channel(1).0);

    let dispatcher = DispatcherBuilder::new()
        .with_runtime(runtime_a.runtime_type, runtime_a)
        .with_runtime(runtime_b.runtime_type, runtime_b)
        .finalize();

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

    let (changes_tx, changes_rx) = sync_channel(16);
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
        .finalize();

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
        .deploy_and_register_artifact(&fork, rust_artifact.clone(), vec![])
        .unwrap();
    dispatcher
        .deploy_and_register_artifact(&fork, java_artifact.clone(), vec![])
        .unwrap();

    // Check if the services are ready for initiation. Note that the artifacts are pending at this
    // point.
    let rust_service = InstanceSpec {
        artifact: rust_artifact.clone(),
        id: RUST_SERVICE_ID,
        name: RUST_SERVICE_NAME.into(),
    };
    dispatcher
        .start_adding_service(&fork, rust_service, vec![])
        .expect("`start_adding_service` failed for rust");

    let java_service = InstanceSpec {
        artifact: java_artifact.clone(),
        id: JAVA_SERVICE_ID,
        name: JAVA_SERVICE_NAME.into(),
    };
    dispatcher
        .start_adding_service(&fork, java_service, vec![])
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
    let err = dispatcher
        .start_adding_service(&fork, conflicting_rust_service, vec![])
        .unwrap_err();
    assert_eq!(err, DispatcherError::ServiceIdExists.into());

    let conflicting_rust_service = InstanceSpec {
        artifact: rust_artifact.clone(),
        id: RUST_SERVICE_ID + 1,
        name: RUST_SERVICE_NAME.to_owned(),
    };
    let err = dispatcher
        .start_adding_service(&fork, conflicting_rust_service, vec![])
        .unwrap_err();
    assert_eq!(err, DispatcherError::ServiceNameExists.into());

    // Activate services / artifacts by calling `Dispatcher::after_commit()`.
    let service_keypair = crypto::gen_keypair();
    let api_sender = ApiSender::new(mpsc::channel(0).0);
    dispatcher.after_commit(&fork, &service_keypair, &api_sender);

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
    let context = ApiContext::new(db.clone(), service_keypair, api_sender);
    assert!(dispatcher.notify_api_changes(&context));
    let expected_api_changes = vec![
        (
            SampleRuntimes::First as u32,
            vec![ApiChange::InstanceAdded(RUST_SERVICE_ID)],
        ),
        (
            SampleRuntimes::Second as u32,
            vec![ApiChange::InstanceAdded(JAVA_SERVICE_ID)],
        ),
    ];
    assert_eq!(
        expected_api_changes,
        changes_rx.iter().take(2).collect::<Vec<_>>()
    );
    // Check that API changes are empty after the `notify_api_changes`.
    assert!(dispatcher.api_changes.is_empty());

    // Check that API changes in the dispatcher contain the started services after restart.
    db.merge(fork.into_patch()).unwrap();
    let mut dispatcher = DispatcherBuilder::new()
        .with_runtime(runtime_a.runtime_type, runtime_a)
        .with_runtime(runtime_b.runtime_type, runtime_b)
        .finalize();
    dispatcher.restore_state(&db.snapshot()).unwrap();
    dispatcher.notify_api_changes(&ApiContext::new(
        db.clone(),
        crypto::gen_keypair(),
        ApiSender::new(mpsc::channel(0).0),
    ));

    assert_eq!(
        expected_api_changes,
        changes_rx.iter().take(2).collect::<Vec<_>>()
    );
}

#[test]
fn test_dispatcher_rust_runtime_no_service() {
    const RUST_SERVICE_ID: InstanceId = 2;
    const RUST_SERVICE_NAME: &str = "rust-service";
    const RUST_METHOD_ID: MethodId = 0;

    // Create dispatcher and test data.
    let db = TemporaryDB::new();

    let mut dispatcher = DispatcherBuilder::default()
        .with_runtime(RuntimeIdentifier::Rust as u32, RustRuntime::default())
        .finalize();

    let rust_artifact = ArtifactId::new(RuntimeIdentifier::Rust as u32, "foo:1.0.0").unwrap();

    assert_eq!(
        dispatcher
            .deploy_artifact(rust_artifact.clone(), vec![])
            .wait()
            .expect_err("deploy artifact succeed"),
        RustRuntimeError::UnableToDeploy.into()
    );

    let fork = db.fork();
    let rust_service = InstanceSpec {
        artifact: rust_artifact.clone(),
        id: RUST_SERVICE_ID,
        name: RUST_SERVICE_NAME.into(),
    };
    assert_eq!(
        dispatcher
            .start_adding_service(&fork, rust_service, vec![])
            .expect_err("start service succeed"),
        DispatcherError::ArtifactNotDeployed.into()
    );

    let service_keypair = crypto::gen_keypair();
    let api_sender = ApiSender::new(mpsc::channel(0).0);
    dispatcher.after_commit(&fork, &service_keypair, &api_sender);
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

impl From<ShutdownRuntime> for Arc<dyn Runtime> {
    fn from(value: ShutdownRuntime) -> Self {
        Arc::new(value)
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
        _fork: &Fork,
        _spec: &InstanceSpec,
        _parameters: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn add_service(&mut self, _spec: &InstanceSpec) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn execute(&self, _: ExecutionContext, _: &CallInfo, _: &[u8]) -> Result<(), ExecutionError> {
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

    fn after_commit(
        &mut self,
        _mailbox: &mut Mailbox,
        _snapshot: &dyn Snapshot,
        _service_keypair: &(PublicKey, SecretKey),
        _tx_sender: &ApiSender,
    ) {
    }

    fn artifact_protobuf_spec(&self, _id: &ArtifactId) -> Option<ArtifactProtobufSpec> {
        None
    }

    fn notify_api_changes(&self, _context: &ApiContext, _changes: &[ApiChange]) {}

    fn shutdown(&self) {
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
        .finalize();

    dispatcher.shutdown();

    assert_eq!(turned_off_a.load(Ordering::Relaxed), true);
    assert_eq!(turned_off_b.load(Ordering::Relaxed), true);
}
