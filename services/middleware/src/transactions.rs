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

//! Transaction logic for `MiddlewareService`.

use exonum::runtime::{
    rust::{CallContext, ChildAuthorization},
    AnyTx, ExecutionError, InstanceStatus,
};
use exonum_derive::*;
use exonum_proto::ProtobufConvert;
use semver::VersionReq;
use serde_derive::*;

use crate::{proto, MiddlewareService};

/// Errors of the `MiddlewareService`.
#[derive(Debug, Clone, Copy, ExecutionFail)]
pub enum Error {
    /// The service instance targeted by the checked call does not exist.
    NoService = 0,
    /// The service instance targeted by the checked call is not active.
    ServiceIsNotActive = 1,
    /// The called service instance has an unexpected artifact.
    ArtifactMismatch = 2,
    /// The called service instance has an unsupported version.
    VersionMismatch = 3,
}

/// Checked call to the service.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, BinaryValue)]
pub struct CheckedCall {
    /// Expected name of the artifact.
    pub artifact_name: String,
    /// Version requirement(s) on the artifact.
    pub artifact_version: VersionReq,
    /// The call contents.
    pub inner: AnyTx,
}

impl ProtobufConvert for CheckedCall {
    type ProtoStruct = proto::CheckedCall;

    fn to_pb(&self) -> Self::ProtoStruct {
        let mut pb = Self::ProtoStruct::new();
        pb.set_artifact_name(self.artifact_name.clone());
        pb.set_artifact_version(self.artifact_version.to_string());
        pb.set_inner(self.inner.to_pb());
        pb
    }

    fn from_pb(mut pb: Self::ProtoStruct) -> Result<Self, failure::Error> {
        let artifact_name = pb.take_artifact_name();
        let artifact_version = pb.get_artifact_version().parse()?;
        let inner = AnyTx::from_pb(pb.take_inner())?;
        Ok(Self {
            artifact_name,
            artifact_version,
            inner,
        })
    }
}

#[test]
fn checked_call_in_json() {
    use exonum::runtime::CallInfo;
    use serde_json::json;

    let mut checked_call = CheckedCall {
        artifact_name: "test-artifact".to_string(),
        artifact_version: "^1.0.0".parse().unwrap(),
        inner: AnyTx {
            call_info: CallInfo::new(100, 0),
            arguments: vec![],
        },
    };
    assert_eq!(
        serde_json::to_value(&checked_call).unwrap(),
        json!({
            "artifact_name": "test-artifact",
            "artifact_version": "^1.0.0",
            "inner": checked_call.inner,
        })
    );

    checked_call.artifact_version = ">=0.9, <2".parse().unwrap();
    assert_eq!(
        serde_json::to_value(&checked_call).unwrap(),
        json!({
            "artifact_name": "test-artifact",
            "artifact_version": ">= 0.9, < 2",
            "inner": checked_call.inner,
        })
    );
}

/// Transactions executed in a batch.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ProtobufConvert, BinaryValue)]
#[protobuf_convert(source = "proto::Batch")]
pub struct Batch {
    /// Transactions included in the batch.
    pub inner: Vec<AnyTx>,
}

impl Batch {
    /// Creates an empty batch.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a call into the batch.
    pub fn with_call(mut self, call: AnyTx) -> Self {
        self.inner.push(call);
        self
    }
}

/// Transactional interface of the utilities service.
#[exonum_interface]
pub trait UtilsInterface {
    /// Performs a checked call to the service. The call is dispatched only if the version
    /// of the service matches the version requirement mentioned in the call.
    ///
    /// # Authorization
    ///
    /// The inner call is authorized in the same way as the `checked_call`.
    fn checked_call(
        &self,
        context: CallContext<'_>,
        arg: CheckedCall,
    ) -> Result<(), ExecutionError>;

    /// Performs batch execution of several transactions. Transactions are executed
    /// in the order they are mentioned in the batch. If execution of the constituent transaction
    /// fails, the method returns an error, thus rolling back any changes performed by earlier
    /// transactions.
    ///
    /// # Authorization
    ///
    /// All transactions are authorized in the same way as the `batch` call itself.
    fn batch(&self, context: CallContext<'_>, arg: Batch) -> Result<(), ExecutionError>;
}

impl UtilsInterface for MiddlewareService {
    fn checked_call(
        &self,
        mut context: CallContext<'_>,
        arg: CheckedCall,
    ) -> Result<(), ExecutionError> {
        let instance_id = arg.inner.call_info.instance_id;
        let dispatcher_schema = context.data().for_dispatcher();
        let state = dispatcher_schema
            .get_instance(instance_id)
            .ok_or(Error::NoService)?;
        if state.status != InstanceStatus::Active {
            return Err(Error::ServiceIsNotActive.into());
        }

        let artifact = &state.spec.artifact;
        if arg.artifact_name != artifact.name {
            return Err(Error::ArtifactMismatch.into());
        }
        if !arg.artifact_version.matches(&artifact.version) {
            return Err(Error::VersionMismatch.into());
        }

        context
            .call_context(instance_id, ChildAuthorization::Fallthrough)?
            .call("", arg.inner.call_info.method_id, arg.inner.arguments)
    }

    fn batch(&self, mut context: CallContext<'_>, arg: Batch) -> Result<(), ExecutionError> {
        for call in arg.inner {
            context
                .call_context(call.call_info.instance_id, ChildAuthorization::Fallthrough)?
                .call("", call.call_info.method_id, call.arguments)?;
        }
        Ok(())
    }
}
