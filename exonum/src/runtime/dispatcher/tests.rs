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
    mpsc::{channel, Sender},
    Arc,
};

use crate::{
    crypto::{self, PublicKey, SecretKey},
    merkledb::{Fork, Snapshot},
    node::ApiSender,
    runtime::{
        dispatcher::Dispatcher,
        rust::{Error as RustRuntimeError, RustRuntime},
        ApiChange, ApiContext, ArtifactId, ArtifactProtobufSpec, CallInfo, Caller, DispatcherError,
        DispatcherRef, DispatcherSender, ExecutionContext, ExecutionError, InstanceDescriptor,
        InstanceId, InstanceSpec, MethodId, Runtime, RuntimeIdentifier, StateHashAggregator,
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
    api_changes_sender: Sender<(u32, Vec<ApiChange>)>,
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
        api_changes_sender: Sender<(u32, Vec<ApiChange>)>,
    ) -> Self {
        Self {
            runtime_type,
            instance_id,
            method_id,
            api_changes_sender,
        }
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

    fn start_service(&mut self, spec: &InstanceSpec) -> Result<(), ExecutionError> {
        if spec.artifact.runtime_id == self.runtime_type {
            Ok(())
        } else {
            Err(DispatcherError::IncorrectRuntime.into())
        }
    }

    fn initialize_service(
        &self,
        _fork: &Fork,
        _instance: InstanceDescriptor,
        _parameters: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn stop_service(&mut self, _instance: InstanceDescriptor) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn execute(
        &self,
        _: &ExecutionContext,
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

    fn before_commit(&self, _dispatcher: &DispatcherRef, _fork: &mut Fork) {}

    fn after_commit(
        &self,
        _dispatcher: &DispatcherSender,
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
    let runtime_a = SampleRuntime::new(SampleRuntimes::First as u32, 0, 0, channel().0);
    let runtime_b = SampleRuntime::new(SampleRuntimes::Second as u32, 1, 0, channel().0);

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
        .finalize();

    let sample_rust_spec = ArtifactId {
        runtime_id: SampleRuntimes::First as u32,
        name: "first".into(),
    };
    let sample_java_spec = ArtifactId {
        runtime_id: SampleRuntimes::Second as u32,
        name: "second".into(),
    };

    // Check if the services are ready for deploy.
    let fork = db.fork();
    dispatcher
        .deploy_and_register_artifact(&fork, &sample_rust_spec, Vec::default())
        .unwrap();
    dispatcher
        .deploy_and_register_artifact(&fork, &sample_java_spec, Vec::default())
        .unwrap();

    // Check if the services are ready for initiation.
    dispatcher
        .start_service(
            &fork,
            InstanceSpec {
                artifact: sample_rust_spec.clone(),
                id: RUST_SERVICE_ID,
                name: RUST_SERVICE_NAME.into(),
            },
            Vec::default(),
        )
        .expect("start_service failed for rust");
    dispatcher
        .start_service(
            &fork,
            InstanceSpec {
                artifact: sample_java_spec.clone(),
                id: JAVA_SERVICE_ID,
                name: JAVA_SERVICE_NAME.into(),
            },
            Vec::default(),
        )
        .expect("start_service failed for java");

    // Check if transactions are ready for execution.
    let tx_payload = [0x00_u8; 1];

    let dispatcher_ref = DispatcherRef::new(&dispatcher);
    let context = ExecutionContext::new(&dispatcher_ref, &fork, Caller::Service { instance_id: 1 });
    dispatcher
        .call(
            &context,
            &CallInfo::new(RUST_SERVICE_ID, RUST_METHOD_ID),
            &tx_payload,
        )
        .expect("Correct tx rust");

    dispatcher
        .call(
            &context,
            &CallInfo::new(RUST_SERVICE_ID, JAVA_METHOD_ID),
            &tx_payload,
        )
        .expect_err("Incorrect tx rust");

    dispatcher
        .call(
            &context,
            &CallInfo::new(JAVA_SERVICE_ID, JAVA_METHOD_ID),
            &tx_payload,
        )
        .expect("Correct tx java");

    dispatcher
        .call(
            &context,
            &CallInfo::new(JAVA_SERVICE_ID, RUST_METHOD_ID),
            &tx_payload,
        )
        .expect_err("Incorrect tx java");

    // Check that API changes in the dispatcher contain the started services.
    let context = ApiContext::new(
        db.clone(),
        crypto::gen_keypair(),
        ApiSender::new(mpsc::channel(0).0),
    );
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

    let sample_rust_spec = ArtifactId::new(RuntimeIdentifier::Rust as u32, "foo:1.0.0").unwrap();

    // Check deploy.
    assert_eq!(
        dispatcher
            .deploy_artifact(sample_rust_spec.clone(), Vec::default())
            .wait()
            .expect_err("deploy artifact succeed"),
        RustRuntimeError::UnableToDeploy.into()
    );

    // Check if the services are ready to start.
    let fork = db.fork();

    assert_eq!(
        dispatcher
            .start_service(
                &fork,
                InstanceSpec {
                    artifact: sample_rust_spec.clone(),
                    id: RUST_SERVICE_ID,
                    name: RUST_SERVICE_NAME.into()
                },
                Vec::default()
            )
            .expect_err("start service succeed"),
        DispatcherError::ArtifactNotDeployed.into()
    );

    // Check if transactions are ready for execution.
    let tx_payload = [0x00_u8; 1];

    let dispatcher_ref = DispatcherRef::new(&dispatcher);
    let context =
        ExecutionContext::new(&dispatcher_ref, &fork, Caller::Service { instance_id: 15 });
    dispatcher
        .call(
            &context,
            &CallInfo::new(RUST_SERVICE_ID, RUST_METHOD_ID),
            &tx_payload,
        )
        .expect_err("execute succeed");
}
