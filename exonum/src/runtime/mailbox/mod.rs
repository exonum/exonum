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

//! The module containing interfaces to request changes in the blockchain structure.

pub mod blockchain_secretary;

use std::collections::HashMap;

use crate::runtime::{ArtifactId, ConfigChange, InstanceId, InstanceSpec};

/// Optional callback to be called after request is completed by the blockchain core.
pub type AfterRequestCompleted = Option<Box<dyn FnOnce() + 'static>>;

/// An interface for runtimes to interact with the Exonum blockchain core.
///
/// All the requests added to the mailbox will be processed by the core `Blockchain` structure.
/// However, `Blockchain` structure can legitimately ignore any of the requests if it will decide
/// that request initiator has no permission to request such a change.
///
/// **Important note:** All the requests received after the transaction execution are considered
/// **the part of execution process**. So, if service requests blockchain to perform some action
/// and blockchain declines this request, the transaction will be treated as failed and, as a
/// result, rolled back.
///
/// If request gets declined after any other runtime method invocation, it will be simply ignored.
///
/// In theory, runtimes can provide services the possibility to write directly to the mailbox.
/// However, since services considered untrusted code by default, any runtime implementation
/// should be aware that service can pretend to be some another service in ordere to make
/// Exonum core do what it want.
///
/// Thus, it's highly recommended for runtimes to create a proxy entity and manage creating
/// requests manually.
///
/// **Policy on request failures:**
///
/// Services **will not** be notified if request was failed or ignored. So it's up to the service
/// implementors to build the logic in such a way that lack of result will not break the service
/// state.
///
/// Services are able to provide `AfterRequestCompleted` callback and consider the situation when
/// callback is not called at the some point of time as failed/ignored request.
#[derive(Default)]
pub struct BlockchainMailbox {
    requests: HashMap<InstanceId, (Action, AfterRequestCompleted)>,
    notifications: Vec<Notification>,
}

impl std::fmt::Debug for BlockchainMailbox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BlockchainMailbox")
    }
}

impl BlockchainMailbox {
    /// Creates a new empty mailbox.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a request for action to the mailbox.
    pub fn add_request(
        &mut self,
        instance_id: InstanceId,
        action: Action,
        and_then: AfterRequestCompleted,
    ) {
        self.requests.insert(instance_id, (action, and_then));
    }

    /// Adds a notification about completed event to the mailbox.
    pub fn add_notification(&mut self, notification: Notification) {
        self.notifications.push(notification);
    }

    /// Drains requests from the mailbox.
    pub(self) fn drain_requests(&mut self) -> HashMap<InstanceId, (Action, AfterRequestCompleted)> {
        let mut requests = HashMap::default();
        std::mem::swap(&mut requests, &mut self.requests);
        requests
    }

    /// Consumes a mailbox, receiving the notifications about performed actions.
    pub fn get_notifications(self) -> Vec<Notification> {
        self.notifications
    }
}

/// Internal notification for blockchain core about actions happened during the
/// mailbox processing.
#[derive(Debug, Clone)]
pub enum Notification {
    /// Notification about adding a deployed artifact into some runtime.
    ArtifactDeployed { artifact: ArtifactId },
    /// Notifiaction about instance added into some runtime.
    InstanceStarted {
        instance: InstanceSpec,
        part_of_core_api: bool,
    },
    /// Notification about instance removed from some runtime.
    /// Currently not used (since `Runtime::stop` method was removed) and
    /// can not be emitted.
    InstanceRemoved {
        instance: InstanceSpec,
        part_of_core_api: bool,
    },
}

/// Enum denoting a request to perform a change in the Exonum blockchain structure.
#[derive(Debug, Clone)]
pub enum Action {
    /// Request to start artifact deployment process.
    StartDeploy { artifact: ArtifactId, spec: Vec<u8> },
    /// Request to register the deployed artifact in the blockchain.
    /// Make sure that you successfully complete the deploy artifact procedure.
    RegisterArtifact { artifact: ArtifactId, spec: Vec<u8> },
    /// Request to add a new service instance with the specified params.
    /// Make sure that the artifact is deployed.
    AddService {
        artifact: ArtifactId,
        instance_name: String,
        config: Vec<u8>,
    },
    /// Request to perform a configuration update with the specified changes.
    /// Make sure that no errors occur when applying these changes.
    UpdateConfig {
        caller_instance_id: InstanceId,
        changes: Vec<ConfigChange>,
    },
}
