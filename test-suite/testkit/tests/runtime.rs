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

use exonum::runtime::{
    migrations::{InitMigrationError, MigrationScript},
    oneshot::Receiver,
    versioning::Version,
    ArtifactId, ExecutionContext, ExecutionError, InstanceState, Mailbox, MethodId, Runtime,
    WellKnownRuntime,
};
use exonum_merkledb::Snapshot;
use exonum_rust_runtime::spec::ForeignSpec;

use std::{sync::Arc, sync::RwLock};

use exonum_testkit::TestKitBuilder;

// Tracks parts of state of runtime that we're interested in.
#[derive(Debug)]
struct RuntimeState {
    deployed_artifact: ArtifactId,
    deploy_spec: Vec<u8>,
    constructor_params: Vec<u8>,
}

// Main purpose is to track and make some assertions on state of runtime.
#[derive(Debug)]
struct RuntimeTester {
    state: RwLock<RuntimeState>,
}

impl RuntimeTester {
    fn deploy_artifact(&self, artifact: ArtifactId, deploy_spec: Vec<u8>) {
        let mut state = self.state.write().unwrap();
        state.deployed_artifact = artifact;
        state.deploy_spec = deploy_spec;
    }

    fn construct_service(&self, constructor_params: Vec<u8>) {
        let mut state = self.state.write().unwrap();
        state.constructor_params = constructor_params;
    }

    fn is_artifact_deployed(&self, artifact_id: &ArtifactId) -> bool {
        let state = self.state.read().unwrap();
        state.deployed_artifact == *artifact_id
    }

    fn assert_artifact_deployed(&self, artifact_id: ArtifactId, deploy_spec: &[u8]) {
        let state = self.state.read().unwrap();
        assert_eq!(state.deployed_artifact, artifact_id);
        assert_eq!(state.deploy_spec, deploy_spec);
    }

    fn assert_constructor_params_passed(&self, constructor_params: &[u8]) {
        let state = self.state.read().unwrap();
        assert_eq!(state.constructor_params, constructor_params)
    }
}

impl Default for RuntimeTester {
    fn default() -> Self {
        let state = RuntimeState {
            deployed_artifact: ArtifactId::from_raw_parts(
                10,
                "test-artifact".to_owned(),
                "1.0.0".parse().unwrap(),
            ),
            deploy_spec: vec![],
            constructor_params: vec![],
        };

        Self {
            state: RwLock::new(state),
        }
    }
}

#[derive(Debug)]
struct TestRuntime {
    tester: Arc<RuntimeTester>,
}

impl TestRuntime {
    pub fn with_runtime_tester(tester: Arc<RuntimeTester>) -> Self {
        TestRuntime { tester }
    }
}

impl Runtime for TestRuntime {
    fn deploy_artifact(&mut self, artifact: ArtifactId, deploy_spec: Vec<u8>) -> Receiver {
        self.tester.deploy_artifact(artifact, deploy_spec);

        Receiver::with_result(Ok(()))
    }

    fn is_artifact_deployed(&self, id: &ArtifactId) -> bool {
        self.tester.is_artifact_deployed(id)
    }

    fn initiate_adding_service(
        &self,
        _context: ExecutionContext<'_>,
        _artifact: &ArtifactId,
        parameters: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        self.tester.construct_service(parameters);
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
        _arguments: &[u8],
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

impl WellKnownRuntime for TestRuntime {
    const ID: u32 = 42;
}

/// We assert that:
///
/// 1. TestRuntime was passed to the testing blockchain
/// 2. Artifact was deployed with correct deploy specification
/// 3. Service was instantiated with correct initialization parameters
#[test]
fn test_runtime_factory() {
    let tester = Arc::new(RuntimeTester::default());

    let artifact =
        ArtifactId::new(TestRuntime::ID, "artifact-name", Version::new(1, 0, 0)).unwrap();
    let deploy_args = b"deploy_spec";
    let constructor = b"constructor_params";

    // This causes artifact deploying and service instantiation.
    TestKitBuilder::validator()
        .with_additional_runtime(TestRuntime::with_runtime_tester(tester.clone()))
        .with(
            ForeignSpec::new(artifact.clone())
                .with_deploy_spec(deploy_args.to_vec())
                .with_instance(1, "test_instance", constructor.to_vec()),
        )
        .build();

    tester.assert_artifact_deployed(artifact, deploy_args);
    tester.assert_constructor_params_passed(constructor);
}
