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

use crate::proto::{self, ProtobufConvert};
use super::{
    error::{DeployError, ExecutionError, InitError},
    ArtifactSpec, DeployStatus, DispatchInfo, EnvContext, InstanceInitData, MethodId,
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, ProtobufConvert)]
#[exonum(pb = "proto::schema::runtime::RustArtifactSpec", crate = "crate")]
pub struct RustArtifactSpec {
    name: String,
    version: String,
}

impl RuntimeEnvironment for RustRuntime {
    fn start_deploy(&self, artifact: ArtifactSpec) -> Result<(), DeployError> {
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

    fn check_deploy_status(&self, artifact: ArtifactSpec) -> Result<DeployStatus, DeployError> {
        let artifact = if let ArtifactSpec::Rust(artifact) = artifact {
            artifact
        } else {
            return Err(DeployError::WrongArtifact);
        };

        let inner = self.inner.read().expect("rust runtime read");

        if inner.deployed.get(&artifact).is_some() {
            Ok(DeployStatus::Deployed)
        } else {
            Err(DeployError::FailedToDeploy)
        }
    }

    fn init_service(
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

    fn execute(
        &self,
        context: &mut EnvContext,
        dispatch: DispatchInfo,
        payload: &[u8],
    ) -> Result<(), ExecutionError> {
        let inner = self.inner.read().unwrap();
        let instance = inner.initialized.get(&dispatch.instance_id).unwrap();
        let handler = instance.methods.get(&dispatch.method_id).unwrap();

        let mut ctx = TransactionContext::from_env_ctx(context);

        (handler.fun_untyped)(&mut ctx, payload)
    }
}

struct TransactionContext<'a> {
    _env_context: &'a EnvContext<'a>,
}

impl<'a> TransactionContext<'a> {
    fn from_env_ctx(env_context: &'a EnvContext<'a>) -> Self {
        Self {
            _env_context: env_context,
        }
    }
}

struct Handler {
    pub fun_untyped: Box<dyn Fn(&mut TransactionContext, &[u8]) -> Result<(), ExecutionError>>,
}

struct RustArtifactData {
    methods: HashMap<MethodId, Handler>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::schema::tests::TimestampTx;
    use crate::storage::{Database, MemoryDB};
    use protobuf::Message;

    /// Test service description: method, interface constructor, artifact builder.
    fn example_method(
        _ctx: &mut TransactionContext,
        _tx: &TimestampTx,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn get_example_interface() -> RustArtifactData {
        let handler = Handler {
            fun_untyped: Box::new(
                |ctx: &mut TransactionContext, payload: &[u8]| -> Result<(), ExecutionError> {
                    let mut tx = TimestampTx::new();
                    tx.merge_from_bytes(payload).unwrap();
                    example_method(ctx, &tx)
                },
            ),
        };
        let mut methods = HashMap::new();
        methods.insert("method".to_owned(), handler);
        RustArtifactData {
            methods,
        }
    }
    fn get_test_service_artifact() -> (RustArtifactSpec, RustArtifactData) {
        let spec = RustArtifactSpec {
            name: "service.zero".to_owned(),
            version: "1.0.0".to_owned(),
        };

        let data = get_example_interface();

        (spec, data)
    }

    #[test]
    fn test_rust_runtime_env() {
        let db = MemoryDB::new();

        let runtime = RustRuntime::default();
        let (serv_spec, serv_impl) = get_test_service_artifact();
        runtime.add_artifact(serv_spec.clone(), serv_impl);

        runtime
            .start_deploy(ArtifactSpec::Rust(serv_spec.clone()))
            .unwrap();

        let init_data = InstanceInitData {
            instance_id: 2,
            constructor_data: None,
        };

        {
            let mut fork = db.fork();
            let mut context = EnvContext::from_fork(&mut fork);
            runtime
                .init_service(
                    &mut context,
                    ArtifactSpec::Rust(serv_spec.clone()),
                    &init_data,
                )
                .unwrap();
        }

        let dispatch_info = DispatchInfo {
            instance_id: 2,
            method_id: "method".to_string(),
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
}
