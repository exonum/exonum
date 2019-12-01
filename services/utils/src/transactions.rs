//! Transaction logic for `UtilsService`.

use exonum::runtime::{rust::CallContext, AnyTx, DeployStatus, ExecutionError};
use exonum_derive::*;
use exonum_proto::ProtobufConvert;
use semver::VersionReq;
use serde_derive::*;

use crate::{proto, UtilsService};
use exonum::runtime::rust::ChildAuthorization;

/// Errors of the `UtilsService`.
#[derive(Debug, Clone, Copy, IntoExecutionError)]
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
    #[serde(with = "serde_str")]
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

/// Transactions executed in a batch.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ProtobufConvert, BinaryValue)]
#[protobuf_convert(source = "proto::Batch")]
pub struct Batch {
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

impl UtilsInterface for UtilsService {
    fn checked_call(
        &self,
        mut context: CallContext<'_>,
        arg: CheckedCall,
    ) -> Result<(), ExecutionError> {
        let instance_id = arg.inner.call_info.instance_id;
        let dispatcher_schema = context.data().for_dispatcher();
        let (state, status) = dispatcher_schema
            .get_instance(instance_id)
            .ok_or(Error::NoService)?;
        if status != DeployStatus::Active {
            return Err(Error::ServiceIsNotActive.into());
        }
        if arg.artifact_name != state.artifact.name {
            return Err(Error::ArtifactMismatch.into());
        }
        if !arg.artifact_version.matches(&state.artifact.version) {
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
