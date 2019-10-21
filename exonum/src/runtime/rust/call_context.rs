use exonum_merkledb::{BinaryValue, Fork};
use futures::Future;

use super::super::{
    dispatcher::Error as DispatcherError, error::catch_panic, ArtifactId, CallInfo, Caller,
    ExecutionContext, ExecutionError, InstanceDescriptor, InstanceId, InstanceSpec, MethodId,
    SUPERVISOR_INSTANCE_ID,
};
use crate::{blockchain::Schema as CoreSchema, helpers::ValidatorId};

/// Context for the executed call.
///
/// The call can mean a transaction call, or the `before_commit` hook.
#[derive(Debug)]
pub struct CallContext<'a> {
    /// Underlying execution context.
    inner: ExecutionContext<'a>,
    /// ID of the executing service.
    instance_id: InstanceId,
}

impl<'a> CallContext<'a> {
    /// Creates a new transaction context for the specified execution context and the instance
    /// descriptor.
    pub(crate) fn new(context: ExecutionContext<'a>, instance_id: InstanceId) -> Self {
        Self {
            inner: context,
            instance_id,
        }
    }

    /// Returns a writable snapshot of the current blockchain state.
    pub fn fork(&self) -> &Fork {
        self.inner.fork
    }

    /// Returns the initiator of the actual transaction execution.
    pub fn caller(&self) -> &Caller {
        &self.inner.caller
    }

    pub fn instance(&self) -> InstanceDescriptor {
        InstanceDescriptor {
            id: self.instance_id,
            // Safety of `unwrap()` is guaranteed by construction of `CallContext` instances:
            // we check that `instance_id` exists before creating the instance.
            name: self
                .inner
                .dispatcher
                .service_name(self.instance_id)
                .unwrap(),
        }
    }

    /// Returns the validator ID if the transaction author is a validator.
    pub fn validator_id(&self) -> Option<ValidatorId> {
        // TODO Perhaps we should optimize this method [ECR-3222]
        self.caller().author().and_then(|author| {
            CoreSchema::new(self.fork())
                .consensus_config()
                .find_validator(|validator_keys| author == validator_keys.service_key)
        })
    }

    #[doc(hidden)]
    pub fn call(
        &mut self,
        interface_name: impl AsRef<str>,
        method_id: MethodId,
        arguments: impl BinaryValue,
    ) -> Result<(), ExecutionError> {
        let call_info = CallInfo {
            instance_id: self.instance_id,
            method_id,
        };
        self.inner.call(
            interface_name.as_ref(),
            &call_info,
            arguments.into_bytes().as_ref(),
        )
    }

    // TODO This method is hidden until it is fully tested in next releases. [ECR-3493]
    /// Creates a client to call interface methods of the specified service instance.
    #[doc(hidden)]
    pub fn interface<'s, T>(&'s mut self, called: InstanceId) -> Result<T, ExecutionError>
    where
        T: From<CallContext<'s>>,
    {
        self.call_context(called).map(Into::into)
    }

    // TODO This method is hidden until it is fully tested in next releases. [ECR-3493]
    #[doc(hidden)]
    pub fn call_context(&mut self, called_id: InstanceId) -> Result<CallContext, ExecutionError> {
        self.inner
            .dispatcher
            .service_name(called_id)
            .ok_or(DispatcherError::IncorrectInstanceId)?;
        Ok(CallContext {
            inner: self.inner.child_context(self.instance_id),
            instance_id: called_id,
        })
    }

    /// Checks the caller of this method with the specified closure.
    ///
    /// If the closure returns `Some(value)`, then the method returns `Some((value, fork))` thus you
    /// get a write access to the blockchain state. Otherwise this method returns
    /// an occurred error.
    pub fn verify_caller<F, T>(&self, predicate: F) -> Option<(T, &Fork)>
    where
        F: Fn(&Caller) -> Option<T>,
    {
        // TODO Think about returning structure with the named fields instead of unnamed tuple
        // to make code more clear. [ECR-3222]
        predicate(&self.inner.caller).map(|result| (result, &*self.inner.fork))
    }

    #[doc(hidden)]
    pub fn supervisor_extensions(&mut self) -> Option<SupervisorExtensions> {
        if self.instance().id == SUPERVISOR_INSTANCE_ID {
            Some(SupervisorExtensions {
                inner: self.reborrow(),
            })
        } else {
            None
        }
    }

    fn reborrow(&mut self) -> CallContext {
        CallContext {
            inner: self.inner.reborrow(),
            instance_id: self.instance_id,
        }
    }
}

#[derive(Debug)]
pub struct SupervisorExtensions<'a> {
    inner: CallContext<'a>,
}

impl<'a> SupervisorExtensions<'a> {
    fn execution_context(&mut self) -> &mut ExecutionContext<'a> {
        &mut self.inner.inner
    }

    pub fn start_deploy(
        &mut self,
        artifact: ArtifactId,
        spec: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        self.execution_context()
            .dispatcher
            .deploy_artifact(artifact, spec)
            .wait()
    }

    pub fn register_artifact(
        &mut self,
        artifact: ArtifactId,
        spec: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        let exec_context = self.execution_context();
        exec_context
            .dispatcher
            .register_artifact(exec_context.fork, &artifact, spec)
    }

    pub fn add_service(
        &mut self,
        artifact: ArtifactId,
        instance_name: String,
        config: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        let ctx = self.execution_context();
        let instance_spec = InstanceSpec {
            artifact,
            name: instance_name,
            id: ctx.dispatcher.assign_instance_id(ctx.fork),
        };
        ctx.dispatcher
            .add_service(ctx.fork, instance_spec.clone(), config)
    }

    pub fn isolate(
        &mut self,
        f: impl FnOnce(CallContext) -> Result<(), ExecutionError>,
    ) -> Result<(), ExecutionError> {
        let result = catch_panic(|| f(self.inner.reborrow()));
        match result {
            Ok(()) => self.inner.inner.fork.flush(),
            Err(_) => self.inner.inner.fork.rollback(),
        }
        result
    }
}
