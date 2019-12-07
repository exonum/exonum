use exonum_merkledb::{access::Prefixed, BinaryValue, Fork};

use crate::blockchain::Schema as CoreSchema;
use crate::runtime::{
    dispatcher::{Dispatcher, Error as DispatcherError},
    rust::stubs::{CallStub, MethodDescriptor},
    ArtifactId, BlockchainData, CallInfo, Caller, ExecutionContext, ExecutionError,
    InstanceDescriptor, InstanceQuery, InstanceSpec, SUPERVISOR_INSTANCE_ID,
};

/// Context for the executed call.
///
/// The call can mean a transaction call, `before_transactions` / `after_transactions` hook,
/// or the service constructor invocation.
#[derive(Debug)]
pub struct CallContext<'a> {
    /// Underlying execution context.
    inner: ExecutionContext<'a>,
    /// ID of the executing service.
    instance: InstanceDescriptor<'a>,
}

impl<'a> CallContext<'a> {
    /// Creates a new transaction context for the specified execution context and the instance
    /// descriptor.
    pub(crate) fn new(context: ExecutionContext<'a>, instance: InstanceDescriptor<'a>) -> Self {
        Self {
            inner: context,
            instance,
        }
    }

    /// Provides access to blockchain data.
    pub fn data(&self) -> BlockchainData<'a, &Fork> {
        BlockchainData::new(self.inner.fork, self.instance)
    }

    /// Provides access to the data of the executing service.
    pub fn service_data(&self) -> Prefixed<'a, &Fork> {
        self.data().for_executing_service()
    }

    /// Returns the initiator of the actual transaction execution.
    pub fn caller(&self) -> &Caller {
        &self.inner.caller
    }

    /// Returns a descriptor of the executing service instance.
    pub fn instance(&self) -> InstanceDescriptor<'_> {
        self.instance
    }

    // TODO This method is hidden until it is fully tested in next releases. [ECR-3494]
    #[doc(hidden)]
    pub fn client_stub<'s>(
        &'s mut self,
        called_id: impl Into<InstanceQuery<'s>>,
    ) -> Result<ClientStub<'s>, ExecutionError> {
        let descriptor = self
            .inner
            .dispatcher
            .get_service(self.inner.fork, called_id)
            .ok_or(DispatcherError::IncorrectInstanceId)?;
        Ok(ClientStub {
            inner: self.inner.child_context(self.instance.id),
            instance: descriptor,
        })
    }

    /// Provides writeable access to core schema.
    ///
    /// This method can only be called by the supervisor; the call will panic otherwise.
    #[doc(hidden)]
    pub fn writeable_core_schema(&self) -> CoreSchema<&Fork> {
        if self.instance.id != SUPERVISOR_INSTANCE_ID {
            panic!("`writeable_core_schema` called within a non-supervisor service");
        }
        CoreSchema::new(self.inner.fork)
    }

    /// Marks an artifact as *committed*, i.e., one which service instances can be deployed from.
    ///
    /// If / when a block with this instruction is accepted, artifact deployment becomes
    /// a requirement for all nodes in the network. A node that did not successfully
    /// deploy the artifact previously blocks until the artifact is deployed successfully.
    /// If a node cannot deploy the artifact, it panics.
    ///
    /// This method can only be called by the supervisor; the call will panic otherwise.
    #[doc(hidden)]
    pub fn start_artifact_registration(
        &self,
        artifact: ArtifactId,
        spec: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        if self.instance.id != SUPERVISOR_INSTANCE_ID {
            panic!("`start_artifact_registration` called within a non-supervisor service");
        }

        Dispatcher::commit_artifact(self.inner.fork, artifact, spec)
    }

    /// Starts adding a service instance to the blockchain.
    ///
    /// The service is not immediately activated; it activates if / when the block containing
    /// the activation transaction is committed.
    ///
    /// This method can only be called by the supervisor; the call will panic otherwise.
    #[doc(hidden)]
    pub fn start_adding_service(
        &mut self,
        instance_spec: InstanceSpec,
        constructor: impl BinaryValue,
    ) -> Result<(), ExecutionError> {
        if self.instance.id != SUPERVISOR_INSTANCE_ID {
            panic!("`start_adding_service` called within a non-supervisor service");
        }

        self.inner
            .child_context(self.instance.id)
            .start_adding_service(instance_spec, constructor)
    }
}

/// Client stub allowing to call methods of a service on the same blockchain.
#[derive(Debug)]
pub struct ClientStub<'a> {
    inner: ExecutionContext<'a>,
    instance: InstanceDescriptor<'a>,
}

impl CallStub for ClientStub<'_> {
    type Output = Result<(), ExecutionError>;

    fn call_stub(&mut self, method: MethodDescriptor<'_>, args: Vec<u8>) -> Self::Output {
        let call_info = CallInfo {
            instance_id: self.instance.id,
            method_id: method.id,
        };
        self.inner.call(method.interface_name, &call_info, &args)
    }
}
