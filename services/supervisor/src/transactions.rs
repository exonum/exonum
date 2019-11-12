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
#[exonum_service]
pub trait SupervisorInterface {
    /// Requests artifact deploy.
    ///
    /// This request should be sent by the each of validators.
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
    /// current configuration to new one. All another validators able to vote for this
    /// configuration by sending `confirm_config_change` transaction.
    /// The configuration application rules rely on the `Supervisor` mode.
    /// Note: only one proposal at time is possible.
    fn propose_config_change(
        &self,
        context: CallContext<'_>,
        propose: ConfigPropose,
    ) -> Result<(), ExecutionError>;

    /// Confirm config change
    ///
    /// This confirm should be sent by validators to vote for proposed configuration.
    /// Vote of the author of the propose_config_change transaction is taken into
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

impl ValidateInput for StartService {
    type Error = ExecutionError;

    fn validate(&self) -> Result<(), Self::Error> {
        self.artifact
            .validate()
            .map_err(|e| (Error::InvalidArtifactId, e))?;
        InstanceSpec::is_valid_name(&self.name)
            .map_err(|e| (Error::InvalidInstanceName, e))
            .map_err(Self::Error::from)
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

        // To prevent multiple consensus change proposition in one request
        let mut consensus_propose_added = false;
        // To prevent multiple service change proposition in one request
        let mut service_ids = HashSet::new();
        // To prevent multiple services start in one request.
        let mut services_to_start = HashSet::new();

        // Perform config verification.
        for change in &propose.changes {
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
                    if service_ids.contains(&config.instance_id) {
                        log::trace!("Discarded multiple service change proposals in one request.");
                        return Err(Error::MalformedConfigPropose.into());
                    }
                    service_ids.insert(config.instance_id);

                    context
                        .interface::<ConfigureCall<'_>>(config.instance_id)?
                        .verify_config(config.params.clone())
                        .map_err(|e| (Error::MalformedConfigPropose, e))?;
                }

                ConfigChange::StartService(start_service) => {
                    start_service.validate()?;

                    if context
                        .dispatcher_info()
                        .get_artifact(start_service.artifact.name.as_str())
                        .is_none()
                    {
                        log::trace!(
                            "Discarded start of service {} from the unknown artifact {}.",
                            &start_service.name,
                            &start_service.artifact.name,
                        );

                        return Err(Error::UnknownArtifact.into());
                    }

                    if context
                        .dispatcher_info()
                        .get_instance(start_service.name.as_str())
                        .is_some()
                    {
                        log::trace!(
                            "Discarded start of the already running instance {}.",
                            &start_service.name
                        );
                        return Err(Error::InstanceExists.into());
                    }

                    if services_to_start.contains(&start_service.name) {
                        log::trace!(
                            "Discarded multiple instances with the same name in one request."
                        );
                        return Err(Error::MalformedConfigPropose.into());
                    }

                    services_to_start.insert(&start_service.name);
                }
            }
        }

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

        // Verifies that the deployment request is not yet registered.
        if schema.pending_deployments().contains(&deploy.artifact) {
            return Err(Error::DeployRequestAlreadyRegistered.into());
        }

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
