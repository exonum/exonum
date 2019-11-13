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

use exonum::{
    blockchain,
    helpers::{Height, ValidateInput},
    runtime::{rust::CallContext, Caller, DispatcherError, ExecutionError, InstanceSpec},
};
use exonum_derive::*;
use exonum_merkledb::ObjectHash;

use std::collections::HashSet;

use super::{
    mode, ConfigChange, ConfigProposalWithHash, ConfigPropose, ConfigVote, ConfigureCall,
    DeployConfirmation, DeployRequest, Error, Schema, StartService, Supervisor,
};

/// Supervisor service transactions.
#[exonum_interface]
pub trait SupervisorInterface {
    /// Requests artifact deploy.
    ///
    /// This request should be initiated by the validator (and depending on the `Supervisor`
    /// mode several other actions can be required, e.g. sending the same request by majority
    /// of other validators as well).
    /// After that, the supervisor will try to deploy the artifact, and if this procedure
    /// will be successful it will send `confirm_artifact_deploy` transaction.
    fn request_artifact_deploy(
        &self,
        context: CallContext<'_>,
        artifact: DeployRequest,
    ) -> Result<(), ExecutionError>;

    /// Confirmation that the artifact was successfully deployed by the validator.
    ///
    /// Artifact will be registered in dispatcher if all of validators will send this confirmation.
    fn confirm_artifact_deploy(
        &self,
        context: CallContext<'_>,
        artifact: DeployConfirmation,
    ) -> Result<(), ExecutionError>;

    /// Propose config change
    ///
    /// This request should be sent by one of validators as the proposition to change
    /// current configuration to new one. All another validators are able to vote for this
    /// configuration by sending `confirm_config_change` transaction.
    /// The configuration application rules rely on the `Supervisor` mode, e.g. confirmations
    /// are not required for the `Simple` mode, and for `Decentralized` mode (2/3+1) confirmations
    /// are required.
    /// Note: only one proposal at time is possible.
    fn propose_config_change(
        &self,
        context: CallContext<'_>,
        propose: ConfigPropose,
    ) -> Result<(), ExecutionError>;

    /// Confirm config change
    ///
    /// This confirm should be sent by validators to vote for proposed configuration.
    /// Vote of the author of the `propose_config_change` transaction is taken into
    /// account automatically.
    /// The configuration application rules rely on the `Supervisor` mode.
    fn confirm_config_change(
        &self,
        context: CallContext<'_>,
        vote: ConfigVote,
    ) -> Result<(), ExecutionError>;
}

impl ValidateInput for DeployRequest {
    type Error = ExecutionError;

    fn validate(&self) -> Result<(), Self::Error> {
        self.artifact
            .validate()
            .map_err(|e| (Error::InvalidArtifactId, e).into())
    }
}

impl ValidateInput for DeployConfirmation {
    type Error = ExecutionError;

    fn validate(&self) -> Result<(), Self::Error> {
        self.artifact
            .validate()
            .map_err(|e| (Error::InvalidArtifactId, e).into())
    }
}

impl StartService {
    fn validate(&self, context: &CallContext<'_>) -> Result<(), ExecutionError> {
        self.artifact
            .validate()
            .map_err(|e| (Error::InvalidArtifactId, e))?;
        InstanceSpec::is_valid_name(&self.name)
            .map_err(|e| (Error::InvalidInstanceName, e))
            .map_err(ExecutionError::from)?;

        // Check that artifact is deployed.
        if context
            .dispatcher_info()
            .get_artifact(self.artifact.name.as_str())
            .is_none()
        {
            log::trace!(
                "Discarded start of service {} from the unknown artifact {}.",
                &self.name,
                &self.artifact.name,
            );

            return Err(Error::UnknownArtifact.into());
        }

        // Check that there is no instance with the same name.
        if context
            .dispatcher_info()
            .get_instance(self.name.as_str())
            .is_some()
        {
            log::trace!(
                "Discarded start of the already running instance {}.",
                &self.name
            );
            return Err(Error::InstanceExists.into());
        }

        Ok(())
    }
}

