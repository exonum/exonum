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

//! This module contains the entities capable of processing messages received through
//! `BlockchainMailbox`.

// TODO only for development purposes, remove it later
use futures::Future;

use exonum_merkledb::Fork;

use crate::runtime::{
    dispatcher::{Dispatcher, Error as DispatcherError},
    error::{catch_panic, ExecutionError},
    CallInfo, InstanceSpec,
};

use super::{Action, AfterRequestCompleted, BlockchainMailbox, Notification};

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

/// `BlockchainSecretary` is an intermediate entity capable of processing the requests
/// from services to `Blockchain`, and delegating the allowed actions to the dispatcher.
/// Also `BlockchainSecretary` creates notifications for `Blockchain` about performed actions
/// so `Blockchain` will be able to do any follow-up things if they are needed.
#[derive(Debug)]
pub struct BlockchainSecretary {
    context: MailboxContext,
}

impl BlockchainSecretary {
    /// Creates a new `BlockchainSecretary` instance.
    pub fn new(context: MailboxContext) -> Self {
        Self { context }
    }

    /// Processes requests within immutable context.
    /// Since any failures won't affect the blockchain state, they are simply ignored.
    pub fn process_requests(&self, mailbox: &mut BlockchainMailbox, dispatcher: &mut Dispatcher) {
        // During requests processing new requests may appear.
        loop {
            let requests = mailbox.drain_requests();

            if requests.is_empty() {
                break;
            }

            for (request, and_then) in requests {
                // Requests within an immutable context are considered independent
                // so we don't care about the result.
                let _result = self.process_request(mailbox, dispatcher, None, request, and_then);
            }
        }
    }

    /// Processes requests within mutable context.
    /// Depending on the mailbox context, failure of request execution
    /// may lead to the stop of the whole processing process.
    pub fn process_requests_mut(
        &self,
        mailbox: &mut BlockchainMailbox,
        dispatcher: &mut Dispatcher,
        fork: &mut Fork,
    ) -> Result<(), ExecutionError> {
        // During requests processing new requests may appear.
        loop {
            let requests = mailbox.drain_requests();

            if requests.is_empty() {
                break;
            }

            for (request, and_then) in requests {
                let result = self.process_request(
                    mailbox,
                    dispatcher,
                    Some(fork),
                    request.clone(),
                    and_then,
                );

                match result {
                    Ok(_) => {
                        self.flush_if_needed(fork);
                        trace!("Successfully completed request {:?}", request);
                    }
                    Err(err) => {
                        // Cancel any changes occurred during the errored request.
                        fork.rollback();

                        trace!(
                            "Error occurred during request {:?} within context {:?}",
                            request,
                            self.context
                        );
                        self.check_if_should_stop(err)?;
                    }
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
        &self,
        mailbox: &mut BlockchainMailbox,
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

                mailbox.add_notification(Notification::ArtifactDeployed { artifact });

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
                mailbox.add_notification(Notification::InstanceStarted {
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

                dispatcher.update_config(mailbox, fork, caller_instance_id, changes);
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
