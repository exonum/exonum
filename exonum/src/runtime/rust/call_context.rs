use exonum_merkledb::{access::Prefixed, BinaryValue, Fork};

use std::fmt;

use crate::blockchain::Schema as CoreSchema;
use crate::runtime::{
    dispatcher::{Dispatcher, Error as DispatcherError},
    error::{ErrorKind, ExecutionError, ServiceFail},
    ArtifactId, BlockchainData, CallInfo, Caller, ExecutionContext, InstanceDescriptor, InstanceId,
    InstanceQuery, InstanceSpec, MethodId, SUPERVISOR_INSTANCE_ID,
};

/// Context for the executed call. The call can mean a transaction call, a `before_commit` hook,
/// or a service constructor.
///
/// Use `*err` methods of the context together with [`ServiceFail`] trait to create errors
/// in the service code. More complex ways to create errors are rarely required and may not be
/// forward compatible.
///
/// [`ServiceFail`]: ../error/trait.ServiceFail.html
#[derive(Debug)]
pub struct CallContext<'a> {
    /// Underlying execution context.
    inner: ExecutionContext<'a>,
    /// ID of the executing service.
    instance: InstanceDescriptor<'a>,
    /// Call location.
    call_location: CallLocation,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CallLocation {
    Method { id: MethodId },
    Constructor,
    BeforeCommit,
}

impl fmt::Display for CallLocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CallLocation::Method { id } => write!(formatter, "method #{}", id),
            CallLocation::Constructor => formatter.write_str("constructor"),
            CallLocation::BeforeCommit => formatter.write_str("before_commit hook"),
        }
    }
}

impl<'a> CallContext<'a> {
    /// Creates a new transaction context for the specified execution context and the instance
    /// descriptor.
    pub(crate) fn new(
        context: ExecutionContext<'a>,
        instance: InstanceDescriptor<'a>,
        call_location: CallLocation,
    ) -> Self {
        Self {
            inner: context,
            instance,
            call_location,
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

    /// Creates an `ExecutionError` with the specified contents.
    pub fn err(&self, inner_error: impl ServiceFail) -> ExecutionError {
        let error_kind = ErrorKind::Service {
            code: inner_error.code(),
            instance_id: self.instance.id,
        };
        ExecutionError::new(error_kind, inner_error.description())
    }

    /// Creates an `ExecutionError` corresponding to a malformed argument.
    pub fn malformed_err(&self, details: impl fmt::Display) -> ExecutionError {
        let error_kind = DispatcherError::MalformedArguments.into();
        let description = format!(
            "Unable to parse argument for {method} in service {instance} \
             (interface `{interface}`): {details}",
            instance = self.instance,
            interface = self.inner.interface_name,
            method = self.call_location,
            details = details,
        );
        ExecutionError::new(error_kind, description)
    }

    /// Creates an `ExecutionError` corresponding to an unimplemented interface.
    pub fn no_interface_err(&self) -> ExecutionError {
        let error_kind = DispatcherError::NoSuchInterface.into();
        let description = format!(
            "Service {instance} does not implement `{interface}`",
            instance = self.instance,
            interface = self.inner.interface_name,
        );
        ExecutionError::new(error_kind, description)
    }

    /// Creates an `ExecutionError` corresponding to a missing or unimplemented method.
    pub fn no_method_err(&self, details: Option<&str>) -> ExecutionError {
        let error_kind = DispatcherError::NoSuchMethod.into();
        let mut description = format!(
            "{method} is absent in the `{interface}` interface of the service {instance}",
            instance = self.instance,
            interface = self.inner.interface_name,
            method = self.call_location,
        );
        if let Some(details) = details {
            description.push_str(": ");
            description.push_str(details);
        }

        ExecutionError::new(error_kind, description)
    }

    // TODO This method is hidden until it is fully tested in next releases. [ECR-3494]
    #[doc(hidden)]
    pub fn local_stub<'s>(
        &'s mut self,
        called_id: impl Into<InstanceQuery<'s>>,
    ) -> Result<LocalStub<'s>, ExecutionError> {
        let descriptor = self
            .inner
            .dispatcher
            .get_service(self.inner.fork, called_id)
            .ok_or(DispatcherError::IncorrectInstanceId)?;
        Ok(LocalStub {
            inner: self.inner.child_context(self.instance.id),
            instance: descriptor,
        })
    }

    // TODO This method is hidden until it is fully tested in next releases. [ECR-3494]
    /// Creates a client to call interface methods of the specified service instance.
    #[doc(hidden)]
    pub fn interface<'s, T>(&'s mut self, called: InstanceId) -> Result<T, ExecutionError>
    where
        T: From<LocalStub<'s>>,
    {
        self.local_stub(called).map(Into::into)
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

/// Local client stub for executing calls to the services. One can obtain a stub by calling
/// the `CallContext::local_stub` method.
// TODO: refine stubs (ECR-3910)
#[doc(hidden)]
#[derive(Debug)]
pub struct LocalStub<'a> {
    inner: ExecutionContext<'a>,
    instance: InstanceDescriptor<'a>,
}

impl LocalStub<'_> {
    /// Invokes an arbitrary method in the stub.
    pub fn call(
        &mut self,
        interface_name: impl AsRef<str>,
        method_id: MethodId,
        arguments: impl BinaryValue,
    ) -> Result<(), ExecutionError> {
        let call_info = CallInfo {
            instance_id: self.instance.id,
            method_id,
        };
        self.inner.call(
            interface_name.as_ref(),
            &call_info,
            arguments.into_bytes().as_ref(),
        )
    }
}
