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
    helpers::ValidateInput,
    runtime::{rust::CallContext, DispatcherError, ExecutionError, InstanceSpec},
};
use exonum_derive::*;
use exonum_merkledb::ObjectHash;

use std::collections::HashSet;

use super::{
    ConfigChange, ConfigProposalWithHash, ConfigPropose, ConfigVote, ConfigureCall,
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
        context: CallContext,
        artifact: DeployRequest,
    ) -> Result<(), ExecutionError>;

    /// Confirmation that the artifact was successfully deployed by the validator.
    ///
    /// Artifact will be registered in dispatcher if all of validators will send this confirmation.
    fn confirm_artifact_deploy(
        &self,
        context: CallContext,
        artifact: DeployConfirmation,
    ) -> Result<(), ExecutionError>;

    /// Requests start service.
    ///
    /// Service will be started if all of validators will send this confirmation.
    fn start_service(
        &self,
        context: CallContext,
        service: StartService,
    ) -> Result<(), ExecutionError>;

    /// Propose config change
    ///
    /// This request should be sent by one of validators as the proposition to change
    /// current configuration to new one. All another validators able to vote for this
    /// configuration by sending `confirm_config_change` transaction.
    /// Note: only one proposal at time is possible.
    fn propose_config_change(
        &self,
        context: CallContext,
        propose: ConfigPropose,
    ) -> Result<(), ExecutionError>;

    /// Confirm config change
    ///
    /// This confirm should be sent by validators to vote for proposed configuration.
    /// Vote of the author of the propose_config_change transaction is taken into
    /// account automatically.
    /// The configuration will be applied if 2/3+1 validators voted for it.
    fn confirm_config_change(
        &self,
        context: CallContext,
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

impl SupervisorInterface for Supervisor {
    fn propose_config_change(
        &self,
        mut context: CallContext,
        propose: ConfigPropose,
    ) -> Result<(), ExecutionError> {
        let (_, author) = context
            .caller()
            .as_transaction()
            .ok_or(DispatcherError::UnauthorizedCaller)?;

        // Verifies that transaction author is validator.
        context
            .data()
            .for_core()
            .validator_id(author)
            .ok_or(Error::UnknownAuthor)?;

        // Verifies that the `actual_from` height is in the future.
        if context.data().for_core().height() >= propose.actual_from {
            return Err(Error::ActualFromIsPast.into());
        }

        // Verifies that there are no pending config changes.
        if Schema::new(context.service_data())
            .pending_proposal
            .exists()
        {
            return Err(Error::ConfigProposeExists.into());
        }

        // To prevent multiple consensus change proposition in one request
        let mut consensus_propose_added = false;
        // To prevent multiple service change proposition in one request
        let mut service_ids = HashSet::new();

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
                        .interface::<ConfigureCall>(config.instance_id)?
                        .verify_config(config.params.clone())
                        .map_err(|e| (Error::MalformedConfigPropose, e))?;
                }
            }
        }

        let mut schema = Schema::new(context.service_data());
        let propose_hash = propose.object_hash();
        schema.config_confirms.confirm(&propose_hash, author);

        let config_entry = ConfigProposalWithHash {
            config_propose: propose,
            propose_hash,
        };
        schema.pending_proposal.set(config_entry);

        Ok(())
    }

    fn confirm_config_change(
        &self,
        context: CallContext,
        vote: ConfigVote,
    ) -> Result<(), ExecutionError> {
        let (_, author) = context
            .caller()
            .as_transaction()
            .ok_or(DispatcherError::UnauthorizedCaller)?;

        // Verify that transaction author is a validator.
        let core_schema = context.data().for_core();
        core_schema
            .validator_id(author)
            .ok_or(Error::UnknownAuthor)?;

        let mut schema = Schema::new(context.service_data());
        let entry = schema
            .pending_proposal
            .get()
            .ok_or_else(|| Error::ConfigProposeNotRegistered)?;

        // Verifies that this config proposal is registered.
        if entry.propose_hash != vote.propose_hash {
            return Err(Error::ConfigProposeNotRegistered.into());
        }

        let config_propose = entry.config_propose;
        // Verifies that we didn't reach the deadline height.
        if config_propose.actual_from <= core_schema.height() {
            return Err(Error::DeadlineExceeded.into());
        }
        if schema
            .config_confirms
            .confirmed_by(&entry.propose_hash, &author)
        {
            return Err(Error::AttemptToVoteTwice.into());
        }

        schema.config_confirms.confirm(&vote.propose_hash, author);
        log::trace!(
            "Propose config {:?} has been confirmed by {:?}",
            vote.propose_hash,
            author
        );

        Ok(())
    }

    fn request_artifact_deploy(
        &self,
        context: CallContext,
        deploy: DeployRequest,
    ) -> Result<(), ExecutionError> {
        deploy.validate()?;
        let core_schema = context.data().for_core();
        // Verifies that we doesn't reach deadline height.
        if deploy.deadline_height < core_schema.height() {
            return Err(Error::DeadlineExceeded.into());
        }
        let mut schema = Schema::new(context.service_data());

        // Verifies that the deployment request is not yet registered.
        if schema.pending_deployments.contains(&deploy.artifact) {
            return Err(Error::DeployRequestAlreadyRegistered.into());
        }

        // Verifies that transaction author is validator.
        let author = context.caller().author().ok_or(Error::UnknownAuthor)?;
        core_schema
            .validator_id(author)
            .ok_or(Error::UnknownAuthor)?;

        // Verifies that the artifact is not deployed yet.
        if context
            .data()
            .for_dispatcher()
            .get_artifact(&deploy.artifact.name)
            .is_some()
        {
            return Err(Error::AlreadyDeployed.into());
        }

        let confirmations = schema.deploy_requests.confirm(&deploy, author);
        let validator_count = core_schema.consensus_config().validator_keys.len();
        if confirmations == validator_count {
            log::trace!("Deploy artifact request accepted {:?}", deploy.artifact);
            let artifact = deploy.artifact.clone();
            schema.pending_deployments.put(&artifact, deploy);
        }
        Ok(())
    }

    fn confirm_artifact_deploy(
        &self,
        context: CallContext,
        confirmation: DeployConfirmation,
    ) -> Result<(), ExecutionError> {
        confirmation.validate()?;
        let core_schema = context.data().for_core();

        // Verifies that transaction author is validator.
        let author = context.caller().author().ok_or(Error::UnknownAuthor)?;
        core_schema
            .validator_id(author)
            .ok_or(Error::UnknownAuthor)?;

        let mut schema = Schema::new(context.service_data());
        // Verifies that this deployment is registered.
        let deploy_request = schema
            .pending_deployments
            .get(&confirmation.artifact)
            .ok_or(Error::DeployRequestNotRegistered)?;

        // Verifies that we didn't reach deadline height.
        if deploy_request.deadline_height < core_schema.height() {
            return Err(Error::DeadlineExceeded.into());
        }

        let confirmations = schema.deploy_confirmations.confirm(&confirmation, author);
        let validator_count = core_schema.consensus_config().validator_keys.len();
        if confirmations == validator_count {
            log::trace!(
                "Registering deployed artifact in dispatcher {:?}",
                confirmation.artifact
            );

            // Removes artifact from pending deployments.
            schema.pending_deployments.remove(&confirmation.artifact);
            // We have enough confirmations to register the deployed artifact in the dispatcher;
            // if this action fails, this transaction will be canceled.
            context.start_artifact_registration(deploy_request.artifact, deploy_request.spec)?;
        }

        Ok(())
    }

    fn start_service(
        &self,
        mut context: CallContext,
        service: StartService,
    ) -> Result<(), ExecutionError> {
        service.validate()?;
        let author = context.caller().author().ok_or(Error::UnknownAuthor)?;
        let core_schema = context.data().for_core();
        core_schema
            .validator_id(author)
            .ok_or(Error::UnknownAuthor)?;

        // Verifies that we doesn't reach deadline height.
        if service.deadline_height < core_schema.height() {
            return Err(Error::DeadlineExceeded.into());
        }

        // Verifies that the instance name does not exist.
        if context
            .data()
            .for_dispatcher()
            .get_instance(service.name.as_str())
            .is_some()
        {
            return Err(Error::InstanceExists.into());
        }

        let confirmations = Schema::new(context.service_data())
            .pending_instances
            .confirm(&service, author);
        let validator_count = core_schema.consensus_config().validator_keys.len();
        if confirmations == validator_count {
            log::trace!(
                "Request add service with name {:?} from artifact {:?}",
                service.name,
                service.artifact
            );
            // We have enough confirmations to add a new service instance;
            // if this action fails this transaction will be canceled.
            context.start_adding_service(service.artifact, service.name, service.config)?;
        }

        Ok(())
    }
}
