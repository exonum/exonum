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

use futures::Future;

use exonum_merkledb::Fork;

// pub use blockchain_secretary::{BlockchainSecretary,};
use crate::runtime::{
    dispatcher::{Dispatcher, Error as DispatcherError},
    error::{catch_panic, ExecutionError},
    ArtifactId, CallInfo, ConfigChange, InstanceId, InstanceSpec,
};

use std::cell::RefCell;

/// Optional callback to be called after request is completed by the blockchain core.
pub type AfterRequestCompleted = Option<Box<dyn FnOnce() + 'static>>;

/// Internal notification for blockchain core about actions happened during the
/// mailbox processing.
#[derive(Debug, Clone)]
pub enum Notification {
    /// Notification about adding a deployed artifact into some runtime.
    ArtifactDeployed { artifact: ArtifactId },
    /// Notification about instance added into some runtime.
    InstanceStarted {
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

/// Mailbox processing context.
///
/// This enum defines the rules of requests processing:
/// if `MailboxContext` is the `TxExecution`, then any failure appeared during
/// processing will be reported (so blockchain core will be able to revert the transaction).
/// Otherwise any failure will be simply ignored.
#[derive(Debug, Clone)]
pub enum MailboxContext {
    /// Mailbox processing happening during the transaction execution process.
    TxExecution(CallInfo),
    /// Mailbox processing outside of the transaction execution process.
    OutsideTxExecution,
}

/// An interface for runtimes to interact with the Exonum blockchain core.
///
/// All the requests added to the mailbox will be processed by the core `Blockchain` structure.
///
/// **Important note:** All the requests received during the transaction execution are considered
/// **the part of execution process**. So, if service requests blockchain to perform some action
/// and an error occurs during the request processing, the transaction will be treated as failed
/// and, as a result, rolled back.
///
/// **Policy on request failures:**
///
/// Services **will not** be notified if request was failed or ignored. So it's up to the service
/// implementors to build the logic in such a way that lack of result will not break the service
/// state.
///
/// Services are able to provide `AfterRequestCompleted` callback and consider the situation when
/// callback is not called at the some point of time as failed/ignored request.
pub struct BlockchainMailbox {
    context: MailboxContext,
    requests: RefCell<Vec<(Action, AfterRequestCompleted)>>,
    notifications: Vec<Notification>,
}

impl std::fmt::Debug for BlockchainMailbox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BlockchainMailbox")
    }
}

impl BlockchainMailbox {
    /// Creates a new empty mailbox.
    pub fn new(context: MailboxContext) -> Self {
        Self {
            context,
            requests: RefCell::new(Vec::new()),
            notifications: Vec::new(),
        }
    }

    /// Adds a request for action to the mailbox.
    pub fn add_request(&self, action: Action, and_then: AfterRequestCompleted) {
        let mut requests = self.requests.borrow_mut();
        requests.push((action, and_then));
    }

    /// Adds a notification about completed event to the mailbox.
    fn add_notification(&mut self, notification: Notification) {
        self.notifications.push(notification);
    }

    /// Drains requests from the mailbox.
    fn drain_requests(&mut self) -> Vec<(Action, AfterRequestCompleted)> {
        let mut requests = RefCell::new(Vec::default());
        std::mem::swap(&mut requests, &mut self.requests);
        requests.into_inner()
    }

    // TODO: currently blockchain doesn't read notifications, because services started
    // on start of the node aren't added into notifications list.
    // This should be fixed and notifications mechanism should be used instead of
    // `dispatcher.notify_api_changes`.
    #[allow(dead_code)]
    /// Consumes a mailbox, receiving the notifications about performed actions.
    pub(crate) fn get_notifications(self) -> Vec<Notification> {
        self.notifications
    }

    /// Processes requests within immutable context.
    /// Since any failures won't affect the blockchain state, they are simply ignored.
    pub(crate) fn process_requests(&mut self, dispatcher: &mut Dispatcher) {
        let requests = self.drain_requests();

        for (request, and_then) in requests {
            // Requests within an immutable context are considered independent
            // so we don't care about the result (however, we'll add it to the log).
            let result = self.process_request(dispatcher, None, request.clone(), and_then);

            match result {
                Ok(_) => trace!("Successfully completed request {:?}", request),
                Err(err) => {
                    warn!(
                        "Error occurred during request {:?} within context {:?}: {:?}",
                        request, self.context, &err
                    );
                }
            }
        }
    }

    /// Processes requests within mutable context.
    /// Depending on the mailbox context, failure of request execution
    /// may lead to the stop of the whole processing process.
    pub(crate) fn process_requests_mut(
        &mut self,
        dispatcher: &mut Dispatcher,
        fork: &mut Fork,
    ) -> Result<(), ExecutionError> {
        let requests = self.drain_requests();

        for (request, and_then) in requests {
            let result = self.process_request(dispatcher, Some(fork), request.clone(), and_then);

            match result {
                Ok(_) => {
                    self.flush_if_needed(fork);
                    trace!("Successfully completed request {:?}", request);
                }
                Err(err) => {
                    // Cancel any changes occurred during the errored request.
                    fork.rollback();

                    warn!(
                        "Error occurred during request {:?} within context {:?}: {:?}",
                        request, self.context, &err
                    );
                    self.check_if_should_stop(err)?;
                }
            }
        }

        Ok(())
    }

