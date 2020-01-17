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

use crate::{
    helpers::Height,
    merkledb::{access::Prefixed, Fork},
    runtime::{
        BlockchainData, CallInfo, Caller, ExecutionContext, ExecutionContextUnstable,
        ExecutionError, InstanceDescriptor, InstanceId, InstanceQuery, SupervisorExtensions,
        SUPERVISOR_INSTANCE_ID,
    },
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
    pub fn new(context: ExecutionContext<'a>, instance: InstanceDescriptor<'a>) -> Self {
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
}

/// Collection of unstable call context features.
#[doc(hidden)]
pub trait CallContextUnstable {
    /// Re-borrows an call context with the same interface name.
    fn reborrow(&mut self) -> CallContext<'_>;
    /// Returns the service matching the specified query.
    fn get_service<'q>(&self, id: impl Into<InstanceQuery<'q>>) -> Option<InstanceDescriptor<'_>>;
    /// Invokes the interface method of the instance with the specified ID.
    /// You may override the instance ID of the one who calls this method by the given one.
    fn make_child_call(
        &mut self,
        interface_name: &str,
        call_info: &CallInfo,
        arguments: &[u8],
        caller: Option<InstanceId>,
    ) -> Result<(), ExecutionError>;
}

impl<'a> CallContextUnstable for CallContext<'a> {
    fn reborrow(&mut self) -> CallContext<'_> {
        CallContext::new(self.inner.reborrow(), self.instance)
    }

    fn get_service<'q>(&self, id: impl Into<InstanceQuery<'q>>) -> Option<InstanceDescriptor<'_>> {
        self.inner.get_service(id)
    }

    fn make_child_call(
        &mut self,
        interface_name: &str,
        call_info: &CallInfo,
        arguments: &[u8],
        caller: Option<InstanceId>,
    ) -> Result<(), ExecutionError> {
        self.inner
            .make_child_call(interface_name, call_info, arguments, caller)
    }
}