impl<Mode> SupervisorInterface for Supervisor<Mode>
where
    Mode: mode::SupervisorMode,
{
    fn propose_config_change(
        &self,
        mut context: CallContext<'_>,
        mut propose: ConfigPropose,
    ) -> Result<(), ExecutionError> {
        let ((_, author), fork) = context
            .verify_caller(Caller::as_transaction)
            .ok_or(DispatcherError::UnauthorizedCaller)?;
        let schema = Schema::new(context.instance().name, fork);

        // Verifies that transaction author is validator.
        let config_confirms = schema.config_confirms();
        config_confirms
            .validator_id(author)
            .ok_or(Error::UnknownAuthor)?;

        let current_height = blockchain::Schema::new(fork).height();

        // If `actual_from` field is not set, set it to the next height.
        if propose.actual_from == Height(0) {
            propose.actual_from = current_height.next();
        }
        // Otherwise verify that the `actual_from` height is in the future.
        else if current_height >= propose.actual_from {
            return Err(Error::ActualFromIsPast.into());
        }

        // Verifies that there are no pending config changes.
        if schema.pending_proposal().exists() {
            return Err(Error::ConfigProposeExists.into());
        }

        // Verify changes in the proposal.
        self.verify_config_changeset(&mut context, &propose.changes)?;

        let schema = Schema::new(context.instance().name, context.fork());

        // After all the checks verify that configuration number is expected one.
        if propose.configuration_number != schema.get_configuration_number() {
            return Err(Error::IncorrectConfigurationNumber.into());
        }
        schema.increase_configuration_number();

        let propose_hash = propose.object_hash();
        schema.config_confirms().confirm(&propose_hash, author);

        let config_entry = ConfigProposalWithHash {
            config_propose: propose,
            propose_hash,
        };
        schema.pending_proposal().set(config_entry);

        Ok(())
    }

    fn confirm_config_change(
        &self,
        context: CallContext<'_>,
        vote: ConfigVote,
    ) -> Result<(), ExecutionError> {
        let ((_, author), fork) = context
            .verify_caller(Caller::as_transaction)
            .ok_or(DispatcherError::UnauthorizedCaller)?;

        let blockchain_height = blockchain::Schema::new(fork).height();
        let schema = Schema::new(context.instance().name, fork);

        // Verifies that transaction author is validator.
        let mut config_confirms = schema.config_confirms();
        config_confirms
            .validator_id(author)
            .ok_or(Error::UnknownAuthor)?;

        let entry = schema
            .pending_proposal()
            .get()
            .ok_or_else(|| Error::ConfigProposeNotRegistered)?;

        // Verifies that this config proposal is registered.
        if entry.propose_hash != vote.propose_hash {
            return Err(Error::ConfigProposeNotRegistered.into());
        }

        let config_propose = entry.config_propose;
        // Verifies that we didn't reach the deadline height.
        if config_propose.actual_from <= blockchain_height {
            return Err(Error::DeadlineExceeded.into());
        }

        if config_confirms.confirmed_by(&entry.propose_hash, &author) {
            return Err(Error::AttemptToVoteTwice.into());
        }

        config_confirms.confirm(&vote.propose_hash, author);
        log::trace!(
            "Propose config {:?} has been confirmed by {:?}",
            vote.propose_hash,
            author
        );

        Ok(())
    }

    fn request_artifact_deploy(
        &self,
        context: CallContext<'_>,
        deploy: DeployRequest,
    ) -> Result<(), ExecutionError> {
        deploy.validate()?;
        let blockchain_schema = blockchain::Schema::new(context.fork());
        // Verifies that we doesn't reach deadline height.
        if deploy.deadline_height < blockchain_schema.height() {
            return Err(Error::ActualFromIsPast.into());
        }
        let schema = Schema::new(context.instance().name, context.fork());

        // Verifies that transaction author is validator.
        let mut deploy_requests = schema.deploy_requests();
        let author = context
            .caller()
            .author()
            .expect("Wrong `DeployRequest` initiator");

        deploy_requests
            .validator_id(author)
            .ok_or(Error::UnknownAuthor)?;

        // Verifies that the artifact is not deployed yet.
        if context
            .dispatcher_info()
            .get_artifact(&deploy.artifact.name)
            .is_some()
        {
            return Err(Error::AlreadyDeployed.into());
        }

        // If deployment is already registered, check whether request is initiated
        if schema.pending_deployments().contains(&deploy.artifact) {
            let new_confirmation = !deploy_requests.confirmed_by(&deploy, &author);
            if new_confirmation {
                // It's OK, just an additional confirmation.
                deploy_requests.confirm(&deploy, author);
                return Ok(());
            } else {
                // Author already confirmed deployment of this artifact,
                // so it's a duplicate.
                return Err(Error::DeployRequestAlreadyRegistered.into());
            }
        }

        deploy_requests.confirm(&deploy, author);
        if Mode::deploy_approved(&deploy, &deploy_requests) {
            log::trace!("Deploy artifact request accepted {:?}", deploy.artifact);
            let artifact = deploy.artifact.clone();
            schema.pending_deployments().put(&artifact, deploy);
        }
        Ok(())
    }

    fn confirm_artifact_deploy(
        &self,
        context: CallContext<'_>,
        confirmation: DeployConfirmation,
    ) -> Result<(), ExecutionError> {
        confirmation.validate()?;
        let blockchain_schema = blockchain::Schema::new(context.fork());

        let schema = Schema::new(context.instance().name, context.fork());

        // Verifies that transaction author is validator.
        let author = context
            .caller()
            .author()
            .expect("Wrong `DeployConfirmation` initiator");

        let mut deploy_confirmations = schema.deploy_confirmations();
        deploy_confirmations
            .validator_id(author)
            .ok_or(Error::UnknownAuthor)?;

        // Verifies that this deployment is registered.
        let deploy_request = schema
            .pending_deployments()
            .get(&confirmation.artifact)
            .ok_or(Error::DeployRequestNotRegistered)?;

        // Verifies that we didn't reach deadline height.
        if deploy_request.deadline_height < blockchain_schema.height() {
            return Err(Error::DeadlineExceeded.into());
        }

        let confirmations = deploy_confirmations.confirm(&confirmation, author);
        if confirmations == deploy_confirmations.validators_amount() {
            log::trace!(
                "Registering deployed artifact in dispatcher {:?}",
                confirmation.artifact
            );

            // Removes artifact from pending deployments.
            schema.pending_deployments().remove(&confirmation.artifact);
            // We have enough confirmations to register the deployed artifact in the dispatcher;
            // if this action fails, this transaction will be canceled.
            context.start_artifact_registration(deploy_request.artifact, deploy_request.spec)?;
        }

        Ok(())
    }
}