    /// Depending on the context of the mailbox flush the fork if needed:
    /// if requests are independent, we should flush the fork after each request,
    /// so failure of one request won't affect others.
    fn flush_if_needed(&self, fork: &mut Fork) {
        if let MailboxContext::OutsideTxExecution = self.context {
            fork.flush();
        }
    }

    /// Depending on the context of the mailbox decides if we should stop processing
    /// requests or can continue.
    fn check_if_should_stop(&self, err: ExecutionError) -> Result<(), ExecutionError> {
        match self.context {
            MailboxContext::TxExecution(_) => {
                // Requests created within transaction execution process are considered
                // highly tied (since multiple requests are possible only if one service
                // called transaction of other service, and both of them requested an action),
                // so failure of the one means failure of others too.
                Err(err)
            }
            _ => {
                // If this is not a transaction execution context (e.g. `before_commit`),
                // requests are considered independent, so we can safely skip failed one.
                Ok(())
            }
        }
    }

    fn process_request(
        &mut self,
        dispatcher: &mut Dispatcher,
        fork: Option<&mut Fork>,
        request: Action,
        call_after: AfterRequestCompleted,
    ) -> Result<(), ExecutionError> {
        // After the action execution we should call the callback
        // if it is provided.
        // Since callback is untrusted code, it's wrapped into
        // `catch_panic`.
        let callback = move |()| {
            catch_panic(move || {
                if let Some(callback) = call_after {
                    callback();
                }
                Ok(())
            })
        };

        catch_panic(|| match request {
            // Immutable action.
            Action::StartDeploy { artifact, spec } => {
                // Request the dispatcher to start deploy process and wait until it completed.
                // Please note that it doesn't mean completion of the deployment process,
                // because after that artifact should be registered as deployed.

                // TODO: Get rid of `wait` here.
                dispatcher.deploy_artifact(artifact, spec).wait()
            }

            // Mutable action.
            Action::RegisterArtifact { artifact, spec } => {
                // Request the dispatcher to registered artifact as deployed.
                // Performing this action means the completion of the deployment process,
                // artifact will be available in the list of deployed artifacts.

                let fork = fork.ok_or(DispatcherError::InappropriateTimeForAction)?;
                dispatcher.register_artifact(fork, &artifact, spec)?;
                self.add_notification(Notification::ArtifactDeployed { artifact });

                Ok(())
            }

            // Mutable action.
            Action::AddService {
                artifact,
                instance_name,
                config,
            } => {
                // Request the dispatcher to start an instance of service given the
                // deployed artifact.

                let fork = fork.ok_or(DispatcherError::InappropriateTimeForAction)?;

                let instance_spec = InstanceSpec {
                    artifact,
                    name: instance_name,
                    id: dispatcher.assign_instance_id(fork),
                };

                dispatcher.add_service(fork, instance_spec.clone(), config)?;

                let part_of_core_api = is_part_of_core_api(&instance_spec);
                self.add_notification(Notification::InstanceStarted {
                    instance: instance_spec,
                    part_of_core_api,
                });

                Ok(())
            }

            Action::UpdateConfig {
                caller_instance_id,
                changes,
            } => {
                // Request dispatcher to change the configuration of the blockchain
                // part (internal or service).

                let fork = fork.ok_or(DispatcherError::InappropriateTimeForAction)?;
                dispatcher.update_config(self, fork, caller_instance_id, changes);
                Ok(())
            }
        })
        .and_then(callback)
    }
}

/// Internal function encapsulating the check for service
/// to be a part of core api schema.
fn is_part_of_core_api(instance_spec: &InstanceSpec) -> bool {
    // Currently, only Rust runtime uses API schema provided by Exonum.
    instance_spec.artifact.runtime_id == crate::runtime::RuntimeIdentifier::Rust as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_part_of_core_api() {
        let rust_runtime_id = crate::runtime::RuntimeIdentifier::Rust as u32;

        let rust_artifact = format!("{}:artifact_name", rust_runtime_id);
        let rust_instance_spec = InstanceSpec::new(1024, "instance", &rust_artifact).unwrap();
        assert_eq!(is_part_of_core_api(&rust_instance_spec), true);

        let non_rust_artifact = format!("{}:artifact_name", rust_runtime_id + 1);
        let non_rust_instance_spec =
            InstanceSpec::new(1024, "instance", &non_rust_artifact).unwrap();
        assert_eq!(is_part_of_core_api(&non_rust_instance_spec), false);
    }
}
