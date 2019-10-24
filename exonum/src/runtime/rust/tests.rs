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

use exonum_derive::exonum_service;
use exonum_proto::ProtobufConvert;
use futures::sync::mpsc;

use crate::{
    blockchain::Blockchain,
    crypto::Hash,
    helpers::generate_testnet_config,
    merkledb::{BinaryValue, Entry, Snapshot},
    proto::schema::tests::{TestServiceInit, TestServiceTx},
    runtime::{
        error::ExecutionError, CallInfo, Caller, DispatcherError, ExecutionContext,
        InstanceDescriptor, InstanceId, InstanceSpec,
    },
};

use super::{
    service::{Service, ServiceFactory},
    ArtifactId, CallContext, RustRuntime,
};

const SERVICE_INSTANCE_ID: InstanceId = 2;
const SERVICE_INSTANCE_NAME: &str = "test_service_name";

#[derive(Debug, ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "TestServiceInit")]
pub struct Init {
    msg: String,
}

#[derive(Debug, ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "TestServiceTx")]
struct TxA {
    value: u64,
}

#[derive(Debug, ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "TestServiceTx")]
struct TxB {
    value: u64,
}

#[exonum_service(crate = "crate")]
trait TestService {
    fn method_a(&self, context: CallContext, arg: TxA) -> Result<(), ExecutionError>;
    fn method_b(&self, context: CallContext, arg: TxB) -> Result<(), ExecutionError>;
}

#[derive(Debug, ServiceFactory)]
#[exonum(
    crate = "crate",
    artifact_name = "test_service",
    artifact_version = "0.1.0",
    proto_sources = "crate::proto::schema",
    implements("TestService")
)]
pub struct TestServiceImpl;

#[derive(Debug)]
struct TestServiceClient<'a>(CallContext<'a>);

impl<'a> From<CallContext<'a>> for TestServiceClient<'a> {
    fn from(context: CallContext<'a>) -> Self {
        Self(context)
    }
}

impl<'a> TestServiceClient<'a> {
    fn method_b(&mut self, arg: TxB) -> Result<(), ExecutionError> {
        self.0.call("", 1, arg)
    }
}

impl TestService for TestServiceImpl {
    fn method_a(&self, mut context: CallContext, arg: TxA) -> Result<(), ExecutionError> {
        {
            let fork = context.fork();
            let mut entry = Entry::new("method_a_entry", fork);
            entry.set(arg.value);
        }

        // Test calling one service from another.
        context
            .interface::<TestServiceClient>(SERVICE_INSTANCE_ID)?
            .method_b(TxB { value: arg.value })
            .expect("Failed to dispatch call");
        Ok(())
    }

    fn method_b(&self, context: CallContext, arg: TxB) -> Result<(), ExecutionError> {
        let fork = context.fork();
        let mut entry = Entry::new("method_b_entry", fork);
        entry.set(arg.value);
        Ok(())
    }
}

impl Service for TestServiceImpl {
    fn initialize(&self, context: CallContext, params: Vec<u8>) -> Result<(), ExecutionError> {
        let init = Init::from_bytes(params.into()).map_err(DispatcherError::malformed_arguments)?;
        let mut entry = Entry::new("constructor_entry", context.fork());
        entry.set(init.msg);
        Ok(())
    }

    fn state_hash(&self, _instance: InstanceDescriptor, _snapshot: &dyn Snapshot) -> Vec<Hash> {
        vec![]
    }
}

#[test]
fn test_basic_rust_runtime() {
    // Create a runtime and a service.
    let mut runtime = RustRuntime::new(mpsc::channel(0).0);
    let service_factory = Box::new(TestServiceImpl);
    let artifact: ArtifactId = service_factory.artifact_id().into();
    runtime.add_service_factory(service_factory);

    // Create dummy dispatcher.
    let config = generate_testnet_config(1, 0)[0].clone();
    let mut blockchain = Blockchain::for_tests()
        .into_mut(config.consensus)
        .with_additional_runtime(runtime)
        .build()
        .unwrap();

    // Deploy service.
    let fork = blockchain.fork();
    blockchain
        .dispatcher()
        .deploy_artifact_sync(&fork, artifact.clone(), vec![])
        .unwrap();
    blockchain.merge(fork.into_patch()).unwrap();

    // Add service
    let spec = InstanceSpec {
        artifact,
        id: SERVICE_INSTANCE_ID,
        name: SERVICE_INSTANCE_NAME.to_owned(),
    };
    let constructor = Init {
        msg: "constructor_message".to_owned(),
    };
    let mut fork = blockchain.fork();
    ExecutionContext::new(blockchain.dispatcher(), &mut fork, Caller::BeforeCommit)
        .start_adding_service(spec, constructor)
        .unwrap();

    {
        let entry = Entry::new("constructor_entry", &fork);
        assert_eq!(entry.get(), Some("constructor_message".to_owned()));
    }
    blockchain.dispatcher().after_commit(&fork);
    blockchain.merge(fork.into_patch()).unwrap();

    // Execute transaction method A.
    const ARG_A_VALUE: u64 = 11;
    let call_info = CallInfo {
        instance_id: SERVICE_INSTANCE_ID,
        method_id: 0,
    };
    let payload = TxA { value: ARG_A_VALUE }.into_bytes();
    let caller = Caller::Service {
        instance_id: SERVICE_INSTANCE_ID,
    };
    let mut fork = blockchain.fork();
    blockchain
        .dispatcher()
        .call(&mut fork, caller, &call_info, &payload)
        .unwrap();

    {
        let entry = Entry::new("method_a_entry", &fork);
        assert_eq!(entry.get(), Some(ARG_A_VALUE));
        let entry = Entry::new("method_b_entry", &fork);
        assert_eq!(entry.get(), Some(ARG_A_VALUE));
    }
    blockchain.merge(fork.into_patch()).unwrap();

    // Execute transaction method B.
    const ARG_B_VALUE: u64 = 22;
    let call_info = CallInfo {
        instance_id: SERVICE_INSTANCE_ID,
        method_id: 1,
    };
    let payload = TxB { value: ARG_B_VALUE }.into_bytes();
    let caller = Caller::Service {
        instance_id: SERVICE_INSTANCE_ID,
    };
    let mut fork = blockchain.fork();
    blockchain
        .dispatcher()
        .call(&mut fork, caller, &call_info, &payload)
        .unwrap();

    {
        let entry = Entry::new("method_b_entry", &fork);
        assert_eq!(entry.get(), Some(ARG_B_VALUE));
    }
    blockchain.merge(fork.into_patch()).unwrap();
}
