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

use semver::Version;

use crate::proto::schema::tests::{TestServiceInit, TestServiceTx};

use super::{service::Service, ArtifactSpec, RustArtifactSpec, RustRuntime, TransactionContext};
use crate::crypto::{Hash, PublicKey};
use crate::messages::{BinaryForm, CallInfo, ServiceInstanceId};
use crate::runtime::{
    error::{ExecutionError, WRONG_ARG_ERROR},
    DeployStatus, InstanceInitData, RuntimeContext, RuntimeEnvironment, RuntimeIdentifier,
};
use crate::storage::{Database, Entry, MemoryDB};
use protobuf::{well_known_types::Any, Message};

const SERVICE_INSTANCE_ID: ServiceInstanceId = 2;

#[derive(Debug, ProtobufConvert)]
#[exonum(pb = "TestServiceTx", crate = "crate")]
struct TxA {
    value: u64,
}

#[derive(Debug, ProtobufConvert)]
#[exonum(pb = "TestServiceTx", crate = "crate")]
struct TxB {
    value: u64,
}

service_interface! {
    trait TestService {
        fn method_a(&self, ctx: TransactionContext, arg: TxA) -> Result<(), ExecutionError>;
        fn method_b(&self, ctx: TransactionContext, arg: TxB) -> Result<(), ExecutionError>;
    }
}

#[derive(Debug)]
pub struct TestServiceImpl;

impl TestService for TestServiceImpl {
    fn method_a(&self, mut ctx: TransactionContext, arg: TxA) -> Result<(), ExecutionError> {
        let fork = ctx.fork();
        let mut entry = Entry::new("method_a_entry", fork);
        entry.set(arg.value);

        // Test calling one service from another.
        // TODO: It should be improved to support service auth in the future.
        let dispatch_info = CallInfo {
            instance_id: SERVICE_INSTANCE_ID,
            method_id: 1,
        };
        let payload = TxB { value: arg.value }.encode().unwrap();
        ctx.dispatch_call(dispatch_info, &payload)
            .expect("Failed to dispatch call");
        Ok(())
    }

    fn method_b(&self, mut ctx: TransactionContext, arg: TxB) -> Result<(), ExecutionError> {
        let fork = ctx.fork();
        let mut entry = Entry::new("method_b_entry", fork);
        entry.set(arg.value);
        Ok(())
    }
}

impl_service_dispatcher!(TestServiceImpl, TestService);
impl Service for TestServiceImpl {
    fn initialize(&mut self, mut ctx: TransactionContext, arg: Any) -> Result<(), ExecutionError> {
        let mut arg: TestServiceInit = BinaryForm::decode(arg.get_value()).map_err(|e| {
            ExecutionError::with_description(WRONG_ARG_ERROR, format!("Wrong argument: {}", e))
        })?;

        let fork = ctx.fork();
        let mut entry = Entry::new("constructor_entry", fork);
        entry.set(arg.take_msg());
        Ok(())
    }
}

fn get_artifact_spec() -> RustArtifactSpec {
    RustArtifactSpec {
        name: "test_service".to_owned(),
        version: Version::new(0, 1, 0),
    }
}

#[test]
fn test_basic_rust_runtime() {
    let db = MemoryDB::new();

    // Create runtime and service.
    let rust_artifact = get_artifact_spec();
    let artifact = ArtifactSpec {
        runtime_id: RuntimeIdentifier::Rust as u32,
        raw_spec: BinaryForm::encode(&rust_artifact).expect("Can't encode rust artifact"),
    };

    let service = Box::new(TestServiceImpl);

    let runtime = RustRuntime::default();
    runtime.add_service(rust_artifact.clone(), service);

    // Deploy service
    assert!(runtime.start_deploy(artifact.clone()).is_ok());
    assert_eq!(
        runtime
            .check_deploy_status(artifact.clone(), false)
            .unwrap(),
        DeployStatus::Deployed
    );

    // Init service
    {
        let init_data = InstanceInitData {
            instance_id: SERVICE_INSTANCE_ID,
            constructor_data: {
                let mut arg = TestServiceInit::new();
                arg.set_msg("constructor_message".to_owned());

                let mut pb_any = Any::new();
                pb_any.set_value(arg.write_to_bytes().unwrap());
                pb_any
            },
        };

        let mut fork = db.fork();
        let address = PublicKey::zero();
        let tx_hash = Hash::zero();
        let mut context = RuntimeContext::new(&mut fork, &address, &tx_hash);
        runtime
            .init_service(&mut context, artifact.clone(), &init_data)
            .unwrap();

        let entry = Entry::new("constructor_entry", &fork);
        assert_eq!(entry.get(), Some("constructor_message".to_owned()));

        db.merge(fork.into_patch()).unwrap();
    }

    // Execute transaction method A.
    {
        const ARG_A_VALUE: u64 = 11;
        let dispatch_info = CallInfo {
            instance_id: SERVICE_INSTANCE_ID,
            method_id: 0,
        };
        let payload = TxA { value: ARG_A_VALUE }.encode().unwrap();
        let mut fork = db.fork();
        let mut context = RuntimeContext::from_fork(&mut fork);
        runtime
            .execute(&mut context, dispatch_info, &payload)
            .unwrap();

        let entry = Entry::new("method_a_entry", &fork);
        assert_eq!(entry.get(), Some(ARG_A_VALUE));
        let entry = Entry::new("method_b_entry", &fork);
        assert_eq!(entry.get(), Some(ARG_A_VALUE));

        db.merge(fork.into_patch()).unwrap();
    }
    // Execute transaction method B.
    {
        const ARG_B_VALUE: u64 = 22;
        let dispatch_info = CallInfo {
            instance_id: SERVICE_INSTANCE_ID,
            method_id: 1,
        };
        let payload = TxB { value: ARG_B_VALUE }.encode().unwrap();
        let mut fork = db.fork();
        let mut context = RuntimeContext::from_fork(&mut fork);
        runtime
            .execute(&mut context, dispatch_info, &payload)
            .unwrap();

        let entry = Entry::new("method_b_entry", &fork);
        assert_eq!(entry.get(), Some(ARG_B_VALUE));

        db.merge(fork.into_patch()).unwrap();
    }
}
