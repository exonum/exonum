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

use std::{collections::HashMap, sync::RwLock};

use crate::blockchain::ExecutionError;

use super::{
    ArtifactSpec, DeployError, DispatchInfo, EnvContext, InitError, InstanceInitData, InterfaceId,
    RuntimeEnvironment, ServiceInstanceId,
};

#[derive(Default)]
struct RustRuntime {
    inner: RwLock<RustRuntimeInner>,
}

impl RustRuntime {
    fn add_artifact(&self, spec: RustArtifactSpec, data: RustArtifactData) {
        let mut inner = self.inner.write().unwrap();
        inner.services.insert(spec, data);
    }
}

#[derive(Default)]
struct RustRuntimeInner {
    services: HashMap<RustArtifactSpec, RustArtifactData>,
    deployed: HashMap<RustArtifactSpec, RustArtifactData>,
    initialized: HashMap<ServiceInstanceId, RustArtifactData>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct RustArtifactSpec {
    name: String,
    version: (usize, usize, usize),
}

impl RuntimeEnvironment for RustRuntime {
    fn deploy(&self, artifact: ArtifactSpec) -> Result<(), DeployError> {
        let artifact = if let ArtifactSpec::Rust(artifact) = artifact {
            artifact
        } else {
            return Err(DeployError::WrongArtifact);
        };

        let mut inner = self.inner.write().expect("rust runtime write");

        let artifact_data = if let Some(data) = inner.services.remove(&artifact) {
            data
        } else {
            return Err(DeployError::FailedToDeploy);
        };

        if inner.deployed.insert(artifact, artifact_data).is_some() {
            return Err(DeployError::AlreadyDeployed);
        }
        Ok(())
    }

    fn start_init(
        &self,
        _: &mut EnvContext,
        artifact: ArtifactSpec,
        init: &InstanceInitData,
    ) -> Result<(), InitError> {
        let artifact = if let ArtifactSpec::Rust(artifact) = artifact {
            artifact
        } else {
            return Err(InitError::WrongArtifact);
        };

        let mut inner = self.inner.write().expect("rust runtime write");

        let artifact_data = if let Some(data) = inner.deployed.remove(&artifact) {
            data
        } else {
            return Err(InitError::NotDeployed);
        };

        if inner
            .initialized
            .insert(init.instance_id, artifact_data)
            .is_some()
        {
            return Err(InitError::ServiceIdExists);
        }
        Ok(())
    }

    fn finish_init(&self, _: &mut EnvContext, _: ServiceInstanceId, abort: bool) {
        if abort {
            unimplemented!()
        }
    }

    fn execute(&self, context: &mut EnvContext, dispatch: DispatchInfo, payload: &[u8]) {
        let inner = self.inner.read().unwrap();
        let instance = inner.initialized.get(&dispatch.instance_id).unwrap();
        let interface = instance.interfaces.get(&dispatch.interface_id).unwrap();
        let handler = interface.methods.get(dispatch.method_id as usize).unwrap();

        let mut ctx = TransactionContext::from_env_ctx(context);

        (handler.fun_untyped)(&mut ctx, payload);
    }
}

struct TransactionContext<'a> {
    env_context: &'a EnvContext<'a>,
}

impl<'a> TransactionContext<'a> {
    fn from_env_ctx(env_context: &'a EnvContext<'a>) -> Self {
        Self { env_context }
    }
}

struct Handler {
    pub fun_untyped: Box<dyn Fn(&mut TransactionContext, &[u8]) -> Result<(), ExecutionError>>,
}

struct ServiceInterface {
    methods: Vec<Handler>,
}

struct RustArtifactData {
    interfaces: HashMap<InterfaceId, ServiceInterface>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::schema::tests::TimestampTx;
    use crate::storage::{Database, MemoryDB};
    use protobuf::Message;

    /// Test service description: method, interface constructor, artifact builder.
    fn example_method(
        ctx: &mut TransactionContext,
        tx: &TimestampTx,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }
    fn get_example_interface() -> ServiceInterface {
        let handler = Handler {
            fun_untyped: Box::new(
                |ctx: &mut TransactionContext, payload: &[u8]| -> Result<(), ExecutionError> {
                    let mut tx = TimestampTx::new();
                    tx.merge_from_bytes(payload).unwrap();
                    example_method(ctx, &tx)
                },
            ),
        };
        ServiceInterface {
            methods: vec![handler],
        }
    }
    fn get_test_service_artifact() -> (RustArtifactSpec, RustArtifactData) {
        let spec = RustArtifactSpec {
            name: "service.zero".to_owned(),
            version: (1, 0, 0),
        };

        let interface_id = 4;
        let interfaces = vec![(interface_id, get_example_interface())]
            .into_iter()
            .collect();
        let data = RustArtifactData { interfaces };

        (spec, data)
    }

    #[test]
    fn test_rust_runtime_env() {
        let db = MemoryDB::new();

        let mut runtime = RustRuntime::default();
        let (serv_spec, serv_impl) = get_test_service_artifact();
        runtime.add_artifact(serv_spec.clone(), serv_impl);

        runtime
            .deploy(ArtifactSpec::Rust(serv_spec.clone()))
            .unwrap();

        let init_data = InstanceInitData {
            instance_id: 2,
            constructor_data: None,
        };

        {
            let mut fork = db.fork();
            let mut context = EnvContext::from_fork(&mut fork);
            runtime
                .start_init(
                    &mut context,
                    ArtifactSpec::Rust(serv_spec.clone()),
                    &init_data,
                )
                .unwrap();
            runtime.finish_init(&mut context, 2, false);
        }

        let dispatch_info = DispatchInfo {
            instance_id: 2,
            interface_id: 4,
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
            runtime.execute(&mut context, dispatch_info, &payload);
        }
    }
}
