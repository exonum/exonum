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

use exonum_merkledb::{BinaryValue, Fork};

use crate::runtime::{
    CallInfo, Caller, CommunicationChannel, ExecutionContext, ExecutionError, InstanceId, MethodId,
};

// TODO Write a full documentation when the interservice communications are fully implemented. [ECR-3493]
/// Provide a low level context for the call of methods of a different service instance.
#[derive(Debug)]
pub struct CallContext<'a> {
    /// Identifier of the caller service instance.
    caller: InstanceId,
    /// Identifier of the called service instance.
    called: InstanceId,
    /// The current state of the blockchain.
    fork: &'a Fork,
    /// Reference to the communication channel.
    communication_channel: &'a CommunicationChannel<'a>,
    /// Depth of call stack.
    /// Since we can call services from other services, we should be aware
    /// of the recursion and terminate tx execution if recursion limit was exceeded.
    call_stack_depth: usize,
}

impl<'a> CallContext<'a> {
    /// Create a new call context.
    pub fn new(
        fork: &'a Fork,
        communication_channel: &'a CommunicationChannel<'a>,
        caller: InstanceId,
        called: InstanceId,
    ) -> Self {
        Self {
            caller,
            called,
            fork,
            communication_channel,
            call_stack_depth: 0,
        }
    }

    /// Create a new call context for the given execution context.
    pub fn from_execution_context(
        inner: &'a ExecutionContext<'a>,
        caller: InstanceId,
        called: InstanceId,
    ) -> Self {
        Self {
            caller,
            called,
            fork: inner.fork,
            communication_channel: inner.communication_channel,
            call_stack_depth: inner.call_stack_depth,
        }
    }

    /// Perform the method call of the specified service interface.
    pub fn call(
        &self,
        interface_name: impl AsRef<str>,
        method_id: MethodId,
        arguments: impl BinaryValue,
        // TODO ExecutionError here mislead about the true cause of an occurred error. [ECR-3222]
    ) -> Result<(), ExecutionError> {
        let context = ExecutionContext {
            fork: self.fork,
            communication_channel: self.communication_channel,
            caller: Caller::Service {
                instance_id: self.caller,
            },
            interface_name: interface_name.as_ref(),
            call_stack_depth: self.call_stack_depth + 1,
        };
        let call_info = CallInfo {
            method_id,
            instance_id: self.called,
        };

        if context.call_stack_depth >= ExecutionContext::MAX_CALL_STACK_DEPTH {
            let kind = crate::runtime::dispatcher::Error::StackOverflow;
            let msg = format!(
                "Maximum depth of call stack has been reached. `MAX_CALL_STACK_DEPTH` is {}.",
                ExecutionContext::MAX_CALL_STACK_DEPTH
            );
            return Err((kind, msg).into());
        }

        self.communication_channel
            .call(&context, &call_info, arguments.into_bytes().as_ref())
    }
}