impl<Mode> Supervisor<Mode>
where
    Mode: mode::SupervisorMode,
{
    /// Verifies that each change introduced within config proposal is valid.
    fn verify_config_changeset(
        &self,
        context: &mut CallContext<'_>,
        changes: &[ConfigChange],
    ) -> Result<(), ExecutionError> {
        // To prevent multiple consensus change proposition in one request
        let mut consensus_propose_added = false;
        // To prevent multiple service change proposition in one request
        let mut service_ids = UniqueSet::new();
        // To prevent multiple services start in one request.
        let mut services_to_start = UniqueSet::new();

        // Perform config verification.
        for change in changes {
            match change {
                ConfigChange::Consensus(config) => {
                    if consensus_propose_added {
                        log::trace!(
                            "Discarded multiple consensus change proposals in one request."
                        );
                        return Err(Error::MalformedConfigPropose.into());
                    }
                    consensus_propose_added = true;

                    config
                        .validate()
                        .map_err(|e| (Error::MalformedConfigPropose, e))?;
                }

                ConfigChange::Service(config) => {
                    if !service_ids.check_unique(config.instance_id) {
                        log::trace!("Discarded multiple service change proposals in one request.");
                        return Err(Error::MalformedConfigPropose.into());
                    }

                    context
                        .interface::<ConfigureCall<'_>>(config.instance_id)?
                        .verify_config(config.params.clone())
                        .map_err(|e| (Error::MalformedConfigPropose, e))?;
                }

                ConfigChange::StartService(start_service) => {
                    if !services_to_start.check_unique(start_service.name.clone()) {
                        log::trace!(
                            "Discarded multiple instances with the same name in one request."
                        );
                        return Err(Error::MalformedConfigPropose.into());
                    }

                    start_service.validate(&context)?;
                }
            }
        }
        Ok(())
    }
}

/// A helper structure designed to check whether a newly
/// introduced element is unique among others.
#[derive(Debug, Default)]
struct UniqueSet<T: std::hash::Hash + Eq + Default> {
    inner: HashSet<T>,
}

impl<T: std::hash::Hash + Eq + Default> UniqueSet<T> {
    /// Creates a new `UniqueSet` object.
    pub fn new() -> Self {
        Self::default()
    }

    /// Checks whether element is unique, and if so
    /// adds it to the collection.
    pub fn check_unique(&mut self, element: T) -> bool {
        if self.inner.contains(&element) {
            return false;
        }

        self.inner.insert(element);
        true
    }
}
