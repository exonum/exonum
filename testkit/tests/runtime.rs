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

use exonum::{
    blockchain::InstanceConfig,
    crypto::{PublicKey, SecretKey},
    exonum_merkledb::{Fork, Snapshot},
    node::ApiSender,
    runtime::{
        ArtifactId, ArtifactProtobufSpec, CallInfo, ExecutionContext, ExecutionError, InstanceId,
        InstanceSpec, Mailbox, Runtime, StateHashAggregator,
    },
};
use exonum_testkit::TestKitBuilder;
use futures::{Future, IntoFuture};
use std::{sync::Arc, sync::RwLock};

// Tracks parts of state of runtime that we're interested in.
#[derive(Debug)]
struct RuntimeState {
    deployed_artifact: ArtifactId,
    deploy_spec: Vec<u8>,
    config_params: Vec<u8>,
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

    fn configure_service(&self, config_params: Vec<u8>) {
        let mut state = self.state.write().unwrap();
        state.config_params = config_params;
    }

    fn is_artifact_deployed(&self, artifact_id: &ArtifactId) -> bool {
        let state = self.state.read().unwrap();
        state.deployed_artifact == *artifact_id
    }

    fn assert_artifact_deployed(&self, artifact_id: ArtifactId, deploy_spec: Vec<u8>) {
        let state = self.state.read().unwrap();
        assert_eq!(state.deployed_artifact, artifact_id);
        assert_eq!(state.deploy_spec, deploy_spec);
    }

    fn assert_config_params_passed(&self, config_params: Vec<u8>) {
        let state = self.state.read().unwrap();
        assert_eq!(state.config_params, config_params)
    }
}

impl Default for RuntimeTester {
    fn default() -> Self {
        let state = RuntimeState {
            deployed_artifact: ArtifactId {
                runtime_id: Default::default(),
                name: Default::default(),
            },
            deploy_spec: Default::default(),
            config_params: Default::default(),
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
    // Runtime identifier.
    const ID: u32 = 42;

    pub fn with_runtime_tester(tester: Arc<RuntimeTester>) -> Self {
        TestRuntime { tester }
    }
}

impl Runtime for TestRuntime {
    fn deploy_artifact(
        &mut self,
        artifact: ArtifactId,
        deploy_spec: Vec<u8>,
    ) -> Box<dyn Future<Item = (), Error = ExecutionError>> {
        self.tester.deploy_artifact(artifact, deploy_spec);
        Box::new(Ok(()).into_future())
    }

    fn is_artifact_deployed(&self, id: &ArtifactId) -> bool {
        self.tester.is_artifact_deployed(id)
    }

    fn artifact_protobuf_spec(&self, _id: &ArtifactId) -> Option<ArtifactProtobufSpec> {
        Some(ArtifactProtobufSpec::default())
    }

    fn add_service(&mut self, _spec: &InstanceSpec) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn start_adding_service(
        &self,
        _fork: &Fork,
        _spec: &InstanceSpec,
        parameters: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        self.tester.configure_service(parameters);
        Ok(())
    }

    fn execute(
        &self,
        _context: ExecutionContext,
        _call_info: &CallInfo,
        _arguments: &[u8],
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

    fn after_commit(
        &mut self,
        _mailbox: &mut Mailbox,
        _snapshot: &dyn Snapshot,
        _service_keypair: &(PublicKey, SecretKey),
        _tx_sender: &ApiSender,
    ) {
    }
}

impl From<TestRuntime> for (u32, Box<dyn Runtime>) {
    fn from(inner: TestRuntime) -> Self {
        (TestRuntime::ID, Box::new(inner))
    }
}

// We assert that:
//  1) TestRuntime was passed to the testing blockchain
//  2) Artifact was deployed with correct deploy specification
//  3) Service was instantiated with correct initialization parameters
#[test]
fn test_runtime_factory() {
    let tester = Arc::new(RuntimeTester::default());

    let artifact_spec: Vec<u8> = "deploy_spec".into();
    let constructor: Vec<u8> = "constructor_params".into();
    let instance_spec = InstanceSpec::new(
        1,
        "test_instance",
        &format!("{}:{}", TestRuntime::ID, "artifact_name"),
    )
    .unwrap();
    let artifact_id = instance_spec.artifact.clone();

    let instances = vec![InstanceConfig::new(
        instance_spec.clone(),
        Some(artifact_spec.clone()),
        constructor.clone(),
    )];

    // This causes artifact deploying and service instantiation.
    TestKitBuilder::validator()
        .with_additional_runtime(TestRuntime::with_runtime_tester(tester.clone()))
        .with_instances(instances)
        .create();

    tester.assert_artifact_deployed(artifact_id, artifact_spec);
    tester.assert_config_params_passed(constructor);
}
