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

use crate::runtime::{
    dispatcher::Dispatcher,
    mailbox::{Action, AfterRequestCompleted, BlockchainMailbox},
    CallInfo, ConfigChange, ExecutionContext, ExecutionError, InstanceId,
};

pub trait SupervisorAccess {}

#[derive(Debug)]
pub struct CommunicationChannel<'a, T = ()> {
    dispatcher: &'a Dispatcher,
    pub(crate) mailbox: &'a BlockchainMailbox,
    phantom: std::marker::PhantomData<T>,
}

impl<'a, T> CommunicationChannel<'a, T> {
    pub(crate) fn new(mailbox: &'a BlockchainMailbox, dispatcher: &'a Dispatcher) -> Self {
        Self {
            mailbox,
            dispatcher,
            phantom: std::marker::PhantomData,
        }
    }

    /// Call the corresponding runtime method.
    pub fn call(
        &self,
        context: &ExecutionContext,
        call_info: &CallInfo,
        arguments: &[u8],
    ) -> Result<(), ExecutionError> {
        self.dispatcher.call(context, call_info, arguments)
    }

    /// Opens an extended interface for supervisor.
    #[doc(hidden)]
    pub fn supervisor_interface<A>(&'a self, _requestor: &A) -> CommunicationChannel<'a, A>
    where
        A: SupervisorAccess,
    {
        CommunicationChannel::<A>::new(self.mailbox, self.dispatcher)
    }
}

impl<'a, T> CommunicationChannel<'a, T>
where
    T: SupervisorAccess,
{
    /// Adds a request to the list of pending actions. These changes will be applied immediately
    /// before the block commit.
    ///
    /// Currently only the supervisor service is allowed to perform this action.
    /// If any other instance will call this method, the request will be ignored.
    #[doc(hidden)]
    pub fn request_action(&self, action: Action, and_then: AfterRequestCompleted) {
        self.mailbox.add_request(action, and_then);
    }

    /// Adds a configuration update to pending actions. These changes will be applied immediately
    /// before the block commit.
    ///
    /// Only the supervisor service is allowed to perform this action.
    /// If any other instance will call this method, the request will be ignored.
    #[doc(hidden)]
    pub fn update_config(&self, caller_instance_id: InstanceId, changes: Vec<ConfigChange>) {
        let action = Action::UpdateConfig {
            caller_instance_id,
            changes,
        };
        self.mailbox.add_request(action, None);
    }
}
