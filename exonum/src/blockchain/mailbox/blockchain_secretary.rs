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
#![allow(dead_code)]

use futures::Future;

use exonum_merkledb::Fork;

use crate::runtime::{
    dispatcher::{Dispatcher, Error as DispatcherError},
    error::{catch_panic, ExecutionError},
    CallInfo, InstanceId, InstanceSpec,
};

use super::{Action, AfterRequestCompleted, BlockchainMailbox, Notification};

/// Mailbox processing context.
///
/// This enum defines the rules of requests processing:
/// if `MailboxContext` is the `TxExecution`, then any failure appeared during
/// processing will be reported (so blockchain core will be able to revert the transaction).
/// Otherwise any failure will be simply ignored.
#[derive(Debug)]
pub enum MailboxContext {
    /// Mailbox processing happening during the transaction execution process.
    TxExecution(CallInfo),
    /// Mailbox processing outside of the transaction execution process.
    NoTx,
}

/// `BlockchainSecretary` is an intermediate entity capable of processing the requests
/// from services to `Blockchain`, and delegating the allowed actions to the dispatcher.
/// Also `BlockchainSecretary` creates notifications for `Blockchain` about performed actions
/// so `Blockchain` will be able to do any follow-up things if they are needed.
pub struct BlockchainSecretary<'a> {
    context: MailboxContext,
    fork: Option<&'a mut Fork>,
}

impl<'a> BlockchainSecretary<'a> {
    pub fn new(context: MailboxContext, fork: Option<&'a mut Fork>) -> Self {
        Self { context, fork }
    }

    pub fn process_requests(
        &self,
        mailbox: &mut BlockchainMailbox,
        dispatcher: &mut Dispatcher,
    ) -> Result<(), ExecutionError> {
        for (request_initiator, (request, and_then)) in mailbox.drain_requests() {
            let authorized_access = self.verify_caller(request_initiator)?;

            // Ignore unauthorized requests.
            if !authorized_access {
                continue;
            }

            // TODO should we exit early on first error?
            // Since there is no more than one action per instance, we should just collect
            // results.
            self.process_request(mailbox, dispatcher, None, request, and_then)?;
        }

        Ok(())
    }

    pub fn process_requests_mut(
        &self,
        mailbox: &mut BlockchainMailbox,
        dispatcher: &mut Dispatcher,
        fork: &mut Fork,
    ) -> Result<(), ExecutionError> {
        for (request_initiator, (request, and_then)) in mailbox.drain_requests() {
            let authorized_access = self.verify_caller(request_initiator)?;

            // Ignore unauthorized requests.
            if !authorized_access {
                continue;
            }

            // TODO should we exit early on first error?
            self.process_request(mailbox, dispatcher, Some(fork), request, and_then)?;
        }

        Ok(())
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
            Action::StartDeploy { artifact, spec } => {
                dispatcher.deploy_artifact(artifact, spec).wait()
            }

            Action::RegisterArtifact { artifact, spec } => {
                let fork = fork.ok_or(DispatcherError::InappropriateTimeForAction)?;

                dispatcher.register_artifact(fork, &artifact, spec)?;

                mailbox.add_notification(Notification::ArtifactDeployed { artifact });

                Ok(())
            }

            Action::AddService {
                artifact,
                instance_name,
                config,
            } => {
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
                let fork = fork.ok_or(DispatcherError::InappropriateTimeForAction)?;

                dispatcher.update_config(fork, caller_instance_id, changes);
                Ok(())
            }
        })
        .and_then(callback)
    }

    /// Checks if caller has sufficient rights to perform request.
    /// If context is `TxExecution` then unauthorized request is considered
    /// an error and `Err` will be returned.
    /// Otherwise unauthorized access will result in `Ok(false)`.
    fn verify_caller(&self, initiator: InstanceId) -> Result<bool, ExecutionError> {
        let authorized_access = is_authorized_for_requests(initiator);

        if let MailboxContext::TxExecution(ref call_info) = self.context {
            if call_info.instance_id != initiator {
                return Err(DispatcherError::FakeInitiator.into());
            }

            if !authorized_access {
                return Err(DispatcherError::UnauthorizedCaller.into());
            }
        }

        Ok(authorized_access)
    }
}

/// Internal function encapsulating the check for service
/// to have sufficient rights to request actions from the blockchain.
fn is_authorized_for_requests(instance_id: InstanceId) -> bool {
    instance_id == crate::runtime::SUPERVISOR_INSTANCE_ID
}

/// Internal function encapsulating the check for service
/// to be a part of core api schema.
fn is_part_of_core_api(instance_spec: &InstanceSpec) -> bool {
    // Currently, only Rust runtime uses API schema provided by Exonum.
    instance_spec.artifact.runtime_id == crate::runtime::RuntimeIdentifier::Rust as u32
}
