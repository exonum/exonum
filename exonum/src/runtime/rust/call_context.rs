use exonum_merkledb::{BinaryValue, Fork};

use super::super::{
    dispatcher::{Dispatcher, Error as DispatcherError, Schema},
    error::catch_panic,
    ArtifactId, CallInfo, Caller, ExecutionContext, ExecutionError, InstanceDescriptor, InstanceId,
    InstanceSpec, MethodId, SUPERVISOR_INSTANCE_ID,
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

    /// Accesses dispatcher information.
    pub fn dispatcher(&self) -> Schema<&Fork> {
        Schema::new(self.fork())
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

    /// Isolates execution of the provided closure.
    ///
    /// This method should be used with extreme care, since it subverts the usual rules
    /// of transaction roll-back provided by Exonum. Namely:
    ///
    /// - If the execution of the closure is successful, all changes to the blockchain state
    ///   preceding to the `isolate()` call are permanently committed. These changes **will not**
    ///   be rolled back if the following transaction code exits with an error.
    /// - If the execution of the closure errors, all changes to the blockchain state
    ///   preceding to the `isolate()` call are rolled back right away. That is, they are not
    ///   persisted even if the following transaction code executes successfully.
    ///
    /// For these reasons, it is strongly advised to propagate the `Result` returned by this method,
    /// as a result of the transaction execution.
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

    #[doc(hidden)]
    pub fn supervisor_extensions(&self) -> Option<SupervisorExtensions> {
        if self.instance().id == SUPERVISOR_INSTANCE_ID {
            Some(SupervisorExtensions {
                dispatcher: self.inner.dispatcher,
                fork: &*self.inner.fork,
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
    dispatcher: &'a Dispatcher,
    fork: &'a Fork,
}

impl<'a> SupervisorExtensions<'a> {
    pub fn start_artifact_registration(
        &self,
        artifact: ArtifactId,
        spec: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        Dispatcher::start_artifact_registration(self.fork, artifact, spec)
    }

    pub fn start_adding_service(
        &self,
        artifact: ArtifactId,
        instance_name: String,
        constructor: impl BinaryValue,
    ) -> Result<(), ExecutionError> {
        let instance_spec = InstanceSpec {
            artifact,
            name: instance_name,
            id: self.dispatcher.assign_instance_id(self.fork),
        };
        self.dispatcher
            .start_adding_service(self.fork, instance_spec, constructor)
    }
}
