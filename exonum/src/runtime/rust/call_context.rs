use exonum_merkledb::{BinaryValue, Fork, Prefixed};

use crate::blockchain::Schema as CoreSchema;
use crate::runtime::{
    dispatcher::{Dispatcher, Error as DispatcherError},
    error::catch_panic,
    ArtifactId, BlockchainData, CallInfo, Caller, ExecutionContext, ExecutionError,
    InstanceDescriptor, InstanceId, InstanceQuery, InstanceSpec, MethodId, SUPERVISOR_INSTANCE_ID,
};

/// Context for the executed call.
///
/// The call can mean a transaction call, or the `before_commit` hook.
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
    pub fn data(&self) -> BlockchainData<&'_ Fork> {
        BlockchainData::new(self.inner.fork, self.instance)
    }

    /// Provides access to the data of the executing service.
    pub fn service_data(&self) -> Prefixed<&'_ Fork> {
        self.data().for_executing_service()
    }

    /// Returns the initiator of the actual transaction execution.
    pub fn caller(&self) -> &Caller {
        &self.inner.caller
    }

    pub fn instance(&self) -> InstanceDescriptor {
        self.instance
    }

    #[doc(hidden)]
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

    // TODO This method is hidden until it is fully tested in next releases. [ECR-3494]
    /// Creates a client to call interface methods of the specified service instance.
    #[doc(hidden)]
    pub fn interface<'s, T>(&'s mut self, called: InstanceId) -> Result<T, ExecutionError>
    where
        T: From<CallContext<'s>>,
    {
        self.call_context(called).map(Into::into)
    }

    // TODO This method is hidden until it is fully tested in next releases. [ECR-3494]
    #[doc(hidden)]
    pub fn call_context<'s>(
        &'s mut self,
        called_id: impl Into<InstanceQuery<'s>>,
    ) -> Result<CallContext<'s>, ExecutionError> {
        let descriptor = self
            .inner
            .dispatcher
            .get_service(self.inner.fork, called_id)
            .ok_or(DispatcherError::IncorrectInstanceId)?;
        Ok(CallContext {
            inner: self.inner.child_context(self.instance.id),
            instance: descriptor,
        })
    }

    /// Isolates execution of the provided closure.
    ///
    /// This method should be used with extreme care, since it subverts the usual rules
    /// of transaction roll-back provided by Exonum. Namely:
    ///
    /// - If the execution of the closure is successful, all changes to the blockchain state
    ///   preceding the `isolate()` call are permanently committed. These changes **will not**
    ///   be rolled back if the following transaction code exits with an error.
    /// - If the execution of the closure errors, all changes to the blockchain state
    ///   preceding the `isolate()` call are rolled back right away. That is, they are not
    ///   persisted even if the following transaction code executes successfully.
    ///
    /// If there are several `isolate()` calls within the same execution context,
    /// commitment / rollback rules are applied to changes since the last call.
    ///
    /// For these reasons, it is strongly advised to:
    ///
    /// - Make `isolate()` call(s) the last logic executed by a transaction, or at least
    ///   not have failure cases after the call(s).
    /// - Propagate errors returned by this method as a result of the transaction execution.
    // TODO: Finalize interface and test [ECR-3740]
    #[doc(hidden)]
    pub fn isolate(
        &mut self,
        f: impl FnOnce(CallContext) -> Result<(), ExecutionError>,
    ) -> Result<(), ExecutionError> {
        let result = catch_panic(|| f(self.reborrow()));
        match result {
            Ok(()) => self.inner.fork.flush(),
            Err(_) => self.inner.fork.rollback(),
        }
        result
    }

    fn reborrow(&mut self) -> CallContext<'_> {
        CallContext {
            inner: self.inner.reborrow(),
            instance: self.instance,
        }
    }

    /// Provides writeable access to core schema.
    ///
    /// This method can only be called by the supervisor; the call will panic otherwise.
    #[doc(hidden)]
    pub fn writeable_core_schema(&self) -> CoreSchema<&Fork> {
        if self.instance.id != SUPERVISOR_INSTANCE_ID {
            panic!("`writeable_core_schema` called within a non-supervisor service");
        }
        CoreSchema::get_unchecked(self.inner.fork)
    }

    /// Marks an artifact as *registered*, i.e., one which service instances can be deployed from.
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
        artifact: ArtifactId,
        instance_name: String,
        constructor: impl BinaryValue,
    ) -> Result<(), ExecutionError> {
        if self.instance.id != SUPERVISOR_INSTANCE_ID {
            panic!("`start_adding_service` called within a non-supervisor service");
        }

        let instance_spec = InstanceSpec {
            artifact,
            name: instance_name,
            id: Dispatcher::assign_instance_id(self.inner.fork),
        };
        self.inner
            .child_context(self.instance.id)
            .start_adding_service(instance_spec, constructor)
    }
}
