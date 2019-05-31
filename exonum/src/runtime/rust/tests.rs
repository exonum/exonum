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

use exonum_derive::service_interface;
use exonum_merkledb::{BinaryValue, Database, Entry, Fork, TemporaryDB};
use protobuf::{well_known_types::Any, Message};
use semver::Version;

use crate::{
    crypto::{Hash, PublicKey},
    messages::{CallInfo, ServiceInstanceId},
    proto::schema::tests::{TestServiceInit, TestServiceTx},
    runtime::{
        error::{ExecutionError, WRONG_ARG_ERROR},
        DeployStatus, Runtime, RuntimeContext, ServiceConstructor,
    },
};

use super::{
    service::{Service, ServiceFactory},
    ArtifactSpec, RustArtifactSpec, RustRuntime, TransactionContext,
};

const SERVICE_INSTANCE_ID: ServiceInstanceId = 2;

#[derive(Debug, ProtobufConvert)]
#[exonum(pb = "TestServiceInit", crate = "crate")]
struct Init {
    msg: String,
}

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

#[service_interface(exonum(crate = "crate"))]
trait TestService {
    fn method_a(&self, ctx: TransactionContext, arg: TxA) -> Result<(), ExecutionError>;
    fn method_b(&self, ctx: TransactionContext, arg: TxB) -> Result<(), ExecutionError>;
}

#[derive(Debug)]
pub struct TestServiceImpl;

impl TestService for TestServiceImpl {
    fn method_a(&self, mut ctx: TransactionContext, arg: TxA) -> Result<(), ExecutionError> {
        {
            let fork = ctx.fork() as &Fork;
            let mut entry = Entry::new("method_a_entry", fork);
            entry.set(arg.value);
        }

        // Test calling one service from another.
        // TODO: It should be improved to support service auth in the future.
        let call_info = CallInfo {
            instance_id: SERVICE_INSTANCE_ID,
            method_id: 1,
        };
        let payload = TxB { value: arg.value }.into_bytes();
        ctx.dispatch_call(call_info, &payload)
            .expect("Failed to dispatch call");
        Ok(())
    }

    fn method_b(&self, ctx: TransactionContext, arg: TxB) -> Result<(), ExecutionError> {
        let fork = ctx.fork() as &Fork;
        let mut entry = Entry::new("method_b_entry", fork);
        entry.set(arg.value);
        Ok(())
    }
}

impl_service_dispatcher!(TestServiceImpl, TestService);

impl Service for TestServiceImpl {
    fn initialize(&mut self, ctx: TransactionContext, arg: &Any) -> Result<(), ExecutionError> {
        let arg: Init = BinaryValue::from_bytes(arg.get_value().into()).map_err(|e| {
            ExecutionError::with_description(WRONG_ARG_ERROR, format!("Wrong argument: {}", e))
        })?;

        let fork = ctx.fork() as &Fork;
        let mut entry = Entry::new("constructor_entry", fork);
        entry.set(arg.msg);
        Ok(())
    }
}

#[derive(Debug)]
struct TestServiceFactory;

impl ServiceFactory for TestServiceFactory {
    fn artifact(&self) -> RustArtifactSpec {
        RustArtifactSpec {
            name: "test_service".to_owned(),
            version: Version::new(0, 1, 0),
        }
    }

    fn new_instance(&self) -> Box<dyn Service> {
        Box::new(TestServiceImpl)
    }
}

#[test]
fn test_basic_rust_runtime() {
    let db = TemporaryDB::new();

    // Create runtime and service.
    let mut runtime = RustRuntime::new();

    let service_factory = Box::new(TestServiceFactory);
    let artifact: ArtifactSpec = service_factory.artifact().into();
    runtime.add_service_factory(service_factory);

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
        let constructor = ServiceConstructor {
            instance_id: SERVICE_INSTANCE_ID,
            data: {
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
        let mut context = RuntimeContext::new(&mut fork, address, tx_hash);
        runtime
            .init_service(&mut context, artifact.clone(), &constructor)
            .unwrap();

        {
            let entry = Entry::new("constructor_entry", &fork);
            assert_eq!(entry.get(), Some("constructor_message".to_owned()));
        }

        db.merge(fork.into_patch()).unwrap();
    }

    // Execute transaction method A.
    {
        const ARG_A_VALUE: u64 = 11;
        let call_info = CallInfo {
            instance_id: SERVICE_INSTANCE_ID,
            method_id: 0,
        };
        let payload = TxA { value: ARG_A_VALUE }.into_bytes();
        let mut fork = db.fork();
        let mut context = RuntimeContext::new(&mut fork, PublicKey::zero(), Hash::zero());
        runtime.execute(&mut context, call_info, &payload).unwrap();

        {
            let entry = Entry::new("method_a_entry", &fork);
            assert_eq!(entry.get(), Some(ARG_A_VALUE));
        }
        {
            let entry = Entry::new("method_b_entry", &fork);
            assert_eq!(entry.get(), Some(ARG_A_VALUE));
        }

        db.merge(fork.into_patch()).unwrap();
    }
    // Execute transaction method B.
    {
        const ARG_B_VALUE: u64 = 22;
        let call_info = CallInfo {
            instance_id: SERVICE_INSTANCE_ID,
            method_id: 1,
        };
        let payload = TxB { value: ARG_B_VALUE }.into_bytes();
        let mut fork = db.fork();
        let mut context = RuntimeContext::new(&mut fork, PublicKey::zero(), Hash::zero());
        runtime.execute(&mut context, call_info, &payload).unwrap();

        {
            let entry = Entry::new("method_b_entry", &fork);
            assert_eq!(entry.get(), Some(ARG_B_VALUE));
        }

        db.merge(fork.into_patch()).unwrap();
    }
}
