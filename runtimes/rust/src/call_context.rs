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

use exonum::{
    helpers::Height,
    merkledb::{access::Prefixed, Fork},
    runtime::{
        BlockchainData, CallInfo, Caller, CoreError, ExecutionContext, ExecutionContextUnstable,
        ExecutionError, InstanceDescriptor, InstanceQuery, SupervisorExtensions,
        SUPERVISOR_INSTANCE_ID,
    },
};

use super::{GenericCallMut, MethodDescriptor};

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

    /// Returns `true` if currently processed block is a genesis block.
    pub fn in_genesis_block(&self) -> bool {
        let core_schema = self.data().for_core();
        core_schema.next_height() == Height(0)
    }

    /// Returns a stub which uses fallthrough auth to authorize calls.
    #[doc(hidden)] // TODO: Hidden until fully tested in next releases. [ECR-3494]
    pub fn with_fallthrough_auth(&mut self) -> FallthroughAuth<'_> {
        FallthroughAuth(CallContext {
            inner: self.inner.reborrow(),
            instance: self.instance,
        })
    }

    /// Returns extensions required for the Supervisor service implementation.
    ///
    /// This method can only be called by the supervisor; the call will panic otherwise.
    #[doc(hidden)]
    pub fn supervisor_extensions(&mut self) -> SupervisorExtensions<'_> {
        if self.instance.id != SUPERVISOR_INSTANCE_ID {
            panic!("`supervisor_extensions` called within a non-supervisor service");
        }

        self.inner.supervisor_extensions()
    }

    fn make_child_call<'q>(
        &mut self,
        called_id: impl Into<InstanceQuery<'q>>,
        method: MethodDescriptor<'_>,
        args: Vec<u8>,
        fallthrough_auth: bool,
    ) -> Result<(), ExecutionError> {
        let descriptor = self
            .inner
            .get_service(called_id)
            .ok_or(CoreError::IncorrectInstanceId)?;

        let call_info = CallInfo::new(descriptor.id, method.id, method.interface_name);

        let caller = if fallthrough_auth {
            None
        } else {
            Some(self.instance.id)
        };

        self.inner
            .make_child_call(method.interface_name, &call_info, &args, caller)
    }
}

impl<'a, I> GenericCallMut<I> for CallContext<'a>
where
    I: Into<InstanceQuery<'a>>,
{
    type Output = Result<(), ExecutionError>;

    fn generic_call_mut(
        &mut self,
        called_id: I,
        method: MethodDescriptor<'_>,
        args: Vec<u8>,
    ) -> Self::Output {
        self.make_child_call(called_id, method, args, false)
    }
}

#[derive(Debug)]
pub struct FallthroughAuth<'a>(CallContext<'a>);

impl<'a, I> GenericCallMut<I> for FallthroughAuth<'a>
where
    I: Into<InstanceQuery<'a>>,
{
    type Output = Result<(), ExecutionError>;

    fn generic_call_mut(
        &mut self,
        called_id: I,
        method: MethodDescriptor<'_>,
        args: Vec<u8>,
    ) -> Self::Output {
        self.0.make_child_call(called_id, method, args, true)
    }
}
