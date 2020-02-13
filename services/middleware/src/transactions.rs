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

//! Transaction logic for `MiddlewareService`.

use exonum::runtime::{AnyTx, CoreError, ExecutionContext, ExecutionError, InstanceId};
use exonum_derive::*;
use exonum_proto::ProtobufConvert;
use exonum_rust_runtime::{FallthroughAuth, GenericCall, GenericCallMut, MethodDescriptor, TxStub};
use semver::VersionReq;
use serde_derive::*;

use crate::{proto, ArtifactReq, MiddlewareService};

/// Errors of the `MiddlewareService`.
#[derive(Debug, Clone, Copy, ExecutionFail)]
pub enum Error {
    /// The called service instance has an unexpected artifact.
    ArtifactMismatch = 0,
    /// The called service instance has an unsupported version.
    VersionMismatch = 1,
}

/// Checked call to the service.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[derive(BinaryValue, ProtobufConvert)]
#[protobuf_convert(source = "proto::CheckedCall")]
pub struct CheckedCall {
    /// Expected name of the artifact.
    pub artifact_name: String,
    /// Version requirement(s) on the artifact.
    #[protobuf_convert(with = "self::pb_version_req")]
    pub artifact_version: VersionReq,
    /// The call contents.
    pub inner: AnyTx,
}

mod pb_version_req {
    use super::*;

    pub fn from_pb(pb: String) -> Result<VersionReq, failure::Error> {
        pb.parse().map_err(From::from)
    }

    pub fn to_pb(value: &VersionReq) -> String {
        value.to_string()
    }
}

impl GenericCall<InstanceId> for ArtifactReq {
    type Output = CheckedCall;

    fn generic_call(
        &self,
        instance_id: InstanceId,
        method: MethodDescriptor<'_>,
        args: Vec<u8>,
    ) -> Self::Output {
        CheckedCall {
            artifact_name: self.0.name.clone(),
            artifact_version: self.0.version.clone(),
            inner: TxStub.generic_call(instance_id, method, args),
        }
    }
}

/// Transactions executed in a batch.
///
/// # Examples
///
/// `Batch` is a mutable stub, which adds corresponding transactions on call:
///
/// ```
/// // Suppose we have this interface defined.
/// mod token {
/// #   use exonum_derive::*;
///     #[exonum_interface]
///     pub trait Token<Ctx> {
///         type Output;
///         #[interface_method(id = 0)]
///         fn create_wallet(&self, ctx: Ctx, owner: String) -> Self::Output;
///         #[interface_method(id = 1)]
///         fn burn(&self, ctx: Ctx, amount: u64) -> Self::Output;
///         // Other methods...
///     }
/// }
///
/// # use exonum::runtime::InstanceId;
/// # use exonum_rust_runtime::DefaultInstance;
/// use exonum_middleware_service::{
///     ArtifactReq, Batch, MiddlewareInterfaceMut, MiddlewareService,
/// };
/// use token::{Token, TokenMut};
///
/// const TOKEN_ID: InstanceId = 100;
///
/// let mut batch = Batch::new();
/// batch.create_wallet(TOKEN_ID, "Alice".into());
/// batch.burn(TOKEN_ID, 50);
/// // Batch can include calls to multiple services. Let's add
/// // a checked call to the token service, which will be routed
/// // through a default middleware service instance.
/// let req: ArtifactReq = "exonum.Token@1".parse().unwrap();
/// let checked_call = req.burn(TOKEN_ID, 20);
/// batch.checked_call(MiddlewareService::INSTANCE_ID, checked_call);
/// assert_eq!(batch.inner.len(), 3);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue)]
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

impl GenericCallMut<InstanceId> for Batch {
    type Output = ();

    fn generic_call_mut(
        &mut self,
        instance_id: InstanceId,
        method: MethodDescriptor<'_>,
        args: Vec<u8>,
    ) -> Self::Output {
        self.inner
            .push(TxStub.generic_call(instance_id, method, args));
    }
}

/// Transactional interface of the utilities service.
#[exonum_interface]
pub trait MiddlewareInterface<Ctx> {
    /// Value output by the interface.
    type Output;

    /// Performs a checked call to the service. The call is dispatched only if the version
    /// of the service matches the version requirement mentioned in the call.
    ///
    /// # Authorization
    ///
    /// The inner call is authorized in the same way as the `checked_call`.
    #[interface_method(id = 0)]
    fn checked_call(&self, context: Ctx, arg: CheckedCall) -> Self::Output;

    /// Performs batch execution of several transactions. Transactions are executed
    /// in the order they are mentioned in the batch. If execution of the constituent transaction
    /// fails, the method returns an error, thus rolling back any changes performed by earlier
    /// transactions.
    ///
    /// # Authorization
    ///
    /// All transactions are authorized in the same way as the `batch` call itself.
    #[interface_method(id = 1)]
    fn batch(&self, context: Ctx, arg: Batch) -> Self::Output;
}

impl MiddlewareInterface<ExecutionContext<'_>> for MiddlewareService {
    type Output = Result<(), ExecutionError>;

    fn checked_call(&self, context: ExecutionContext<'_>, arg: CheckedCall) -> Self::Output {
        let instance_id = arg.inner.call_info.instance_id;
        let dispatcher_schema = context.data().for_dispatcher();
        let state = dispatcher_schema
            .get_instance(instance_id)
            .ok_or(CoreError::IncorrectInstanceId)?;

        let artifact = &state.spec.artifact;
        if arg.artifact_name != artifact.name {
            return Err(Error::ArtifactMismatch.into());
        }
        if !arg.artifact_version.matches(&artifact.version) {
            return Err(Error::VersionMismatch.into());
        }

        // TODO: use interface name from `call_info` once it's added there
        let method = MethodDescriptor::inherent(arg.inner.call_info.method_id);
        FallthroughAuth(context).generic_call_mut(instance_id, method, arg.inner.arguments)
    }

    fn batch(&self, context: ExecutionContext<'_>, arg: Batch) -> Self::Output {
        let mut fallthrough_auth = FallthroughAuth(context);
        for call in arg.inner {
            // TODO: use interface name from `call_info` once it's added there
            let method = MethodDescriptor::inherent(call.call_info.method_id);
            fallthrough_auth.generic_call_mut(
                call.call_info.instance_id,
                method,
                call.arguments,
            )?;
        }
        Ok(())
    }
}

#[test]
fn checked_call_in_json() {
    use exonum::runtime::CallInfo;
    use serde_json::json;

    let mut checked_call = CheckedCall {
        artifact_name: "test-artifact".to_string(),
        artifact_version: "^1.0.0".parse().unwrap(),
        inner: AnyTx::new(CallInfo::new(100, 0), vec![]),
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
