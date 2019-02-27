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

use crate::proto::schema::tests::TimestampTx;

use super::{
    service::SystemService, ArtifactSpec, RustArtifactSpec, RustRuntime, TransactionContext,
};
use crate::blockchain::ExecutionResult;
use crate::runtime::{
    error::ExecutionError, CallInfo, DeployStatus, EnvContext, InstanceInitData, RuntimeEnvironment,
};
use crate::storage::{Database, MemoryDB};
use protobuf::Message;

service_interface! {
    pub trait TestService {
        fn method_a(&self, ctx: TransactionContext, arg: TimestampTx) -> Result<(), ExecutionError>;
        fn method_b(&self, ctx: TransactionContext, arg: TimestampTx) -> Result<(), ExecutionError>;
    }
}

#[derive(Debug)]
pub struct TestServiceImpl;

impl TestService for TestServiceImpl {
    fn method_a(&self, _ctx: TransactionContext, _arg: TimestampTx) -> Result<(), ExecutionError> {
        Ok(())
    }
    fn method_b(&self, _ctx: TransactionContext, _arg: TimestampTx) -> Result<(), ExecutionError> {
        Ok(())
    }
}

impl_service_dispatcher!(TestServiceImpl, TestService);
impl SystemService for TestServiceImpl {}

#[test]
fn test_basic_rust_runtime() {
    let db = MemoryDB::new();

    let rust_artifact = RustArtifactSpec {
        name: "test_service".to_owned(),
        version: (0, 1, 0),
    };
    let artifact = ArtifactSpec::Rust(rust_artifact.clone());
    let service = Box::new(TestServiceImpl);

    let mut runtime = RustRuntime::default();

    runtime.add_service(rust_artifact.clone(), service);

    assert!(runtime.start_deploy(artifact.clone()).is_ok());

    assert_eq!(
        runtime.check_deploy_status(artifact.clone()).unwrap(),
        DeployStatus::Deployed
    );

    let init_data = InstanceInitData {
        instance_id: 2,
        constructor_data: None,
    };

    {
        let mut fork = db.fork();
        let mut context = EnvContext::from_fork(&mut fork);
        runtime
            .init_service(&mut context, artifact.clone(), &init_data)
            .unwrap();
    }

    // Execute transaction.
    let dispatch_info = CallInfo {
        instance_id: 2,
        method_id: 0,
    };
    let payload = {
        let mut tx = TimestampTx::new();
        tx.set_data(vec![0]);
        tx.write_to_bytes().unwrap()
    };
    {
        let mut fork = db.fork();
        let mut context = EnvContext::from_fork(&mut fork);
        runtime
            .execute(&mut context, dispatch_info, &payload)
            .unwrap();
    }
}
